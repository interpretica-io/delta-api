/*
 * Delta API
 *
 * Copyright 2024 Maxim Menshikov
 *
 * Permission is hereby granted, free of charge, to any person obtaining
 * a copy of this software and associated documentation files (the "Software"),
 * to deal in the Software without restriction, including without limitation
 * the rights to use, copy, modify, merge, publish, distribute, sublicense,
 * and/or sell copies of the Software, and to permit persons to whom the
 * Software is furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included
 * in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
 * OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
 * DEALINGS IN THE SOFTWARE.
 */

//! Safe Rust wrapper around the **libasp** C connection API.
//!
//! All protocol details (JSON-RPC serialisation, NNG/TCP transport, response
//! parsing) are handled by libasp itself.  This module is responsible only for
//! converting between idiomatic Rust types and the C structs/linked-lists
//! expected by libasp, and for enforcing memory-safety invariants.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};

use log::{debug, error};

use super::address::Address;
use super::analysis::{Analysis, AnalysisOutline, AnalysisState};
use super::enums::{
    AnalysisJobKind, AnalysisPhase, Compiler, Cpu, IdentificationStatus, Language, Os,
    ReportSeverity, ResourceType, SymbolOrigin,
};
use super::env::Environment;
use super::file::File;
use super::identification::{IdentificationFile, IdentificationReport};
use super::report::Report;
use super::result::AnalysisResult;
use super::status::{AspError, AspResult};
use super::symbol::{Symbol, SymbolCall, SymbolLocation};
use super::workspace::{Workspace, WorkspaceOutline};
use super::sys;

// ─────────────────────────────────────────────────────────────────────────────
// C-string helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Copy a C string pointer into a Rust `String` without freeing the pointer.
/// Returns `None` if `ptr` is null or the bytes are not valid UTF-8.
unsafe fn cstr_to_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
}

/// Copy a C string pointer into a Rust `String`, then free the pointer with
/// the C `free()` function.  Use this for getters whose documentation says
/// "must be freed with free()" (e.g. `asp_analysis_state_get_progress_entity`,
/// `asp_report_get_line_content`, `asp_address2str`).
unsafe fn cstr_to_string_free(ptr: *mut c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
    c_free(ptr as *mut c_void);
    Some(s)
}

