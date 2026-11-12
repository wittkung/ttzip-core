// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::env;
use std::path::{Path, PathBuf};

fn find_third_party_or_vendor(start_dir: &Path) -> PathBuf {
    let mut current = Some(start_dir);
    while let Some(dir) = current {
        let tp = dir.join("third_party");
        if tp.is_dir() {
            return tp;
        }
        let v = dir.join("vendor");
        if v.is_dir() {
            return v;
        }
        current = dir.parent();
    }
    start_dir.join("vendor")
}

const FL2_SOURCES: &[&str] = &[
    "dict_buffer.c",
    "fl2_common.c",
    "fl2_compress.c",
    "fl2_decompress.c",
    "fl2_pool.c",
    "fl2_threading.c",
    "lzma2_dec.c",
    "lzma2_enc.c",
    "radix_bitpack.c",
    "radix_mf.c",
    "radix_struct.c",
    "range_enc.c",
    "util.c",
    "xxhash.c",
];

const LZFSE_SOURCES: &[&str] = &[
    "lzfse_decode.c",
    "lzfse_decode_base.c",
    "lzfse_encode.c",
    "lzfse_encode_base.c",
    "lzfse_fse.c",
    "lzvn_decode_base.c",
    "lzvn_encode_base.c",
];

const DEFLATE_SOURCES: &[&str] = &[
    "lib/adler32.c",
    "lib/crc32.c",
    "lib/deflate_compress.c",
    "lib/deflate_decompress.c",
    "lib/gzip_compress.c",
    "lib/gzip_decompress.c",
    "lib/utils.c",
    "lib/zlib_compress.c",
    "lib/zlib_decompress.c",
];

const LZ4_SOURCES: &[&str] = &["lz4.c", "lz4frame.c", "lz4hc.c"];

const ZSTD_SOURCES: &[&str] = &[
    "common/debug.c",
    "common/entropy_common.c",
    "common/error_private.c",
    "common/fse_decompress.c",
    "common/pool.c",
    "common/threading.c",
    "common/xxhash.c",
    "common/zstd_common.c",
    "compress/fse_compress.c",
    "compress/hist.c",
    "compress/huf_compress.c",
    "compress/zstd_compress.c",
    "compress/zstd_compress_literals.c",
    "compress/zstd_compress_sequences.c",
    "compress/zstd_compress_superblock.c",
    "compress/zstd_double_fast.c",
    "compress/zstd_fast.c",
    "compress/zstd_lazy.c",
    "compress/zstd_ldm.c",
    "compress/zstd_opt.c",
    "compress/zstd_preSplit.c",
    "compress/zstdmt_compress.c",
    "decompress/huf_decompress.c",
    "decompress/huf_decompress_amd64.S",
    "decompress/zstd_ddict.c",
    "decompress/zstd_decompress.c",
    "decompress/zstd_decompress_block.c",
    "dictBuilder/cover.c",
    "dictBuilder/divsufsort.c",
    "dictBuilder/fastcover.c",
    "dictBuilder/zdict.c",
];

fn add_existing_sources(build: &mut cc::Build, dir: &Path, sources: &[&str]) {
    for src in sources {
        let path = dir.join(src);
        if path.exists() {
            build.file(path);
        }
    }
}

fn configure_libdeflate(build: &mut cc::Build, libdeflate_dir: &Path, target: &str) {
    if !libdeflate_dir.exists() {
        return;
    }
    add_existing_sources(build, libdeflate_dir, DEFLATE_SOURCES);
    if target.contains("aarch64") || target.contains("arm64") {
        add_existing_sources(build, libdeflate_dir, &["lib/arm/cpu_features.c"]);
    } else if target.contains("x86_64") {
        add_existing_sources(build, libdeflate_dir, &["lib/x86/cpu_features.c"]);
    }
    build.include(libdeflate_dir.join("lib"));
    build.include(libdeflate_dir);
}

