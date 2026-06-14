//! Tauri build script for GosCertInspector.
//!
//! Three independent build paths, selected by Cargo features and env vars:
//!
//! 1. `--features rust-core`  -> pure-Rust TLS (rustls + x509-parser).
//!    No C/C++ toolchain or OpenSSL required.  This is the default for
//!    GitHub-hosted Android CI because no NDK-targeted OpenSSL is needed.
//!
//! 2. `--features mock-core`  -> in-process mock used by unit tests and
//!    desktop preview.  No native dependencies.
//!
//! 3. default (no feature)    -> links against the C++ core in `../cpp-core`,
//!    which is built via CMake and depends on OpenSSL 3.x as required by the
//!    technical specification (Section 4 - Technological Stack):
//!        `[Tauri UI] -> [Rust plugin] -> [C++ Core (OpenSSL)] -> HTTPS`
//!
//! The native (C++) path is only invoked when explicitly requested, so the
//! tree always compiles in `cargo test` / `cargo build` without OpenSSL
//! installed.

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    // Tauri's own build step (CSP, capabilities, icons, etc.) must always run.
    tauri_build::build();

    if should_skip_cpp_build() {
        return;
    }

    build_cpp_core();
}

/// Returns true when we must NOT invoke CMake on `../cpp-core`.
///
/// We skip in three cases:
///   * the user opted into the pure-Rust core (`--features rust-core`);
///   * the user opted into the mock core (`--features mock-core`);
///   * the user explicitly requested skipping with `GCI_SKIP_NATIVE=1`.
///
/// Cargo exposes selected features as `CARGO_FEATURE_<NAME>` env vars during
/// build script execution.
fn should_skip_cpp_build() -> bool {
    if env::var_os("CARGO_FEATURE_RUST_CORE").is_some() {
        println!("cargo:warning=gci-app: rust-core feature active - skipping C++/OpenSSL build");
        return true;
    }
    if env::var_os("CARGO_FEATURE_MOCK_CORE").is_some() {
        println!("cargo:warning=gci-app: mock-core feature active - skipping C++/OpenSSL build");
        return true;
    }
    if matches!(env::var("GCI_SKIP_NATIVE").as_deref(), Ok("1")) {
        println!("cargo:warning=gci-app: GCI_SKIP_NATIVE=1 - skipping C++/OpenSSL build");
        return true;
    }
    false
}

