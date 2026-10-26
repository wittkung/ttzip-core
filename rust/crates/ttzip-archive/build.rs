// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    if env::var("BAZEL_BUILD").is_ok() {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let _repo_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.clone());

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        // Resolve native library search path hermetically without hardcoded host paths
        if let Ok(lib_dir) = env::var("TTZIP_SYS_LIB_DIR") {
            println!("cargo:rustc-link-search=native={lib_dir}");
        } else if let Ok(brew_prefix) = env::var("HOMEBREW_PREFIX") {
            let p = Path::new(&brew_prefix).join("lib");
            if p.exists() {
                println!("cargo:rustc-link-search=native={}", p.display());
            }
        } else if let Ok(output) = std::process::Command::new("pkg-config")
            .args(["--variable=libdir", "libarchive"])
            .output()
        {
            if output.status.success() {
                let dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !dir.is_empty() && Path::new(&dir).exists() {
                    println!("cargo:rustc-link-search=native={dir}");
                }
            }
        }
        println!("cargo:rustc-link-lib=archive");
        println!("cargo:rustc-link-lib=iconv");
        println!("cargo:rustc-link-lib=c++");
        println!("cargo:rustc-link-lib=compression");
        println!("cargo:rustc-link-lib=framework=Security");
    }

    println!("cargo:rerun-if-changed=build.rs");
}
