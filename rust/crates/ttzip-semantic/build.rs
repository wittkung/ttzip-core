// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::env;
use std::path::{Path, PathBuf};

fn find_llama_cpp_root(start_dir: &Path) -> Option<PathBuf> {
    let mut current = Some(start_dir);
    while let Some(dir) = current {
        let candidate = dir.join("third_party/llama_cpp");
        if candidate.join("include").is_dir() {
            return Some(candidate);
        }
        current = dir.parent();
    }
    None
}

fn main() {
    if env::var("BAZEL_BUILD").is_ok()
        || env::var("CARGO_BAZEL").is_ok()
        || env::var("BUILD_WORKSPACE_DIRECTORY").is_ok()
    {
        println!("cargo:rerun-if-changed=build.rs");
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let c_src = manifest_dir.join("csrc/ttzip_llama_bridge.c");
    let c_hdr = manifest_dir.join("csrc/ttzip_llama_bridge.h");

    println!("cargo:rerun-if-changed={}", c_src.display());
    println!("cargo:rerun-if-changed={}", c_hdr.display());

    let mut build = cc::Build::new();
    build.file(&c_src);
    build.include(manifest_dir.join("csrc"));
    build.flag_if_supported("-std=c11");
    build.warnings(false);

    if let Some(llama_root) = find_llama_cpp_root(&manifest_dir) {
        build.include(llama_root.join("include"));
        let llama_lib = llama_root.join("lib");
        if llama_lib.is_dir() && env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "macos" {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", llama_lib.display());
        }
    }

    build.compile("ttzip_llama_bridge");
}
