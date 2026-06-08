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

//! Build script. The native libasp this crate links against is built from the
//! pinned asp source: either an explicit checkout passed via `ASP_SRC_DIR` (e.g.
//! CI that cloned the private repo with its own credentials), or — by default —
//! a fresh clone of the pinned ref into a shared cache. Implicit sibling-directory
//! discovery is never used.
//!
//! Where the build runs:
//!   - Non-macOS targets (Linux, Windows): inside asp's own Docker image
//!     (`tools/build-env/Dockerfile_ubuntu_2404`), which carries asp's full
//!     toolchain, via its `run_make.sh`. The host needs no asp build tools.
//!   - macOS target: asp's `run_make.sh` on the host directly. A mac host
//!     already builds the mac dylib natively, and asp's Linux image cannot (it
//!     lacks osxcross + the macOS SDK), so Docker there would be both broken
//!     and pointless.
//!
//! Both paths run the same `run_make.sh`, which installs a self-contained
//! libasp (nng linked statically) into `<build>/fs/lib`.
//!
//! Overrides: `ASP_FORCE_DOCKER=1` forces the Docker path even on macOS (use an
//! image that carries a working macOS SDK via `ASP_DOCKER_IMAGE`).
//! `ASP_NATIVE_BUILD=1` forces the host build path everywhere (no Docker).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// asp's own build image and the Dockerfile that produces it (relative to the
/// asp source root). Override the image name/tag with `ASP_DOCKER_IMAGE`.
const ASP_DOCKER_IMAGE_DEFAULT: &str = "asp:ubuntu_2404";
const ASP_DOCKERFILE: &str = "tools/build-env/Dockerfile_ubuntu_2404";

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // ── Rebuild triggers ──────────────────────────────────────────────────
    println!("cargo:rerun-if-changed=build.rs");
    // The pinned asp URL/ref live in Cargo.toml metadata.
    println!("cargo:rerun-if-changed=Cargo.toml");
    for key in [
        "ASP_SRC_DIR",
        "ASP_GIT_URL",
        "ASP_GIT_REF",
        "ASP_CACHE_DIR",
        "ASP_DOCKER_IMAGE",
        "ASP_NATIVE_BUILD",
        "ASP_FORCE_DOCKER",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    // Feature toggles whether the native libasp is built/linked at all.
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_ASP_CLIENT");

    // The asp client (FFI to native libasp) is gated behind the `asp_client`
    // feature. With it off there is nothing native to build or link, so skip the
    // whole libasp build — this lets the crate compile for targets that cannot
    // link a native dylib (e.g. the wasm WebUI) using only the pure data models.
    if env::var_os("CARGO_FEATURE_ASP_CLIENT").is_none() {
        return;
    }

    // ── Obtain the asp source, build libasp for this target, locate it ────
    let (asp_url, asp_ref) = asp_source(&manifest_dir);
    let asp_dir = obtain_asp(&asp_url, &asp_ref);
    let (build_subdir, run_make_flag) = target_build();
    let lib_dir = build_libasp(&asp_dir, &build_subdir, run_make_flag);

    let include_dir = asp_dir.join("lib/include/public");
    assert!(
        has_asp_header(&include_dir),
        "public headers missing under {}",
        include_dir.display()
    );

    // ── Tell Cargo where to find and link libasp ──────────────────────────
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=asp");
    // Bake the library dir as an rpath so this crate's OWN artifacts (its tests,
    // benches and the delta-server bin) can load libasp at runtime. This does
    // not propagate to downstream crates — they use DEP_ASP_LIB_DIR below.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());

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
// asp source (URL + ref). The pin MUST be declared in Cargo.toml
// (`[package.metadata.asp]`); a missing entry is a hard error. Env vars
// `ASP_GIT_URL` / `ASP_GIT_REF` override the value at build time.
// ─────────────────────────────────────────────────────────────────────────────

fn asp_source(manifest_dir: &Path) -> (String, String) {
    let (meta_url, meta_ref) = read_asp_metadata(manifest_dir);
    let url = env::var("ASP_GIT_URL")
        .ok()
        .or(meta_url)
        .unwrap_or_else(|| {
            panic!(
                "[package.metadata.asp] git-url is missing from Cargo.toml (set it or ASP_GIT_URL)"
            )
        });
    let git_ref = env::var("ASP_GIT_REF")
        .ok()
        .or(meta_ref)
        .unwrap_or_else(|| {
            panic!(
                "[package.metadata.asp] git-ref is missing from Cargo.toml (set it or ASP_GIT_REF)"
            )
        });
    (url, git_ref)
}

/// Read `[package.metadata.asp] git-url / git-ref` from the crate's Cargo.toml.
fn read_asp_metadata(manifest_dir: &Path) -> (Option<String>, Option<String>) {
    let path = manifest_dir.join("Cargo.toml");
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return (None, None),
    };
    let value: toml::Value = match text.parse() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[asp-build] could not parse {}: {e}", path.display());
            return (None, None);
        }
    };
    let asp = value
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("asp"));
    let field = |name: &str| {
        asp.and_then(|a| a.get(name))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };
    (field("git-url"), field("git-ref"))
}

