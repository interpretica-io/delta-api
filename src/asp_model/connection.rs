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

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use log::{debug, error};
use serde_json::{json, Value};

use super::address::Address;
use super::analysis::{Analysis, AnalysisOutline, AnalysisState};
use super::enums::{
    AnalysisPhase, IdentificationStatus, Language, ReportSeverity, ResourceType, SymbolOrigin,
};
use super::env::Environment;
use super::file::File;
use super::identification::{IdentificationFile, IdentificationReport};
use super::report::Report;
use super::result::AnalysisResult;
use super::status::{AspError, AspResult};
use super::symbol::{Symbol, SymbolCall, SymbolLocation};
use super::workspace::{Workspace, WorkspaceOutline};

// ---------------------------------------------------------------------------
// Global request-ID counter (JSON-RPC 2.0 "id" field)
// ---------------------------------------------------------------------------

static REQUEST_ID: AtomicU32 = AtomicU32::new(1);

// ---------------------------------------------------------------------------
// Connection struct
// ---------------------------------------------------------------------------

/// TCP connection to an ASP (Analysis Server Protocol) server.
///
/// The wire format is newline-delimited JSON-RPC 2.0: each message is a
/// single JSON object followed by `\n`.
pub struct Connection {
    /// Write half — used exclusively for sending requests.
    stream: TcpStream,
    /// Buffered read half — a clone of `stream` wrapped in a `BufReader`.
    reader: BufReader<TcpStream>,
    /// Stored send-timeout value (milliseconds).
    send_timeout_ms: Option<u64>,
    /// Stored receive-timeout value (milliseconds).
    recv_timeout_ms: Option<u64>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

impl Connection {
    // ── Lifecycle ──────────────────────────────────────────────────────────

    /// Open a TCP connection to an ASP server.
    pub fn connect(host: &str, port: u16) -> AspResult<Self> {
        let addr = format!("{}:{}", host, port);
        debug!("ASP connecting to {}", addr);
        let stream = TcpStream::connect(&addr).map_err(|_| AspError::ConnectionFailed)?;
        let reader_stream = stream.try_clone().map_err(AspError::from)?;
        let reader = BufReader::new(reader_stream);
        Ok(Connection {
            stream,
            reader,
            send_timeout_ms: None,
            recv_timeout_ms: None,
        })
    }

    // ── Timeout configuration ──────────────────────────────────────────────

    /// Set the receive (read) timeout in milliseconds.
    pub fn set_recv_timeout(&mut self, ms: u64) {
        self.recv_timeout_ms = Some(ms);
        let timeout = Some(Duration::from_millis(ms));
        let _ = self.stream.set_read_timeout(timeout);
        // Also apply to the reader's inner clone so `read_line` honours it.
        let _ = self.reader.get_mut().set_read_timeout(timeout);
    }

    /// Set the send (write) timeout in milliseconds.
    pub fn set_send_timeout(&mut self, ms: u64) {
        self.send_timeout_ms = Some(ms);
        let _ = self.stream.set_write_timeout(Some(Duration::from_millis(ms)));
    }

    // ── Workspace operations ───────────────────────────────────────────────

    /// Create a new workspace on the server and populate `workspace.id`.
    pub fn add_workspace(&mut self, workspace: &mut Workspace) -> AspResult<()> {
        let mut params = json!({});
        if let Some(name) = &workspace.name {
            params["name"] = json!(name);
        }
        if let Some(env) = &workspace.env {
            params["environment"] = env_to_json(env);
        }
        if !workspace.files.is_empty() {
            params["file"] = files_to_json(&workspace.files);
        }
        let result = self.send_request("workspace_create", params)?;
        workspace.id = result["workspace_id"]
            .as_u64()
            .ok_or(AspError::MalformedResult)? as u32;
        debug!("workspace created, id={}", workspace.id);
        Ok(())
    }

    /// Destroy a workspace on the server.
    pub fn destroy_workspace(&mut self, workspace: &Workspace) -> AspResult<()> {
        let params = json!({ "workspace_id": workspace.id });
        self.send_request("workspace_destroy", params)?;
        debug!("workspace {} destroyed", workspace.id);
        Ok(())
    }

    /// Retrieve a list of all workspaces known to the server.
    pub fn get_workspaces(&mut self) -> AspResult<Vec<WorkspaceOutline>> {
        let result = self.send_request("workspace_list", json!({}))?;
        let mut list = Vec::new();
        if let Some(arr) = result["workspace"].as_array() {
            for v in arr {
                list.push(WorkspaceOutline {
                    id: v["id"].as_u64().unwrap_or(0) as u32,
                    name: v["name"].as_str().map(|s| s.to_string()),
                });
            }
        }
        Ok(list)
    }

