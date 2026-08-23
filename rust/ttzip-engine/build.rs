// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn compile_native_codecs(repo_root: &Path, out_dir: &Path, target: &str) {
    let fast_lzma2_dir = repo_root.join("Vendor/turbobench/fast-lzma2");
    let lzfse_dir = repo_root.join("Vendor/turbobench/lzfse/src");

    let mut obj_files = Vec::new();

    // Compile fast-lzma2 C files
    let fl2_sources = [
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
    let min_ver = env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "14.0".to_string());
    let min_ver_flag = format!("-mmacosx-version-min={}", min_ver);

    for src in fl2_sources {
        let src_path = fast_lzma2_dir.join(src);
        if !src_path.exists() {
            continue;
        }
        let obj_path = out_dir.join(format!("fl2_{}.o", src));
        let status = Command::new("clang")
            .args(["-O3", "-c", "-target", target, &min_ver_flag, "-I", fast_lzma2_dir.to_str().unwrap()])
            .arg(&src_path)
            .arg("-o")
            .arg(&obj_path)
            .status()
            .expect("Failed to compile fast-lzma2 source");
        assert!(status.success(), "Failed compiling {}", src);
        obj_files.push(obj_path);
    }

    // Compile lzfse C files
    let lzfse_sources = [
        "lzfse_decode.c",
        "lzfse_decode_base.c",
        "lzfse_encode.c",
        "lzfse_encode_base.c",
        "lzfse_fse.c",
        "lzvn_decode_base.c",
        "lzvn_encode_base.c",
    ];
    for src in lzfse_sources {
        let src_path = lzfse_dir.join(src);
        if !src_path.exists() {
            continue;
        }
        let obj_path = out_dir.join(format!("lzfse_{}.o", src));
        let status = Command::new("clang")
            .args(["-O3", "-c", "-target", target, &min_ver_flag, "-I", lzfse_dir.to_str().unwrap()])
            .arg(&src_path)
            .arg("-o")
            .arg(&obj_path)
            .status()
            .expect("Failed to compile lzfse source");
        assert!(status.success(), "Failed compiling {}", src);
        obj_files.push(obj_path);
    }

    // Compile libdeflate C files (adler32, crc32, gzip, zlib, deflate)
    let libdeflate_dir = repo_root.join("Vendor/libdeflate-upstream");
    if libdeflate_dir.exists() {
        let deflate_sources = [
            "lib/adler32.c",
            "lib/crc32.c",
            "lib/deflate_compress.c",
            "lib/deflate_decompress.c",
            "lib/gzip_compress.c",
            "lib/gzip_decompress.c",
            "lib/utils.c",
            "lib/zlib_compress.c",
            "lib/zlib_decompress.c",
            "lib/arm/cpu_features.c",
        ];
        for src in deflate_sources {
            let src_path = libdeflate_dir.join(src);
            if !src_path.exists() {
                continue;
            }
            let safe_name = src.replace('/', "_");
            let obj_path = out_dir.join(format!("deflate_{}.o", safe_name));
            let status = Command::new("clang")
                .args(["-O3", "-c", "-target", target, "-mmacosx-version-min=14.0", "-I", libdeflate_dir.join("lib").to_str().unwrap(), "-I", libdeflate_dir.to_str().unwrap()])
                .arg(&src_path)
                .arg("-o")
                .arg(&obj_path)
                .status()
                .expect("Failed to compile libdeflate source");
            assert!(status.success(), "Failed compiling {}", src);
            obj_files.push(obj_path);
        }
    }

    // Compile LZ4 C files
    let lz4_dir = repo_root.join("Vendor/lz4-upstream/lib");
    if lz4_dir.exists() {
        let lz4_sources = ["lz4.c", "lz4frame.c", "lz4hc.c"];
        for src in lz4_sources {
            let src_path = lz4_dir.join(src);
            if !src_path.exists() {
                continue;
            }
            let obj_path = out_dir.join(format!("lz4_{}.o", src));
            let status = Command::new("clang")
                .args(["-O3", "-c", "-target", target, "-mmacosx-version-min=14.0", "-I", lz4_dir.to_str().unwrap()])
                .arg(&src_path)
                .arg("-o")
                .arg(&obj_path)
                .status()
                .expect("Failed to compile lz4 source");
            assert!(status.success(), "Failed compiling {}", src);
            obj_files.push(obj_path);
        }
    }

    // Compile Zstandard C files
    let zstd_dir = repo_root.join("Vendor/zstd-upstream/lib");
    if zstd_dir.exists() {
        let zstd_sources = [
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
            "decompress/zstd_ddict.c",
            "decompress/zstd_decompress.c",
            "decompress/zstd_decompress_block.c",
        ];
        for src in zstd_sources {
            let src_path = zstd_dir.join(src);
            if !src_path.exists() {
                continue;
            }
            let safe_name = src.replace('/', "_");
            let obj_path = out_dir.join(format!("zstd_{}.o", safe_name));
            let status = Command::new("clang")
                .args([
                    "-O3",
                    "-c",
                    "-target",
                    target,
                    "-mmacosx-version-min=14.0",
                    "-I",
                    zstd_dir.to_str().unwrap(),
                    "-I",
                    zstd_dir.join("common").to_str().unwrap(),
                    "-DZSTD_MULTITHREAD",
                ])
                .arg(&src_path)
                .arg("-o")
                .arg(&obj_path)
                .status()
                .expect("Failed to compile zstd source");
            assert!(status.success(), "Failed compiling {}", src);
            obj_files.push(obj_path);
        }
    }

    if !obj_files.is_empty() {
        let lib_path = out_dir.join("libttzip_native_codecs.a");
        let mut libtool = Command::new("libtool");
        libtool.arg("-static").arg("-no_warning_for_no_symbols").arg("-o").arg(&lib_path);
        for obj in &obj_files {
            libtool.arg(obj);
        }
        let status = libtool.status().expect("Failed to run libtool");
        assert!(status.success(), "Failed creating libttzip_native_codecs.a");

        println!("cargo:rustc-link-search=native={}", out_dir.display());
        println!("cargo:rustc-link-lib=static=ttzip_native_codecs");
    }
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.clone());

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let target = env::var("TARGET").unwrap_or_else(|_| "aarch64-apple-darwin".to_string());

    let vendor_dir = repo_root.join("Vendor");
    let vendor_lib_dir = vendor_dir.join("lib");
    if vendor_lib_dir.exists() {
        println!("cargo:rustc-link-search=native={}", vendor_lib_dir.display());
        println!("cargo:rustc-link-lib=static=archive");
        println!("cargo:rustc-link-lib=static=deflate");
        println!("cargo:rustc-link-lib=static=zstd");
        println!("cargo:rustc-link-lib=static=lzma");
        println!("cargo:rustc-link-lib=static=lz4");
        println!("cargo:rustc-link-lib=static=uchardet");
        println!("cargo:rustc-link-lib=static=z");
        println!("cargo:rustc-link-lib=static=b2");
    }

    // Compile and link in-tree native codecs (fast-lzma2, lzfse, snappy)
    compile_native_codecs(&repo_root, &out_dir, &target);

    // System libraries & frameworks required on macOS
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        println!("cargo:rustc-link-lib=bz2");
        println!("cargo:rustc-link-lib=iconv");
        println!("cargo:rustc-link-lib=xml2");
        println!("cargo:rustc-link-lib=expat");
        println!("cargo:rustc-link-lib=c++");
        println!("cargo:rustc-link-lib=compression");
        println!("cargo:rustc-link-lib=framework=Security");
    }

    println!("cargo:rerun-if-changed=build.rs");
}

