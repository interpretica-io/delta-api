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

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    // Only do work when the asp_client feature is requested.
    if env::var("CARGO_FEATURE_ASP_CLIENT").is_err() {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // ── Locate libasp library directory ──────────────────────────────────
    let lib_dir = find_lib_dir(&manifest_dir);

    // ── Locate libasp public headers directory ────────────────────────────
    let include_dir = find_include_dir(&manifest_dir);

    // ── Tell Cargo where to find and link libasp ──────────────────────────
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=asp");

    // Publish the library directory as DEP_ASP_LIB_DIR so that downstream
    // build scripts (e.g. delta-cli/build.rs) can add the rpath to the
    // final binary without duplicating the search logic.
    println!("cargo:lib_dir={}", lib_dir.display());



    // ── Rebuild triggers ──────────────────────────────────────────────────
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=LIBASP_BUILD_DIR");
    println!("cargo:rerun-if-env-changed=ASP_SRC_DIR");
    println!("cargo:rerun-if-env-changed=LIBASP_INCLUDE_DIR");
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
// Library directory discovery
// ─────────────────────────────────────────────────────────────────────────────

fn find_lib_dir(manifest_dir: &Path) -> PathBuf {
    // 1. Explicit override: directory that already contains libasp.{dylib,so}
    if let Ok(dir) = env::var("LIBASP_BUILD_DIR") {
        let p = PathBuf::from(&dir);
        if has_libasp(&p) {
            return p;
        }
        let lib = p.join("lib");
        if has_libasp(&lib) {
            return lib;
        }
    }

    // 2. Locate the asp source tree, then search its build sub-directories.
    if let Some(asp_dir) = find_asp_src(manifest_dir) {
        let build_candidates = [
            "build-mac-arm64/lib",
            "build-macos-arm64/lib",
            "build-macos-x86/lib",
            "build-linux-arm64/lib",
            "build-linux-x86_64/lib",
            "build-win32/lib",
            "build/lib",
            "build_test/lib",
            // Flat build directories (some generators put the library here)
            "build-mac-arm64",
            "build-macos-arm64",
            "build-macos-x86",
            "build-linux-arm64",
            "build-linux-x86_64",
            "build-win32",
            "build",
            "build_test",
        ];
        for rel in &build_candidates {
            let candidate = asp_dir.join(rel);
            if has_libasp(&candidate) {
                return candidate;
            }
        }
    }

    panic!(
        "\n\
        ┌─────────────────────────────────────────────────────────────────────┐\n\
        │  Could not find libasp.{{dylib|so|dll}} anywhere.                   │\n\
        │                                                                     │\n\
        │  Option A – point directly to the built library directory:          │\n\
        │    export LIBASP_BUILD_DIR=/path/to/asp/build-mac-arm64/lib         │\n\
        │                                                                     │\n\
        │  Option B – build libasp first, then re-run cargo:                  │\n\
        │    cd /path/to/asp                                                  │\n\
        │    cmake -B build-mac-arm64 -G Ninja                               │\n\
        │    cmake --build build-mac-arm64                                   │\n\
        │                                                                     │\n\
        │  Option C – set ASP_SRC_DIR if asp lives outside the default paths: │\n\
        │    export ASP_SRC_DIR=/path/to/asp                                  │\n\
        └─────────────────────────────────────────────────────────────────────┘"
    );
}

fn has_libasp(dir: &Path) -> bool {
    dir.join("libasp.dylib").exists()
        || dir.join("libasp.so").exists()
        || dir.join("asp.dll").exists()
        || dir.join("libasp.dll").exists()
}

// ─────────────────────────────────────────────────────────────────────────────
// Public-header directory discovery
// ─────────────────────────────────────────────────────────────────────────────

fn find_include_dir(manifest_dir: &Path) -> PathBuf {
    // 1. Explicit override
    if let Ok(dir) = env::var("LIBASP_INCLUDE_DIR") {
        let p = PathBuf::from(dir);
        if has_asp_header(&p) {
            return p;
        }
    }

    // 2. Derive from asp source tree
    if let Some(asp_dir) = find_asp_src(manifest_dir) {
        let inc = asp_dir.join("lib/include/public");
        if has_asp_header(&inc) {
            return inc;
        }
    }

    panic!(
        "\n\
        Could not find libasp public headers (asp/asp.h).\n\
        Set LIBASP_INCLUDE_DIR to the directory that contains the asp/ folder,\n\
        or set ASP_SRC_DIR to the root of the asp source tree."
    );
}

fn has_asp_header(dir: &Path) -> bool {
    dir.join("asp/asp.h").exists()
}

// ─────────────────────────────────────────────────────────────────────────────
// asp source tree discovery
// ─────────────────────────────────────────────────────────────────────────────

/// Try to locate the root of the asp source tree, trying several well-known
/// relative paths from the delta-api crate manifest directory.
fn find_asp_src(manifest_dir: &Path) -> Option<PathBuf> {
    // 1. Explicit environment variable
    if let Ok(dir) = env::var("ASP_SRC_DIR") {
        let p = PathBuf::from(dir);
        if p.is_dir() {
            return Some(p);
        }
    }

    // 2. Relative paths — ordered from most to least likely.
    //    The project may be laid out in different ways depending on the
    //    developer's machine (sibling repo, sub-directory of a mono-repo, …).
    let candidates: &[&str] = &[
        // Sibling directory of delta-api's parent (most common monorepo layout)
        "../asp",
        // delta-api lives two levels deep (e.g. midair-platform/delta-api),
        // asp lives two levels deep in a different subtree (e.g. sa/asp)
        "../../sa/asp",
        "../../asp",
        "../sa/asp",
        // Three levels up
        "../../../asp",
        "../../../sa/asp",
    ];

    for rel in candidates {
        let p = manifest_dir.join(rel);
        // Use exists() + is_dir() instead of canonicalize() so we get a clear
        // false rather than a panic when the path doesn't exist.
        if p.exists() && p.is_dir() {
            // Best-effort canonicalize — fall back to the un-canonicalized path.
            return Some(p.canonicalize().unwrap_or(p));
        }
    }

    None
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