/// Builds the C++ core via CMake and emits the linker directives required to
/// statically link it together with OpenSSL into the Tauri binary/lib.
///
/// Expected layout (relative to `src-tauri/`):
///
///     ../cpp-core/CMakeLists.txt        # builds libgci_core.a + tests
///     ../cpp-core/include/inspector.h
///
/// OpenSSL is located via:
///   * `OPENSSL_ROOT_DIR` (set on Android/iOS CI to the static install dir),
///   * `OPENSSL_LIB_DIR`  (explicit lib path, takes priority for `-L`),
///   * pkg-config / system defaults on desktop.
fn build_cpp_core() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let cpp_core_dir = manifest_dir.join("..").join("cpp-core");
    let target = env::var("TARGET").unwrap_or_default();

    if !cpp_core_dir.exists() {
        println!(
            "cargo:warning=gci-app: cpp-core not found at {} - skipping native build",
            cpp_core_dir.display()
        );
        return;
    }

    // Re-run this script if any C++ source / header / env input changes.
    println!("cargo:rerun-if-changed={}", cpp_core_dir.display());
    println!("cargo:rerun-if-env-changed=OPENSSL_ROOT_DIR");
    println!("cargo:rerun-if-env-changed=OPENSSL_LIB_DIR");
    println!("cargo:rerun-if-env-changed=OPENSSL_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=GCI_SKIP_NATIVE");

    // ----- Resolve OpenSSL paths -------------------------------------------
    // Priority:
    //   1. Explicit env vars (OPENSSL_ROOT_DIR / *_LIB_DIR / *_INCLUDE_DIR).
    //   2. Auto-detect `third_party/openssl/install/<target-subdir>` next to
    //      the workspace root. This is the layout produced by
    //      scripts/build-openssl-android.sh and the iOS workflow.
    //
    // Auto-detection is essential for iOS because `cargo tauri ios build`
    // invokes `xcodebuild`, whose script phases do NOT reliably propagate
    // arbitrary environment variables from the calling shell to the inner
    // `cargo build`. Without this fallback, `find_package(OpenSSL)` fails
    // with "Could NOT find OpenSSL" on CI.
    let openssl_root = resolve_openssl_root(&target, &manifest_dir);

    // ----- CMake configuration ----------------------------------------------
    let mut cfg = cmake::Config::new(&cpp_core_dir);
    cfg.define("GCI_BUILD_TESTS", "OFF")
        .define("GCI_BUILD_SHARED", "OFF")
        .profile("Release");

    // Skip the `install` target. The default cmake-rs build target is
    // `install`, which on Linux/macOS resolves `CMAKE_INSTALL_LIBDIR` via
    // GNUInstallDirs to an absolute path (`/usr/local/lib`) under some
    // configurations, leading to "Permission denied" on CI. We only need the
    // static archive, which we pick up directly from the cmake build dir.
    cfg.build_target("gci_core_static");

    if let Some(root) = openssl_root.as_ref() {
        let root_str = root.to_string_lossy().into_owned();
        cfg.define("OPENSSL_ROOT_DIR", &root_str);

        let include_dir = env::var("OPENSSL_INCLUDE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| root.join("include"));
        cfg.define("OPENSSL_INCLUDE_DIR", include_dir.to_string_lossy().as_ref());

        let lib_dir = env::var("OPENSSL_LIB_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| root.join("lib"));
        cfg.define(
            "OPENSSL_CRYPTO_LIBRARY",
            lib_dir.join("libcrypto.a").to_string_lossy().as_ref(),
        );
        cfg.define(
            "OPENSSL_SSL_LIBRARY",
            lib_dir.join("libssl.a").to_string_lossy().as_ref(),
        );
    }

    // `Config::build()` with a custom build_target returns OUT_DIR; the
    // actual cmake build tree lives under `<OUT_DIR>/build`.
    let out_dir = cfg.build();
    let build_dir = out_dir.join("build");

    // ----- Link search paths ------------------------------------------------
    println!("cargo:rustc-link-search=native={}", build_dir.display());

    // Our static library (libgci_core.a is emitted at the root of build_dir).
    println!("cargo:rustc-link-lib=static=gci_core");

    // OpenSSL: prefer explicit env, then fall back to auto-detected root.
    if let Ok(dir) = env::var("OPENSSL_LIB_DIR") {
        println!("cargo:rustc-link-search=native={}", dir);
    } else if let Some(root) = openssl_root.as_ref() {
        println!("cargo:rustc-link-search=native={}/lib", root.display());
        println!("cargo:rustc-link-search=native={}/lib64", root.display());
    } else if let Ok(root) = env::var("OPENSSL_ROOT_DIR") {
        println!("cargo:rustc-link-search=native={}/lib", root);
        println!("cargo:rustc-link-search=native={}/lib64", root);
    }
    println!("cargo:rustc-link-lib=static=ssl");
    println!("cargo:rustc-link-lib=static=crypto");

    // C++ runtime - required because gci_core is a C++17 library.
    if target.contains("android") {
        println!("cargo:rustc-link-lib=dylib=c++_shared");
    } else if target.contains("apple") {
        println!("cargo:rustc-link-lib=dylib=c++");
    } else if target.contains("linux") {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }
}

/// Resolves the OpenSSL install root from env or by auto-detecting the
/// workspace `third_party/openssl/install/<target-subdir>` layout.
fn resolve_openssl_root(target: &str, manifest_dir: &Path) -> Option<PathBuf> {
    if let Ok(root) = env::var("OPENSSL_ROOT_DIR") {
        let p = PathBuf::from(root);
        if p.exists() {
            return Some(p);
        }
    }

    let subdir = match target {
        "aarch64-apple-ios" => "ios-arm64",
        "aarch64-apple-ios-sim" => "ios-simulator-arm64",
        "x86_64-apple-ios" => "ios-simulator-x86_64",
        "aarch64-linux-android" => "android-arm64",
        "armv7-linux-androideabi" => "android-arm",
        "x86_64-linux-android" => "android-x86_64",
        "i686-linux-android" => "android-x86",
        _ => return None,
    };

    let workspace_root = manifest_dir.parent()?;
    let candidate = workspace_root
        .join("third_party")
        .join("openssl")
        .join("install")
        .join(subdir);

    let libssl = candidate.join("lib").join("libssl.a");
    let libcrypto = candidate.join("lib").join("libcrypto.a");
    let header = candidate.join("include").join("openssl").join("ssl.h");

    if libssl.exists() && libcrypto.exists() && header.exists() {
        println!(
            "cargo:warning=gci-app: auto-detected OpenSSL at {}",
            candidate.display()
        );
        Some(candidate)
    } else {
        None
    }
}