// ─────────────────────────────────────────────────────────────────────────────
// Obtain the asp source tree.
// ─────────────────────────────────────────────────────────────────────────────

/// Return the asp source directory. If `ASP_SRC_DIR` points at an asp checkout,
/// use it as-is — this lets a caller that already cloned asp (e.g. CI reusing
/// its own credentials for the private repo) hand it over instead of having this
/// script clone anonymously. Otherwise clone the pinned ref into a shared cache.
fn obtain_asp(url: &str, git_ref: &str) -> PathBuf {
    if let Some(dir) = env::var_os("ASP_SRC_DIR") {
        let p = PathBuf::from(&dir);
        if is_asp_source(&p) {
            println!(
                "cargo:warning=using asp source from ASP_SRC_DIR={}",
                p.display()
            );
            return p;
        }
        if !p.as_os_str().is_empty() {
            eprintln!(
                "[asp-build] ASP_SRC_DIR={} is not an asp checkout; falling back to clone",
                p.display()
            );
        }
    }
    clone_asp(url, git_ref)
}

/// A directory looks like an asp checkout if it carries the public headers.
fn is_asp_source(dir: &Path) -> bool {
    dir.join("lib/include/public/asp/asp.h").exists()
}

// ─────────────────────────────────────────────────────────────────────────────
// Clone the pinned asp into a shared cache.
// ─────────────────────────────────────────────────────────────────────────────

fn clone_asp(url: &str, git_ref: &str) -> PathBuf {
    let cache_root = cache_root();
    fs::create_dir_all(&cache_root).unwrap_or_else(|e| {
        panic!(
            "could not create asp cache dir {}: {e}",
            cache_root.display()
        )
    });

    // Serialize concurrent builds (multiple crates sharing one cache).
    let _lock = FileLock::acquire(&cache_root.join(".asp-build.lock"));

    let safe_ref: String = git_ref
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let src = cache_root.join(format!("asp-{safe_ref}"));

    if !src.join(".git").is_dir() {
        let _ = fs::remove_dir_all(&src);
        println!("cargo:warning=cloning asp {git_ref} from {url}");
        let src_str = src.to_str().expect("non-UTF8 cache path");

        // Fast path: shallow clone of a tag/branch.
        let shallow = run(
            "git",
            &[
                "clone", "--quiet", "--depth", "1", "--branch", git_ref, url, src_str,
            ],
        );
        if !shallow {
            // Fallback: full clone + checkout (handles raw commit SHAs).
            let _ = fs::remove_dir_all(&src);
            if !run("git", &["clone", "--quiet", url, src_str]) {
                panic!(
                    "git clone of asp ({url}) failed — is git installed and the network reachable?"
                );
            }
            if !run("git", &["-C", src_str, "checkout", "--quiet", git_ref]) {
                panic!("git checkout of asp ref '{git_ref}' failed");
            }
        }
    }

    src
}

// ─────────────────────────────────────────────────────────────────────────────
// Build libasp for the requested target (in asp's Docker image by default).
// ─────────────────────────────────────────────────────────────────────────────

/// Map the Cargo target to (asp build sub-directory, `run_make.sh` flag).
/// The sub-directory names match what `tools/run_make.sh` produces.
fn target_build() -> (String, Option<&'static str>) {
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    match (os.as_str(), arch.as_str()) {
        ("macos", "aarch64") => ("build-mac-arm64".to_string(), Some("--mac-arm64")),
        ("macos", _) => ("build-mac-x86_64".to_string(), Some("--mac-x86_64")),
        ("windows", _) => ("build-win32".to_string(), Some("--win32")),
        // Native (glibc) build for Linux and anything else.
        _ => ("build".to_string(), None),
    }
}