/// Thin wrapper around the libc `free()` symbol so we avoid adding a `libc`
/// dependency just for this one call.
unsafe fn c_free(ptr: *mut c_void) {
    extern "C" {
        fn free(p: *mut c_void);
    }
    if !ptr.is_null() {
        free(ptr);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// asp_status → AspError
// ─────────────────────────────────────────────────────────────────────────────

/// Convert a non-zero `asp_status` return value into an [`AspError`].
fn asp_error(rc: sys::asp_status) -> AspError {
    match rc {
        -1 => AspError::FdFailure,
        -2 => AspError::ConnectionFailed,
        -3 => AspError::NoMemory,
        -4 => AspError::Io("transport error".into()),
        -5 => AspError::MalformedResult,
        -6 => AspError::Fail,
        -7 => AspError::InvalidArgument,
        -8 => AspError::NoEntry,
        -9 => AspError::TimedOut,
        _  => AspError::Fail,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Rust → C enum mappings
// ─────────────────────────────────────────────────────────────────────────────
//
// The C enums start at 0 = Unspecified / Unknown and match the order of our
// Rust enums, so we use explicit match arms to be safe against future
// reorderings.

fn to_c_language(l: &Language) -> u32 {
    match l {
        Language::Unspecified => 0,
        Language::C           => 1,
        Language::Cpp         => 2,
        Language::Ruc         => 3,
    }
}

fn to_c_compiler(c: &Compiler) -> u32 {
    match c {
        Compiler::Unspecified => 0,
        Compiler::Gcc         => 1,
        Compiler::Clang       => 2,
        Compiler::Ruc         => 3,
        Compiler::MingwGcc    => 4,
        Compiler::MingwClang  => 5,
        Compiler::Msvc        => 6,
        Compiler::ClangCl     => 7,
    }
}

fn to_c_cpu(c: &Cpu) -> u32 {
    match c {
        Cpu::Unspecified => 0,
        Cpu::X86         => 1,
        Cpu::X8664       => 2,
        Cpu::ArmLe       => 3,
        Cpu::ArmBe       => 4,
        Cpu::Arm64Le     => 5,
        Cpu::Arm64Be     => 6,
        Cpu::Mips32Le    => 7,
        Cpu::Mips32Be    => 8,
        Cpu::Mips64Le    => 9,
        Cpu::Mips64Be    => 10,
        Cpu::RucVm       => 11,
    }
}

fn to_c_os(o: &Os) -> u32 {
    match o {
        Os::Unspecified => 0,
        Os::Linux       => 1,
        Os::Windows     => 2,
        Os::Macos       => 3,
        Os::Baremetal   => 4,
        Os::RucVm       => 5,
    }
}

fn to_c_job_kind(k: &AnalysisJobKind) -> u32 {
    match k {
        AnalysisJobKind::Unknown        => 0,
        AnalysisJobKind::Defects        => 1,
        AnalysisJobKind::Identification => 2,
        AnalysisJobKind::Dependency     => 3,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// C → Rust enum mappings
// ─────────────────────────────────────────────────────────────────────────────

fn from_c_phase(v: u32) -> AnalysisPhase {
    AnalysisPhase::try_from(v).unwrap_or_default()
}

fn from_c_severity(v: u32) -> ReportSeverity {
    match v {
        1 => ReportSeverity::Paranoid,
        2 => ReportSeverity::Low,
        3 => ReportSeverity::Medium,
        4 => ReportSeverity::High,
        5 => ReportSeverity::Critical,
        _ => ReportSeverity::Unknown,
    }
}

fn from_c_language(v: u32) -> Language {
    match v {
        1 => Language::C,
        2 => Language::Cpp,
        3 => Language::Ruc,
        _ => Language::Unspecified,
    }
}

fn from_c_symbol_origin(v: u32) -> SymbolOrigin {
    match v {
        1 => SymbolOrigin::FromSource,
        2 => SymbolOrigin::External,
        3 => SymbolOrigin::StandardLibrary,
        4 => SymbolOrigin::Internal,
        _ => SymbolOrigin::Unknown,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Rust → C object builders
// ─────────────────────────────────────────────────────────────────────────────

/// Build a `*mut asp_address` for a local-file path.
unsafe fn build_c_address_local(path: &str) -> AspResult<*mut sys::asp_address> {
    let c_path = CString::new(path).map_err(|_| AspError::InvalidArgument)?;
    let addr = sys::asp_address_local_file(c_path.as_ptr());
    if addr.is_null() {
        Err(AspError::NoMemory)
    } else {
        Ok(addr)
    }
}

/// Build a `*mut asp_address` for an IP endpoint.
unsafe fn build_c_address_ip(host: &str, port: u16) -> AspResult<*mut sys::asp_address> {
    let c_host = CString::new(host).map_err(|_| AspError::InvalidArgument)?;
    let addr = sys::asp_address_ip(c_host.as_ptr(), port);
    if addr.is_null() {
        Err(AspError::NoMemory)
    } else {
        Ok(addr)
    }
}

/// Build a `*mut asp_address` for an NNG URL.
unsafe fn build_c_address_nng(url: &str) -> AspResult<*mut sys::asp_address> {
    let c_url = CString::new(url).map_err(|_| AspError::InvalidArgument)?;
    let addr = sys::asp_address_nng(c_url.as_ptr());
    if addr.is_null() {
        Err(AspError::NoMemory)
    } else {
        Ok(addr)
    }
}

/// Build a `*mut asp_address` from our Rust [`Address`].
unsafe fn build_c_address(addr: &Address) -> AspResult<*mut sys::asp_address> {
    match &addr.urn {
        None => Err(AspError::InvalidArgument),
        Some(urn) => {
            use super::address::AddressType;
            match addr.addr_type {
                AddressType::Inet | AddressType::Inet6 => {
                    build_c_address_ip(urn, addr.port.unwrap_or(0))
                }
                AddressType::Nng => build_c_address_nng(urn),
                _ => build_c_address_local(urn),
            }
        }
    }
}

/// Build a `*mut asp_env` from our Rust [`Environment`].
/// Ownership of the returned pointer belongs to the caller.
unsafe fn build_c_env(env: &Environment) -> AspResult<*mut sys::asp_env> {
    let c_env = sys::asp_env_create();
    if c_env.is_null() {
        return Err(AspError::NoMemory);
    }

    if let Some(lang) = &env.language {
        sys::asp_env_set_language(c_env, to_c_language(lang));
    }
    if let Some(compiler) = &env.compiler {
        sys::asp_env_set_compiler(c_env, to_c_compiler(compiler));
    }
    if let Some(cpu) = &env.cpu {
        sys::asp_env_set_cpu(c_env, to_c_cpu(cpu));
    }
    if let Some(os) = &env.runtime_os {
        sys::asp_env_set_runtime_os(c_env, to_c_os(os));
    }
    if let Some(wk) = &env.windows_kit {
        let c = CString::new(wk.as_str()).map_err(|_| AspError::InvalidArgument)?;
        sys::asp_env_set_windows_kit(c_env, c.as_ptr());
    }
    if let Some(vct) = &env.vctools {
        let c = CString::new(vct.as_str()).map_err(|_| AspError::InvalidArgument)?;
        sys::asp_env_set_vctools(c_env, c.as_ptr());
    }
    if let Some(loc) = &env.locale {
        let c = CString::new(loc.as_str()).map_err(|_| AspError::InvalidArgument)?;
        sys::asp_env_set_locale(c_env, c.as_ptr());
    }
    if env.override_higher_level {
        sys::asp_env_set_override_higher_level(c_env, true);
    }
    for inc in &env.include_dirs {
        let c_addr = build_c_address(&inc.address)?;
        // env takes ownership of the address (own_address = true)
        sys::asp_env_add_include_directory(c_env, c_addr, true);
    }

    Ok(c_env)
}

/// Build a `*mut asp_file` from our Rust [`File`].
/// Ownership of the returned pointer belongs to the caller.
unsafe fn build_c_file(file: &File) -> AspResult<*mut sys::asp_file> {
    let c_addr = build_c_address(&file.address)?;
    // file takes ownership of the address
    let c_file = sys::asp_file_create(c_addr, true);
    if c_file.is_null() {
        sys::asp_address_free(c_addr);
        return Err(AspError::NoMemory);
    }
    if let Some(env) = &file.env {
        let c_env = build_c_env(env)?;
        // file takes ownership of the env
        sys::asp_file_attach_env(c_file, c_env, true);
    }
    Ok(c_file)
}

/// Build a full `*mut asp_workspace` from our Rust [`Workspace`].
/// Ownership of the returned pointer belongs to the caller.
unsafe fn build_c_workspace(ws: &Workspace) -> AspResult<*mut sys::asp_workspace> {
    let c_ws = sys::asp_workspace_create();
    if c_ws.is_null() {
        return Err(AspError::NoMemory);
    }
    if let Some(name) = &ws.name {
        let c_name = CString::new(name.as_str()).map_err(|_| AspError::InvalidArgument)?;
        sys::asp_workspace_set_name(c_ws, c_name.as_ptr());
    }
    if ws.id != Workspace::ID_UNKNOWN {
        sys::asp_workspace_set_id(c_ws, ws.id);
    }
    if let Some(env) = &ws.env {
        let c_env = build_c_env(env)?;
        // workspace takes ownership
        sys::asp_workspace_attach_env(c_ws, c_env, true);
    }
    for file in &ws.files {
        let c_file = build_c_file(file)?;
        // workspace takes ownership
        sys::asp_workspace_add_file(c_ws, c_file, true);
    }
    Ok(c_ws)
}

/// Build a minimal `*mut asp_workspace` that only carries an ID.
/// Use this as a handle when calling connection functions that need the
/// workspace only to identify it on the server.
unsafe fn build_c_workspace_id(id: u32) -> AspResult<*mut sys::asp_workspace> {
    let c_ws = sys::asp_workspace_create();
    if c_ws.is_null() {
        return Err(AspError::NoMemory);
    }
    sys::asp_workspace_set_id(c_ws, id);
    Ok(c_ws)
}

/// Build a full `*mut asp_analysis` from our Rust [`Analysis`].
unsafe fn build_c_analysis(an: &Analysis) -> AspResult<*mut sys::asp_analysis> {
    let c_an = sys::asp_analysis_create();
    if c_an.is_null() {
        return Err(AspError::NoMemory);
    }
    if an.id != Analysis::ID_UNKNOWN {
        sys::asp_analysis_set_id(c_an, an.id);
    }
    for job in &an.jobs {
        let c_job = sys::asp_analysis_job_create();
        if c_job.is_null() {
            sys::asp_analysis_free(c_an);
            return Err(AspError::NoMemory);
        }
        sys::asp_analysis_job_set_kind(c_job, to_c_job_kind(&job.kind));
        // analysis takes ownership of the job
        sys::asp_analysis_add_job(c_an, c_job, true);
    }
    Ok(c_an)
}

/// Build a minimal `*mut asp_analysis` carrying only an ID.
unsafe fn build_c_analysis_id(id: u32) -> AspResult<*mut sys::asp_analysis> {
    let c_an = sys::asp_analysis_create();
    if c_an.is_null() {
        return Err(AspError::NoMemory);
    }
    sys::asp_analysis_set_id(c_an, id);
    Ok(c_an)
}

// ─────────────────────────────────────────────────────────────────────────────
// C → Rust linked-list walkers
// ─────────────────────────────────────────────────────────────────────────────

/// Convert a `*mut asp_symbol` linked list (owned by a result or location
/// struct) into a `Vec<Symbol>`.  The caller must NOT free the individual
/// nodes — they are owned by the parent C struct.
unsafe fn walk_symbols(head: *mut sys::asp_symbol) -> Vec<Symbol> {
    let mut out = Vec::new();
    let mut ptr = head;
    while !ptr.is_null() {
        out.push(Symbol {
            name:          cstr_to_string(sys::asp_symbol_get_name(ptr)),
            namespace:     cstr_to_string(sys::asp_symbol_get_namespace(ptr)),
            resource_type: ResourceType(sys::asp_symbol_get_resource_type(ptr)),
            origin:        from_c_symbol_origin(sys::asp_symbol_get_origin(ptr)),
        });
        ptr = sys::asp_symbol_next(ptr);
    }
    out
}

/// Convert a `*mut asp_file` linked list into a `Vec<File>`.
unsafe fn walk_files(head: *mut sys::asp_file) -> Vec<File> {
    let mut out = Vec::new();
    let mut ptr = head;
    while !ptr.is_null() {
        let addr_ptr = sys::asp_file_get_address(ptr);
        // asp_address has a public `address` field (char*) we can read directly
        // after bindgen exposes the struct layout.
        let path = if addr_ptr.is_null() {
            String::new()
        } else {
            // asp_address2str returns a newly allocated string → free it.
            let raw = sys::asp_address2str(addr_ptr);
            cstr_to_string_free(raw).unwrap_or_default()
        };
        out.push(File {
            address: Address::local_file(path),
            env: None,
        });
        ptr = sys::asp_file_next(ptr);
    }
    out
}

/// Convert a `*mut asp_symbol_location` linked list.
unsafe fn walk_symbol_locations(head: *mut sys::asp_symbol_location) -> Vec<SymbolLocation> {
    let mut out = Vec::new();
    let mut ptr = head;
    while !ptr.is_null() {
        out.push(SymbolLocation {
            name:    cstr_to_string(sys::asp_symbol_location_get_name(ptr)),
            files:   walk_files(sys::asp_symbol_location_get_file(ptr)),
            symbols: walk_symbols(sys::asp_symbol_location_get_symbol(ptr)),
        });
        ptr = sys::asp_symbol_location_next(ptr);
    }
    out
}

/// Convert a `*mut asp_symbol_call` linked list.
unsafe fn walk_symbol_calls(head: *mut sys::asp_symbol_call) -> Vec<SymbolCall> {
    let mut out = Vec::new();
    let mut ptr = head;
    while !ptr.is_null() {
        out.push(SymbolCall {
            callers: walk_symbols(sys::asp_symbol_call_get_caller(ptr)),
            callees: walk_symbols(sys::asp_symbol_call_get_callee(ptr)),
        });
        ptr = sys::asp_symbol_call_next(ptr);
    }
    out
}

/// Convert a `*mut asp_identification_report` linked list.
unsafe fn walk_ident_reports(
    head: *mut sys::asp_identification_report,
) -> Vec<IdentificationReport> {
    let mut out = Vec::new();
    let mut ptr = head;
    while !ptr.is_null() {
        out.push(IdentificationReport {
            language: from_c_language(sys::asp_identification_report_get_language(ptr)),
            status:   IdentificationStatus(
                sys::asp_identification_report_get_status(ptr),
            ),
        });
        ptr = sys::asp_identification_report_next(ptr);
    }
    out
}

/// Convert a `*mut asp_identification_file` linked list.
unsafe fn walk_ident_files(head: *mut sys::asp_identification_file) -> Vec<IdentificationFile> {
    let mut out = Vec::new();
    let mut ptr = head;
    while !ptr.is_null() {
        let c_file = sys::asp_identification_file_get_file(ptr);
        let file = if c_file.is_null() {
            None
        } else {
            let addr_ptr = sys::asp_file_get_address(c_file);
            let path = if addr_ptr.is_null() {
                String::new()
            } else {
                let raw = sys::asp_address2str(addr_ptr);
                cstr_to_string_free(raw).unwrap_or_default()
            };
            Some(File {
                address: Address::local_file(path),
                env: None,
            })
        };
        let reports = walk_ident_reports(sys::asp_identification_file_get_report(ptr));
        out.push(IdentificationFile { file, reports });
        ptr = sys::asp_identification_file_next(ptr);
    }
    out
}

/// Convert a `*mut asp_report` linked list into `Vec<Report>`.
unsafe fn walk_reports(head: *mut sys::asp_report) -> Vec<Report> {
    let mut out = Vec::new();
    let mut ptr = head;
    while !ptr.is_null() {
        // line_content is documented as "must be freed with free()"
        let line_content_raw = sys::asp_report_get_line_content(ptr);
        let line_content = cstr_to_string_free(line_content_raw);

        out.push(Report {
            file:         cstr_to_string(sys::asp_report_get_file(ptr) as *const c_char),
            rule_id:      cstr_to_string(sys::asp_report_get_rule_id(ptr) as *const c_char),
            explanation:  cstr_to_string(sys::asp_report_get_explanation(ptr) as *const c_char),
            line_content,
            line:         sys::asp_report_get_line(ptr),
            column:       sys::asp_report_get_column(ptr),
            severity:     from_c_severity(sys::asp_report_get_severity(ptr)),
        });
        ptr = sys::asp_report_next(ptr);
    }
    out
}

// Symbol-location kind constants (match asp_analysis_result.h)
const SYM_LOC_PER_FILE:       u32 = 0; // ASP_SYMBOL_LOCATION_KIND_PER_FILE
const SYM_LOC_CONNECTED_AREAS: u32 = 1; // ASP_SYMBOL_LOCATION_KIND_CONNECTED_AREAS

// ─────────────────────────────────────────────────────────────────────────────
// Connection struct
// ─────────────────────────────────────────────────────────────────────────────

/// A connection to an ASP (Analysis Server Protocol) server.
///
/// Wraps the opaque `asp_connection*` C handle.  All network I/O, protocol
/// framing and JSON serialisation is performed by **libasp** itself — this
/// struct only converts between Rust and C types.
pub struct Connection {
    conn: *mut sys::asp_connection,
}

// SAFETY: asp_connection is not aliased from another thread while this struct
// is alive; we never hand out references to the raw pointer.
unsafe impl Send for Connection {}

impl Drop for Connection {
    fn drop(&mut self) {
        unsafe {
            sys::asp_connection_close(self.conn);
            sys::asp_connection_free(self.conn);
            sys::asp_deinitialize();
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

impl Connection {
    // ── Lifecycle ─────────────────────────────────────────────────────────

    /// Connect to an ASP server over TCP.
    pub fn connect(host: &str, port: u16) -> AspResult<Self> {
        debug!("ASP connecting (TCP) to {}:{}", host, port);
        unsafe {
            let rc = sys::asp_initialize();
            if rc != 0 {
                return Err(asp_error(rc));
            }

            let conn = sys::asp_connection_create();
            if conn.is_null() {
                sys::asp_deinitialize();
                return Err(AspError::NoMemory);
            }

            let addr = build_c_address_ip(host, port)?;
            // connection takes ownership of address (own_address = true)
            let rc = sys::asp_connection_connect(conn, addr, true);
            if rc != 0 {
                error!("asp_connection_connect failed: {}", rc);
                sys::asp_connection_free(conn);
                sys::asp_deinitialize();
                return Err(asp_error(rc));
            }

            debug!("ASP connected to {}:{}", host, port);
            Ok(Connection { conn })
        }
    }

    /// Connect to an ASP server via an NNG URL (e.g. `"tcp://127.0.0.1:5700"`).
    pub fn connect_nng(url: &str) -> AspResult<Self> {
        debug!("ASP connecting (NNG) to {}", url);
        unsafe {
            let rc = sys::asp_initialize();
            if rc != 0 {
                return Err(asp_error(rc));
            }

            let conn = sys::asp_connection_create();
            if conn.is_null() {
                sys::asp_deinitialize();
                return Err(AspError::NoMemory);
            }

            let addr = build_c_address_nng(url)?;
            let rc = sys::asp_connection_connect(conn, addr, true);
            if rc != 0 {
                error!("asp_connection_connect (NNG) failed: {}", rc);
                sys::asp_connection_free(conn);
                sys::asp_deinitialize();
                return Err(asp_error(rc));
            }

            debug!("ASP connected (NNG) to {}", url);
            Ok(Connection { conn })
        }
    }

    // ── Timeouts ──────────────────────────────────────────────────────────

    /// Set the receive timeout in milliseconds.
    pub fn set_recv_timeout(&mut self, ms: i32) {
        unsafe {
            sys::asp_connection_set_recv_timeout(self.conn, ms);
        }
    }

    /// Set the send timeout in milliseconds.
    pub fn set_send_timeout(&mut self, ms: i32) {
        unsafe {
            sys::asp_connection_set_send_timeout(self.conn, ms);
        }
    }

    // ── Workspace operations ──────────────────────────────────────────────

    /// Create a new workspace on the server.
    ///
    /// On success `workspace.id` is updated with the server-assigned ID.
    pub fn add_workspace(&mut self, workspace: &mut Workspace) -> AspResult<()> {
        unsafe {
            let c_ws = build_c_workspace(workspace)?;
            let rc = sys::asp_connection_add_workspace(self.conn, c_ws);
            if rc == 0 {
                workspace.id = sys::asp_workspace_get_id(c_ws);
                debug!("workspace created, id={}", workspace.id);
            }
            sys::asp_workspace_free(c_ws);
            if rc != 0 {
                return Err(asp_error(rc));
            }
            Ok(())
        }
    }

    /// Destroy a workspace on the server.
    pub fn destroy_workspace(&mut self, workspace: &Workspace) -> AspResult<()> {
        unsafe {
            let c_ws = build_c_workspace_id(workspace.id)?;
            let rc = sys::asp_connection_destroy_workspace(self.conn, c_ws);
            sys::asp_workspace_free(c_ws);
            if rc != 0 {
                return Err(asp_error(rc));
            }
            debug!("workspace {} destroyed", workspace.id);
            Ok(())
        }
    }

    /// Return a list of all workspaces currently known to the server.
    pub fn get_workspaces(&mut self) -> AspResult<Vec<WorkspaceOutline>> {
        unsafe {
            let mut head: *mut sys::asp_workspace_outline = std::ptr::null_mut();
            let rc = sys::asp_connection_get_workspaces(self.conn, &mut head);
            if rc != 0 {
                return Err(asp_error(rc));
            }

            let mut out = Vec::new();
            let mut ptr = head;
            while !ptr.is_null() {
                let id = sys::asp_workspace_outline_get_id(ptr);
                // asp_workspace_outline has a public `name` field; read it
                // directly since the C API has no dedicated getter.
                let name = cstr_to_string((*ptr).name as *const c_char);
                out.push(WorkspaceOutline { id, name });
                ptr = sys::asp_workspace_outline_next(ptr);
            }

            // Free the outline list (each node is freed by the recursive free)
            let mut ptr = head;
            while !ptr.is_null() {
                let next = sys::asp_workspace_outline_next(ptr);
                sys::asp_workspace_outline_free(ptr);
                ptr = next;
            }

            Ok(out)
        }
    }

    /// Register an additional source file inside an existing workspace.
    pub fn add_workspace_file(
        &mut self,
        workspace: &Workspace,
        file: &File,
    ) -> AspResult<()> {
        unsafe {
            let c_ws   = build_c_workspace_id(workspace.id)?;
            let c_file = build_c_file(file)?;
            let rc = sys::asp_connection_add_workspace_file(self.conn, c_ws, c_file);
            sys::asp_file_free(c_file);
            sys::asp_workspace_free(c_ws);
            if rc != 0 {
                return Err(asp_error(rc));
            }
            Ok(())
        }
    }

    /// Remove a source file from an existing workspace.
    pub fn remove_workspace_file(
        &mut self,
        workspace: &Workspace,
        file: &File,
    ) -> AspResult<()> {
        unsafe {
            let c_ws   = build_c_workspace_id(workspace.id)?;
            let c_file = build_c_file(file)?;
            let rc = sys::asp_connection_remove_workspace_file(self.conn, c_ws, c_file);
            sys::asp_file_free(c_file);
            sys::asp_workspace_free(c_ws);
            if rc != 0 {
                return Err(asp_error(rc));
            }
            Ok(())
        }
    }

    // ── Analysis operations ───────────────────────────────────────────────

    /// Start an analysis on the given workspace.
    ///
    /// On success `analysis.id` is updated with the server-assigned ID.
    pub fn start_analysis(
        &mut self,
        workspace: &Workspace,
        analysis: &mut Analysis,
    ) -> AspResult<()> {
        unsafe {
            let c_ws = build_c_workspace_id(workspace.id)?;
            let c_an = build_c_analysis(analysis)?;
            let rc = sys::asp_connection_start_analysis(self.conn, c_ws, c_an);
            if rc == 0 {
                analysis.id = sys::asp_analysis_get_id(c_an);
                debug!("analysis started, id={}", analysis.id);
            }
            sys::asp_analysis_free(c_an);
            sys::asp_workspace_free(c_ws);
            if rc != 0 {
                return Err(asp_error(rc));
            }
            Ok(())
        }
    }

    /// Request that the server stop a running analysis.
    pub fn stop_analysis(
        &mut self,
        workspace: &Workspace,
        analysis: &Analysis,
    ) -> AspResult<()> {
        unsafe {
            let c_ws = build_c_workspace_id(workspace.id)?;
            let c_an = build_c_analysis_id(analysis.id)?;
            let rc = sys::asp_connection_stop_analysis(self.conn, c_ws, c_an);
            sys::asp_analysis_free(c_an);
            sys::asp_workspace_free(c_ws);
            if rc != 0 {
                return Err(asp_error(rc));
            }
            Ok(())
        }
    }

    /// Release server resources associated with a completed or stopped analysis.
    pub fn destroy_analysis(
        &mut self,
        workspace: &Workspace,
        analysis: &Analysis,
    ) -> AspResult<()> {
        unsafe {
            let c_ws = build_c_workspace_id(workspace.id)?;
            let c_an = build_c_analysis_id(analysis.id)?;
            let rc = sys::asp_connection_destroy_analysis(self.conn, c_ws, c_an);
            sys::asp_analysis_free(c_an);
            sys::asp_workspace_free(c_ws);
            if rc != 0 {
                return Err(asp_error(rc));
            }
            debug!("analysis {} destroyed", analysis.id);
            Ok(())
        }
    }

    /// Retrieve the full result of a finished analysis.
    pub fn get_analysis_result(
        &mut self,
        workspace: &Workspace,
        analysis: &Analysis,
    ) -> AspResult<AnalysisResult> {
        unsafe {
            let c_ws     = build_c_workspace_id(workspace.id)?;
            let c_an     = build_c_analysis_id(analysis.id)?;
            let c_result = sys::asp_analysis_result_create();
            if c_result.is_null() {
                sys::asp_analysis_free(c_an);
                sys::asp_workspace_free(c_ws);
                return Err(AspError::NoMemory);
            }

            let rc = sys::asp_connection_get_analysis_result(
                self.conn, c_ws, c_an, c_result,
            );
            sys::asp_analysis_free(c_an);
            sys::asp_workspace_free(c_ws);

            if rc != 0 {
                sys::asp_analysis_result_free(c_result);
                return Err(asp_error(rc));
            }

            // ── Convert the C result tree into Rust types ─────────────────

            // Defect reports
            let reports = walk_reports(sys::asp_analysis_result_get_report(c_result));

            // SARIF raw text (struct field, no dedicated getter in public API)
            let sarif_text = cstr_to_string((*c_result).sarif_text as *const c_char);

            // Per-file symbol data
            let symbol_data = walk_symbol_locations(
                sys::asp_analysis_result_get_symloc(c_result, SYM_LOC_PER_FILE),
            );

            // Connected-area symbol data
            let connected_symbol_data = walk_symbol_locations(
                sys::asp_analysis_result_get_symloc(c_result, SYM_LOC_CONNECTED_AREAS),
            );

            // Call map
            let symbol_calls =
                walk_symbol_calls(sys::asp_analysis_result_get_symcall(c_result));

            // Language identification
            let identification_files = walk_ident_files(
                sys::asp_analysis_result_get_identification_file(c_result),
            );

            sys::asp_analysis_result_free(c_result);

            Ok(AnalysisResult {
                sarif_text,
                reports,
                symbol_data,
                connected_symbol_data,
                symbol_calls,
                identification_files,
            })
        }
    }

    /// Retrieve the current execution state (phase, progress) of an analysis.
    pub fn get_analysis_state(
        &mut self,
        workspace: &Workspace,
        analysis: &Analysis,
    ) -> AspResult<AnalysisState> {
        unsafe {
            let c_ws    = build_c_workspace_id(workspace.id)?;
            let c_an    = build_c_analysis_id(analysis.id)?;
            let c_state = sys::asp_analysis_state_create();
            if c_state.is_null() {
                sys::asp_analysis_free(c_an);
                sys::asp_workspace_free(c_ws);
                return Err(AspError::NoMemory);
            }

            let rc = sys::asp_connection_get_analysis_state(
                self.conn, c_ws, c_an, c_state,
            );
            sys::asp_analysis_free(c_an);
            sys::asp_workspace_free(c_ws);

            if rc != 0 {
                sys::asp_analysis_state_free(c_state);
                return Err(asp_error(rc));
            }

            // progress_entity is documented as "to be freed with free()"
            let entity_raw = sys::asp_analysis_state_get_progress_entity(c_state);
            let progress_entity =
                cstr_to_string_free(entity_raw).unwrap_or_default();

            let state = AnalysisState {
                current_job:     sys::asp_analysis_state_get_current_job(c_state),
                job_count:       sys::asp_analysis_state_get_job_count(c_state),
                phase:           from_c_phase(sys::asp_analysis_state_get_phase(c_state)),
                current:         sys::asp_analysis_state_get_current(c_state),
                total:           sys::asp_analysis_state_get_total(c_state),
                progress:        sys::asp_analysis_state_get_progress(c_state),
                progress_entity,
            };

            sys::asp_analysis_state_free(c_state);
            Ok(state)
        }
    }

    /// Return a list of all analyses currently known to the server.
    pub fn get_analyses(&mut self) -> AspResult<Vec<AnalysisOutline>> {
        unsafe {
            let mut head: *mut sys::asp_analysis_outline = std::ptr::null_mut();
            let rc = sys::asp_connection_get_analyses(self.conn, &mut head);
            if rc != 0 {
                return Err(asp_error(rc));
            }

            let mut out = Vec::new();
            let mut ptr = head;
            while !ptr.is_null() {
                out.push(AnalysisOutline {
                    id:           sys::asp_analysis_outline_get_id(ptr),
                    workspace_id: sys::asp_analysis_outline_get_workspace_id(ptr),
                });
                ptr = sys::asp_analysis_outline_next(ptr);
            }

            // Free the outline list
            let mut ptr = head;
            while !ptr.is_null() {
                let next = sys::asp_analysis_outline_next(ptr);
                sys::asp_analysis_outline_free(ptr);
                ptr = next;
            }

            Ok(out)
        }
    }

    /// Block the calling thread until the analysis reaches the *Finished*
    /// phase.  Delegates entirely to `asp_connection_wait_analysis` in libasp.
    pub fn wait_analysis(
        &mut self,
        _workspace: &Workspace,
        analysis: &Analysis,
    ) -> AspResult<()> {
        unsafe {
            // The C function only needs the analysis handle, not the workspace.
            let c_an = build_c_analysis_id(analysis.id)?;
            debug!("waiting for analysis {} to finish…", analysis.id);
            let rc = sys::asp_connection_wait_analysis(self.conn, c_an);
            sys::asp_analysis_free(c_an);
            if rc != 0 {
                return Err(asp_error(rc));
            }
            debug!("analysis {} finished", analysis.id);
            Ok(())
        }
    }
}