fn configure_zstd(build: &mut cc::Build, zstd_dir: &Path) {
    if !zstd_dir.exists() {
        return;
    }
    add_existing_sources(build, zstd_dir, ZSTD_SOURCES);
    build.include(zstd_dir);
    build.include(zstd_dir.join("common"));
    build.include(zstd_dir.join("dictBuilder"));
    build.define("ZSTD_MULTITHREAD", None);
}

/// Compiles in-tree native C codecs (fast-lzma2, lzfse) for standalone Cargo builds.
/// In Bazel, build.rs is short-circuited by BAZEL_BUILD and Bzlmod handles all codecs hermetically.
fn compile_native_codecs(repo_root: &Path) {
    let top_vendor = find_third_party_or_vendor(repo_root);

    let fast_lzma2_dir = top_vendor.join("fast-lzma2");
    let lzfse_dir = if top_vendor.join("lzfse/src").exists() {
        top_vendor.join("lzfse/src")
    } else {
        top_vendor.join("lzfse")
    };
    let libdeflate_dir = top_vendor.join("libdeflate");
    let lz4_dir = top_vendor.join("lz4/lib");
    let zstd_dir = top_vendor.join("zstd/lib");

    let mut build = cc::Build::new();
    build.opt_level(3);
    build.warnings(false);
    let target = env::var("TARGET").unwrap_or_default();
    if target.contains("apple") {
        build.flag("-mmacosx-version-min=14.0");
    }

    if fast_lzma2_dir.exists() {
        add_existing_sources(&mut build, &fast_lzma2_dir, FL2_SOURCES);
        build.include(&fast_lzma2_dir);
    }

    if lzfse_dir.exists() {
        add_existing_sources(&mut build, &lzfse_dir, LZFSE_SOURCES);
        build.include(&lzfse_dir);
    }

    configure_libdeflate(&mut build, &libdeflate_dir, &target);

    if lz4_dir.exists() {
        add_existing_sources(&mut build, &lz4_dir, LZ4_SOURCES);
        build.include(&lz4_dir);
    }

    configure_zstd(&mut build, &zstd_dir);

    build.compile("ttzip_native_codecs");
}

fn is_bazel_build() -> bool {
    env::var("BAZEL_BUILD").is_ok()
        || env::var("CARGO_BAZEL").is_ok()
        || env::var("BUILD_WORKSPACE_DIRECTORY").is_ok()
}

fn configure_macos_link_search() {
    if let Ok(lib_dir) = env::var("TTZIP_SYS_LIB_DIR") {
        println!("cargo:rustc-link-search=native={lib_dir}");
    }
    for prefix in ["/opt/homebrew", "/usr/local"] {
        for sub in ["lib", "opt/libdeflate/lib"] {
            let path = Path::new(prefix).join(sub);
            if path.exists() {
                println!("cargo:rustc-link-search=native={}", path.display());
            }
        }
    }
    if let Ok(brew_prefix) = env::var("HOMEBREW_PREFIX") {
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
}

fn emit_macos_system_libs() {
    for lib in [
        "deflate",
        "zstd",
        "lz4",
        "archive",
        "bz2",
        "lzma",
        "iconv",
        "xml2",
        "expat",
        "c++",
        "compression",
    ] {
        println!("cargo:rustc-link-lib={lib}");
    }
    println!("cargo:rustc-link-lib=framework=Security");
}

fn main() {
    if is_bazel_build() {
        println!("cargo:rerun-if-changed=build.rs");
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.clone());
    let vendor_lib_dir = repo_root.join("Vendor").join("lib");
    if vendor_lib_dir.exists() {
        println!("cargo:rustc-link-search=native={}", vendor_lib_dir.display());
        for lib in ["archive", "deflate", "zstd", "lzma", "lz4", "uchardet", "z", "b2"] {
            println!("cargo:rustc-link-lib=static={lib}");
        }
    }

    compile_native_codecs(&repo_root);

    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "macos" {
        configure_macos_link_search();
        emit_macos_system_libs();
    }

    println!("cargo:rerun-if-changed=build.rs");
}