    /// Add a single file to an existing workspace.
    pub fn add_workspace_file(&mut self, workspace: &Workspace, file: &File) -> AspResult<()> {
        let params = json!({
            "workspace_id": workspace.id,
            "file": [file_to_json(file)],
        });
        self.send_request("workspace_file_add", params)?;
        Ok(())
    }

    /// Remove a single file from an existing workspace.
    pub fn remove_workspace_file(&mut self, workspace: &Workspace, file: &File) -> AspResult<()> {
        let params = json!({
            "workspace_id": workspace.id,
            "file": [file_to_json(file)],
        });
        self.send_request("workspace_file_remove", params)?;
        Ok(())
    }

    // ── Analysis operations ────────────────────────────────────────────────

    /// Start an analysis and populate `analysis.id` with the server-assigned ID.
    pub fn start_analysis(
        &mut self,
        workspace: &Workspace,
        analysis: &mut Analysis,
    ) -> AspResult<()> {
        let jobs: Vec<Value> = analysis
            .jobs
            .iter()
            .map(|j| json!({ "kind": serde_json::to_value(&j.kind).unwrap_or(Value::Null) }))
            .collect();

        let params = json!({
            "workspace_id": workspace.id,
            "analysis_id":  analysis.id,
            "job":          jobs,
        });
        let result = self.send_request("analysis_start", params)?;
        analysis.id = result["analysis_id"]
            .as_u64()
            .ok_or(AspError::MalformedResult)? as u32;
        debug!("analysis started, id={}", analysis.id);
        Ok(())
    }

    /// Request that the server stop a running analysis.
    pub fn stop_analysis(&mut self, workspace: &Workspace, analysis: &Analysis) -> AspResult<()> {
        let params = json!({
            "workspace_id": workspace.id,
            "analysis_id":  analysis.id,
        });
        self.send_request("analysis_stop", params)?;
        Ok(())
    }

    /// Destroy a completed or stopped analysis on the server.
    pub fn destroy_analysis(
        &mut self,
        workspace: &Workspace,
        analysis: &Analysis,
    ) -> AspResult<()> {
        let params = json!({
            "workspace_id": workspace.id,
            "analysis_id":  analysis.id,
        });
        self.send_request("analysis_destroy", params)?;
        debug!("analysis {} destroyed", analysis.id);
        Ok(())
    }

    /// Retrieve the full result of a finished analysis.
    pub fn get_analysis_result(
        &mut self,
        workspace: &Workspace,
        analysis: &Analysis,
    ) -> AspResult<AnalysisResult> {
        let params = json!({
            "workspace_id": workspace.id,
            "analysis_id":  analysis.id,
        });
        let result = self.send_request("analysis_result_get", params)?;
        let data = &result["data"];

        // ── SARIF report ─────────────────────────────────────────────────
        let sarif_value = &data["report"];
        let sarif_text = if !sarif_value.is_null() {
            serde_json::to_string(sarif_value).ok()
        } else {
            None
        };

        let mut reports = Vec::new();
        if let Some(runs) = sarif_value["runs"].as_array() {
            for run in runs {
                if let Some(results) = run["results"].as_array() {
                    for r in results {
                        let rule_id =
                            r["ruleId"].as_str().map(|s| s.to_string());
                        let severity = ReportSeverity::from_sarif(
                            r["level"].as_str().unwrap_or(""),
                        );
                        let explanation =
                            r["message"]["text"].as_str().map(|s| s.to_string());

                        let mut file: Option<String> = None;
                        let mut line: u32 = 0;
                        let mut column: u32 = 0;

                        if let Some(locs) = r["locations"].as_array() {
                            if let Some(loc) = locs.first() {
                                let phys = &loc["physicalLocation"];
                                let uri = phys["artifactLocation"]["uri"]
                                    .as_str()
                                    .unwrap_or("");
                                // Strip the "file://" scheme prefix when present.
                                let path = uri
                                    .strip_prefix("file://")
                                    .unwrap_or(uri);
                                if !path.is_empty() {
                                    file = Some(path.to_string());
                                }
                                line = phys["region"]["startLine"]
                                    .as_u64()
                                    .unwrap_or(0) as u32;
                                column = phys["region"]["startColumn"]
                                    .as_u64()
                                    .unwrap_or(0) as u32;
                            }
                        }

                        reports.push(Report {
                            file,
                            rule_id,
                            explanation,
                            line_content: None,
                            line,
                            column,
                            severity,
                        });
                    }
                }
            }
        }

        // ── Symbol data ───────────────────────────────────────────────────
        let symbol_data = data["symbol_data"]
            .as_array()
            .map(|a| a.iter().map(parse_symbol_location).collect())
            .unwrap_or_default();

        let connected_symbol_data = data["connected_symbol_data"]
            .as_array()
            .map(|a| a.iter().map(parse_symbol_location).collect())
            .unwrap_or_default();

        let symbol_calls = data["symbol_calls"]
            .as_array()
            .map(|a| a.iter().map(parse_symbol_call).collect())
            .unwrap_or_default();

        // ── Identification ────────────────────────────────────────────────
        let identification_files = data["identification"]
            .as_array()
            .map(|a| a.iter().map(parse_identification_file).collect())
            .unwrap_or_default();

        Ok(AnalysisResult {
            sarif_text,
            reports,
            symbol_data,
            connected_symbol_data,
            symbol_calls,
            identification_files,
        })
    }

