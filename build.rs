/*
 * Delta API – build script
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

//! Build script: the native libasp the asp client links against is ALWAYS
//! obtained by cloning the pinned asp source and building it. A local/sibling
//! asp checkout is intentionally never used — builds are reproducible and do
//! not depend on whatever happens to sit next to the crate on disk.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Upstream asp repository. Overridable with `ASP_GIT_URL` / `ASP_GIT_REF`.
/// The ref is pinned to a release tag for reproducible builds; bump it together
/// with the FFI surface this crate consumes (bindings are regenerated from the
/// fetched headers, so a wrong ref surfaces as a compile error here, never a
/// silent ABI mismatch).
const ASP_GIT_URL_DEFAULT: &str = "https://github.com/interpretica-io/asp";
const ASP_GIT_REF_DEFAULT: &str = "v1.10.0";

/// Build directory name used inside the cloned tree.
const AUTOFETCH_BUILD_DIR: &str = "build-autofetch";

fn main() {
    // ── Rebuild triggers ──────────────────────────────────────────────────
    println!("cargo:rerun-if-changed=build.rs");
    for key in ["ASP_GIT_URL", "ASP_GIT_REF", "ASP_CACHE_DIR"] {
        println!("cargo:rerun-if-env-changed={key}");
    }

    // ── Always clone + build the pinned asp; locate its lib + headers ──────
    let asp_dir = clone_and_build_asp();
    let lib_dir = locate_lib(&asp_dir)
        .unwrap_or_else(|| panic!("no libasp produced under {}", asp_dir.display()));
    let include_dir = asp_dir.join("lib/include/public");
    assert!(
        has_asp_header(&include_dir),
        "public headers missing under {}",
        include_dir.display()
    );

    // ── Tell Cargo where to find and link libasp ──────────────────────────
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=asp");

    // Publish the library directory as DEP_ASP_LIB_DIR so that downstream
    // build scripts (e.g. delta-cli/build.rs) can add the rpath to the final
    // binary without duplicating the search logic.
    println!("cargo:lib_dir={}", lib_dir.display());

    println!(
        "cargo:rerun-if-changed={}",
        include_dir.join("asp/asp.h").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        include_dir.join("asp/asp_simple.h").display()
    );

    // ── Generate Rust FFI bindings ────────────────────────────────────────
    generate_bindings(&include_dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// Clone the pinned asp and build libasp into a shared cache.
// ─────────────────────────────────────────────────────────────────────────────

fn clone_and_build_asp() -> PathBuf {
    let url = env::var("ASP_GIT_URL").unwrap_or_else(|_| ASP_GIT_URL_DEFAULT.to_string());
    let git_ref = env::var("ASP_GIT_REF").unwrap_or_else(|_| ASP_GIT_REF_DEFAULT.to_string());

    let cache_root = cache_root();
    fs::create_dir_all(&cache_root)
        .unwrap_or_else(|e| panic!("could not create asp cache dir {}: {e}", cache_root.display()));

    // Serialize concurrent builds (multiple crates sharing one cache) with a
    // best-effort lock so they do not clone/build into the same tree at once.
    let _lock = FileLock::acquire(&cache_root.join(".asp-autofetch.lock"));

    let safe_ref: String = git_ref
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' })
        .collect();
    let src = cache_root.join(format!("asp-{safe_ref}"));

    // Clone if the tree is not present yet.
    if !src.join(".git").is_dir() {
        let _ = fs::remove_dir_all(&src);
        println!("cargo:warning=cloning asp {git_ref} from {url}");
        let src_str = src.to_str().expect("non-UTF8 cache path");

        // Fast path: shallow clone of a tag/branch.
        let shallow = run(
            "git",
            &["clone", "--quiet", "--depth", "1", "--branch", &git_ref, &url, src_str],
        );
        if !shallow {
            // Fallback: full clone + checkout (handles raw commit SHAs).
            let _ = fs::remove_dir_all(&src);
            if !run("git", &["clone", "--quiet", &url, src_str]) {
                panic!("git clone of asp ({url}) failed — is git installed and the network reachable?");
            }
            if !run("git", &["-C", src_str, "checkout", "--quiet", &git_ref]) {
                panic!("git checkout of asp ref '{git_ref}' failed");
            }
        }
    }

    // Build libasp (skip if a previous run already produced it).
    if locate_lib(&src).is_none() {
        let src_str = src.to_str().expect("non-UTF8 cache path");
        let build = src.join(AUTOFETCH_BUILD_DIR);
        let build_str = build.to_str().expect("non-UTF8 cache path");
        println!("cargo:warning=building libasp {git_ref} (first run; this can take a minute)");

        if !run(
            "cmake",
            &[
                "-S", src_str,
                "-B", build_str,
                "-DCMAKE_BUILD_TYPE=Release",
                "-DBUILD_SHARED_LIBS=ON",
                "-DBUILD_CLI=OFF",
            ],
        ) {
            panic!("cmake configure of asp failed — is cmake installed?");
        }
        if !run(
            "cmake",
            &["--build", build_str, "--config", "Release", "--target", "asp"],
        ) {
            panic!("cmake build of libasp failed");
        }
    }

    src
}

/// Directory inside the cloned tree that holds the built libasp.
fn locate_lib(asp_dir: &Path) -> Option<PathBuf> {
    [
        asp_dir.join(AUTOFETCH_BUILD_DIR).join("lib"),
        asp_dir.join(AUTOFETCH_BUILD_DIR),
    ]
    .into_iter()
    .find(|c| has_libasp(c))
}

fn has_libasp(dir: &Path) -> bool {
    dir.join("libasp.dylib").exists()
        || dir.join("libasp.so").exists()
        || dir.join("asp.dll").exists()
        || dir.join("libasp.dll").exists()
}

fn has_asp_header(dir: &Path) -> bool {
    dir.join("asp/asp.h").exists()
}

/// Root directory under which fetched asp trees are cached. Defaults to a
/// subfolder of CARGO_HOME so the build is done once and shared across crates;
/// overridable with ASP_CACHE_DIR.
fn cache_root() -> PathBuf {
    if let Some(dir) = env::var_os("ASP_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    if let Some(home) = env::var_os("CARGO_HOME") {
        return PathBuf::from(home).join("asp-cache");
    }
    PathBuf::from(env::var("OUT_DIR").unwrap()).join("asp-cache")
}

/// Run a command, streaming its output, and report whether it succeeded.
fn run(cmd: &str, args: &[&str]) -> bool {
    eprintln!("[asp-fetch] {cmd} {}", args.join(" "));
    match Command::new(cmd).args(args).status() {
        Ok(status) => status.success(),
        Err(e) => {
            eprintln!("[asp-fetch] failed to spawn `{cmd}`: {e}");
            false
        }
    }
}

/// Best-effort inter-process lock via an exclusively-created file.
struct FileLock(PathBuf);

impl FileLock {
    fn acquire(path: &Path) -> FileLock {
        use std::io::ErrorKind;
        // Up to ~5 minutes, then assume the lock is stale and proceed.
        for _ in 0..600 {
            match fs::OpenOptions::new().write(true).create_new(true).open(path) {
                Ok(_) => return FileLock(path.to_path_buf()),
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                    std::thread::sleep(Duration::from_millis(500));
                }
                Err(_) => return FileLock(path.to_path_buf()),
            }
        }
        FileLock(path.to_path_buf())
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Bindgen
// ─────────────────────────────────────────────────────────────────────────────

fn generate_bindings(include_dir: &Path) {
    // asp_simple.h is the super-header: it pulls in asp.h → asp_conn_api.h
    // and all supporting type headers, plus the simple one-shot API.
    let root_header = include_dir.join("asp/asp_simple.h");

    let bindings = bindgen::Builder::default()
        .header(root_header.to_str().expect("non-UTF8 header path"))
        .clang_arg(format!("-I{}", include_dir.display()))
        // Only emit symbols from the asp namespace.
        .allowlist_type("asp_.*")
        .allowlist_function("asp_.*")
        .allowlist_var("ASP_.*")
        // Derive common traits on generated structs.
        .derive_debug(true)
        .derive_copy(true)
        // Skip the layout-validation tests (they add boilerplate we don't need).
        .layout_tests(false)
        // Emit cargo:rerun-if-changed for every transitively included file.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("bindgen failed to generate libasp bindings");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_dir.join("libasp_bindings.rs"))
        .expect("Failed to write libasp_bindings.rs");
}
