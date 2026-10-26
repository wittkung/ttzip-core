// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo_root = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    println!("cargo:rerun-if-changed=csrc/ttzip_llama_bridge.c");
    println!("cargo:rerun-if-changed=csrc/ttzip_llama_bridge.h");

    let mut build = cc::Build::new();
    build
        .file("csrc/ttzip_llama_bridge.c")
        .include("csrc")
        .flag_if_supported("-std=c11")
        .warnings(false);

    // Prefer repository-local third_party/llama_cpp/include, fallback to enclosing monorepo third_party
    let local_llama_include = repo_root.join("third_party/llama_cpp/include");
    let monorepo_llama_include = repo_root
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .map(|p| p.join("third_party/llama_cpp/include"));

    if local_llama_include.exists() {
        println!(
            "cargo:rerun-if-changed={}",
            local_llama_include.join("llama.h").display()
        );
        build.include(&local_llama_include);
    } else if let Some(mono_inc) = monorepo_llama_include {
        if mono_inc.exists() {
            println!(
                "cargo:rerun-if-changed={}",
                mono_inc.join("llama.h").display()
            );
            build.include(&mono_inc);
        }
    }

    build.compile("ttzip_llama_bridge");
}