    /// Retrieve the current execution state of an analysis.
    pub fn get_analysis_state(
        &mut self,
        workspace: &Workspace,
        analysis: &Analysis,
    ) -> AspResult<AnalysisState> {
        let params = json!({
            "workspace_id": workspace.id,
            "analysis_id":  analysis.id,
        });
        let result = self.send_request("analysis_state_get", params)?;

        let phase_raw = result["phase"].as_u64().unwrap_or(0) as u32;
        let phase = AnalysisPhase::try_from(phase_raw).unwrap_or_default();

        Ok(AnalysisState {
            current_job:     result["current_job"].as_i64().unwrap_or(0) as i32,
            job_count:       result["job_count"].as_i64().unwrap_or(0) as i32,
            phase,
            current:         result["current"].as_i64().unwrap_or(0) as i32,
            total:           result["total"].as_i64().unwrap_or(0) as i32,
            progress:        result["progress"].as_i64().unwrap_or(0) as i32,
            progress_entity: result["progress_entity"]
                .as_str()
                .unwrap_or("")
                .to_string(),
        })
    }

    /// Retrieve a list of all analyses known to the server.
    pub fn get_analyses(&mut self) -> AspResult<Vec<AnalysisOutline>> {
        let result = self.send_request("analysis_list", json!({}))?;
        let mut list = Vec::new();
        if let Some(arr) = result["analysis"].as_array() {
            for v in arr {
                list.push(AnalysisOutline {
                    id:           v["id"].as_u64().unwrap_or(0) as u32,
                    workspace_id: v["workspace_id"].as_u64().unwrap_or(0) as u32,
                });
            }
        }
        Ok(list)
    }