/// Build libasp if it is not already present, and return its directory.
///
/// We use asp's `run_make.sh`, which `ninja install`s into `<build>/fs`, so the
/// self-contained library (asp links nng statically, install rpath `$ORIGIN`/
/// `@loader_path`) lives in `<build>/fs/lib` — not the build tree's `<build>/lib`,
/// whose libasp references a non-co-located libnng and fails to load at runtime.
fn build_libasp(asp_dir: &Path, build_subdir: &str, run_make_flag: Option<&str>) -> PathBuf {
    let lib_dir = asp_dir.join(build_subdir).join("fs").join("lib");
    let _lock = FileLock::acquire(&cache_root().join(".asp-build.lock"));
    if has_libasp(&lib_dir) {
        return lib_dir;
    }

    // macOS builds natively on the host (the asp Linux image can't target mac);
    // everything else builds in asp's Docker image. ASP_NATIVE_BUILD forces the
    // host build everywhere; ASP_FORCE_DOCKER forces Docker even on macOS.
    let is_macos = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos");
    let force_native = env::var_os("ASP_NATIVE_BUILD").is_some();
    let force_docker = env::var_os("ASP_FORCE_DOCKER").is_some();

    if force_native || (is_macos && !force_docker) {
        build_native(asp_dir, run_make_flag);
    } else {
        build_in_docker(asp_dir, run_make_flag);
    }

    assert!(
        has_libasp(&lib_dir),
        "libasp was not produced in {}",
        lib_dir.display()
    );
    lib_dir
}

/// Build libasp inside asp's own Docker image via its `run_make.sh`.
fn build_in_docker(asp_dir: &Path, run_make_flag: Option<&str>) {
    let image = ensure_image(asp_dir);
    let asp_str = asp_dir.to_str().expect("non-UTF8 cache path");

    println!(
        "cargo:warning=building libasp in Docker image {image} ({})",
        run_make_flag.unwrap_or("native linux")
    );

    // Mount the cloned tree at /src and run asp's own build there. Artifacts
    // land back in the mounted tree (build*/lib/libasp.*).
    let mount = format!("{asp_str}:/src");
    let mut args: Vec<&str> = vec![
        "run",
        "--rm",
        "-v",
        &mount,
        "-w",
        "/src",
        &image,
        "./tools/run_make.sh",
    ];
    if let Some(flag) = run_make_flag {
        args.push(flag);
    }

    if !run("docker", &args) {
        panic!(
            "building libasp in Docker failed. Ensure Docker is running, or set \
             ASP_NATIVE_BUILD=1 to build on the host instead."
        );
    }
}

/// Ensure asp's build image exists; build it from asp's Dockerfile if it is the
/// default image and missing. A user-specified ASP_DOCKER_IMAGE must pre-exist.
fn ensure_image(asp_dir: &Path) -> String {
    let user_set = env::var("ASP_DOCKER_IMAGE").ok();
    let image = user_set
        .clone()
        .unwrap_or_else(|| ASP_DOCKER_IMAGE_DEFAULT.to_string());

    if run("docker", &["image", "inspect", &image]) {
        return image;
    }
    if user_set.is_some() {
        panic!("Docker image '{image}' (ASP_DOCKER_IMAGE) not found");
    }

    println!("cargo:warning=building asp Docker image {image} (first run; this is slow)");
    let asp_str = asp_dir.to_str().expect("non-UTF8 cache path");
    let dockerfile = asp_dir.join(ASP_DOCKERFILE);
    let dockerfile_str = dockerfile.to_str().expect("non-UTF8 path");
    if !run(
        "docker",
        &["build", "-t", &image, "-f", dockerfile_str, asp_str],
    ) {
        panic!("docker build of asp image failed");
    }
    image
}

/// Build libasp with asp's own `run_make.sh` directly on the host (no Docker) —
/// same canonical build as the Docker path, just without the container. Produces
/// the self-contained install tree under `<build>/fs`.
fn build_native(asp_dir: &Path, run_make_flag: Option<&str>) {
    println!(
        "cargo:warning=building libasp on the host ({})",
        run_make_flag.unwrap_or("native")
    );
    let mut args: Vec<&str> = Vec::new();
    if let Some(flag) = run_make_flag {
        args.push(flag);
    }
    if !run_in(asp_dir, "./tools/run_make.sh", &args) {
        panic!("asp run_make.sh failed — ensure cmake, ninja and a C/C++ compiler are installed.");
    }
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
    eprintln!("[asp-build] {cmd} {}", args.join(" "));
    match Command::new(cmd).args(args).status() {
        Ok(status) => status.success(),
        Err(e) => {
            eprintln!("[asp-build] failed to spawn `{cmd}`: {e}");
            false
        }
    }
}

/// Like `run`, but with an explicit working directory.
fn run_in(dir: &Path, cmd: &str, args: &[&str]) -> bool {
    eprintln!(
        "[asp-build] (cd {}) {cmd} {}",
        dir.display(),
        args.join(" ")
    );
    match Command::new(cmd).current_dir(dir).args(args).status() {
        Ok(status) => status.success(),
        Err(e) => {
            eprintln!("[asp-build] failed to spawn `{cmd}`: {e}");
            false
        }
    }
}

/// Best-effort inter-process lock via an exclusively-created file.
struct FileLock(PathBuf);

impl FileLock {
    fn acquire(path: &Path) -> FileLock {
        use std::io::ErrorKind;
        // Up to ~10 minutes, then assume the lock is stale and proceed.
        for _ in 0..1200 {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
            {
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