    /// Block the calling thread until the analysis reaches
    /// [`AnalysisPhase::Finished`], polling every 500 ms.
    pub fn wait_analysis(
        &mut self,
        workspace: &Workspace,
        analysis: &Analysis,
    ) -> AspResult<()> {
        loop {
            let state = self.get_analysis_state(workspace, analysis)?;
            debug!(
                "analysis {} — phase={:?} progress={}%  [{}/{}]",
                analysis.id, state.phase, state.progress, state.current, state.total
            );
            if state.phase == AnalysisPhase::Finished {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(500));
        }
    }
}

// ---------------------------------------------------------------------------
// Private transport layer
// ---------------------------------------------------------------------------

impl Connection {
    /// Build and send a JSON-RPC 2.0 request, then read and return the
    /// `"result"` value from the response.
    ///
    /// Returns `AspError::Fail` when the server replies with an error object,
    /// and `AspError::MalformedResult` when the response cannot be parsed.
    fn send_request(&mut self, method: &str, params: Value) -> AspResult<Value> {
        let id = REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        let request = json!({
            "jsonrpc": "2.0",
            "method":  method,
            "params":  params,
            "id":      id,
        });

        let mut wire = serde_json::to_string(&request)?;
        wire.push('\n');
        debug!("ASP --> {}", wire.trim_end());
        self.stream.write_all(wire.as_bytes())?;

        let mut line = String::new();
        self.reader.read_line(&mut line)?;
        debug!("ASP <-- {}", line.trim_end());

        let response: Value = serde_json::from_str(&line)?;

        if let Some(err) = response.get("error") {
            let msg = err["message"].as_str().unwrap_or("(no message)");
            error!("ASP server error (method={}): {}", method, msg);
            return Err(AspError::Fail);
        }

        match response.get("result") {
            Some(v) => Ok(v.clone()),
            None => {
                error!("ASP response missing 'result' field for method={}", method);
                Err(AspError::MalformedResult)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// JSON serialisation helpers
// ---------------------------------------------------------------------------

/// Serialise an [`Environment`] into a JSON object suitable for the wire
/// protocol.
fn env_to_json(env: &Environment) -> Value {
    let mut obj = json!({});

    if let Some(c) = &env.compiler {
        obj["compiler"] = serde_json::to_value(c).unwrap_or(Value::Null);
    }
    if let Some(c) = &env.cpu {
        obj["cpu"] = serde_json::to_value(c).unwrap_or(Value::Null);
    }
    if let Some(l) = &env.language {
        obj["language"] = serde_json::to_value(l).unwrap_or(Value::Null);
    }
    if let Some(o) = &env.runtime_os {
        obj["runtime_os"] = serde_json::to_value(o).unwrap_or(Value::Null);
    }
    if let Some(wk) = &env.windows_kit {
        obj["windows_kit"] = json!(wk);
    }
    if let Some(vct) = &env.vctools {
        obj["vctools"] = json!(vct);
    }
    if let Some(loc) = &env.locale {
        obj["locale"] = json!(loc);
    }
    if env.override_higher_level {
        obj["override_higher_level"] = json!(1u32);
    }
    if !env.include_dirs.is_empty() {
        obj["file"] = Value::Array(
            env.include_dirs
                .iter()
                .map(|d| json!({ "address": address_to_json(&d.address) }))
                .collect(),
        );
    }

    obj
}

/// Serialise an [`Address`] into a JSON object.
fn address_to_json(addr: &Address) -> Value {
    let mut obj = json!({});
    if let Some(urn) = &addr.urn {
        obj["urn"] = json!(urn);
    }
    if let Some(port) = addr.port {
        obj["port"] = json!(port);
    }
    obj
}

/// Serialise a [`File`] into a JSON object.
fn file_to_json(file: &File) -> Value {
    let mut obj = json!({ "address": address_to_json(&file.address) });
    if let Some(env) = &file.env {
        obj["environment"] = env_to_json(env);
    }
    obj
}

/// Serialise a slice of [`File`]s into a JSON array.
fn files_to_json(files: &[File]) -> Value {
    Value::Array(files.iter().map(file_to_json).collect())
}

// ---------------------------------------------------------------------------
// JSON deserialisation helpers
// ---------------------------------------------------------------------------

/// Parse a single symbol object from a JSON value.
fn parse_symbol(v: &Value) -> Symbol {
    Symbol {
        namespace: v["namespace"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        name: v["name"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        resource_type: ResourceType::from_str(
            v["resource_type"].as_str().unwrap_or("0"),
        ),
        origin: SymbolOrigin::from_str(v["origin"].as_str().unwrap_or("")),
    }
}

/// Parse a symbol-location entry (a named area containing files and symbols).
///
/// Expected wire format:
/// ```json
/// {
///   "name":   "area_name",
///   "file":   [{ "path": "/path/to/file.c" }],
///   "symbol": [{ "name": "func", "namespace": "ns",
///                "origin": "from_source", "resource_type": "64" }]
/// }
/// ```
fn parse_symbol_location(v: &Value) -> SymbolLocation {
    let name = v["name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let files: Vec<File> = v["file"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|f| File {
                    address: Address::local_file(f["path"].as_str().unwrap_or("")),
                    env: None,
                })
                .collect()
        })
        .unwrap_or_default();

    let symbols: Vec<Symbol> = v["symbol"]
        .as_array()
        .map(|arr| arr.iter().map(parse_symbol).collect())
        .unwrap_or_default();

    SymbolLocation { name, files, symbols }
}

/// Parse a symbol-call entry (callers → callees mapping).
///
/// Expected wire format:
/// ```json
/// {
///   "caller": [{ "name": "func_a", ... }],
///   "callee": [{ "name": "func_b", ... }]
/// }
/// ```
fn parse_symbol_call(v: &Value) -> SymbolCall {
    let callers: Vec<Symbol> = v["caller"]
        .as_array()
        .map(|arr| arr.iter().map(parse_symbol).collect())
        .unwrap_or_default();

    let callees: Vec<Symbol> = v["callee"]
        .as_array()
        .map(|arr| arr.iter().map(parse_symbol).collect())
        .unwrap_or_default();

    SymbolCall { callers, callees }
}

/// Parse an identification-file entry.
///
/// Expected wire format:
/// ```json
/// {
///   "path":   "/path/to/file.c",
///   "report": [{ "language": "c", "status": 5 }]
/// }
/// ```
fn parse_identification_file(v: &Value) -> IdentificationFile {
    let path = v["path"].as_str().unwrap_or("");
    let file = if path.is_empty() {
        None
    } else {
        Some(File {
            address: Address::local_file(path),
            env: None,
        })
    };

    let reports: Vec<IdentificationReport> = v["report"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|r| {
                    let lang_str = r["language"].as_str().unwrap_or("unspecified");
                    // Deserialise language using the serde impl (handles "c",
                    // "cpp", "ruc", "unspecified", etc.)
                    let language: Language =
                        serde_json::from_value(json!(lang_str)).unwrap_or_default();
                    let status =
                        IdentificationStatus(r["status"].as_u64().unwrap_or(0) as u32);
                    IdentificationReport { language, status }
                })
                .collect()
        })
        .unwrap_or_default();

    IdentificationFile { file, reports }
}