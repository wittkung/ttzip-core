// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::env;
use std::path::{Path, PathBuf};

fn find_third_party_or_vendor(repo_root: &Path) -> PathBuf {
    println!("cargo:rerun-if-env-changed=TTZIP_THIRD_PARTY_DIR");
    let monorepo = repo_root.join("../../..");
    let default_directory = if monorepo.join("MODULE.bazel").is_file() {
        monorepo.join("third_party")
    } else {
        repo_root.join("third_party")
    };
    let directory = env::var_os("TTZIP_THIRD_PARTY_DIR")
        .map(PathBuf::from)
        .unwrap_or(default_directory);
    for library in ["fast-lzma2", "lzfse"] {
        assert!(
            directory.join(library).is_dir(),
            "Missing native source {library} in {}. Use a complete standalone checkout or set TTZIP_THIRD_PARTY_DIR.",
            directory.display()
        );
    }
    println!("cargo:rerun-if-changed={}", directory.display());
    directory
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

    // 1. fast-lzma2
    if fast_lzma2_dir.exists() {
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
        for src in fl2_sources {
            let path = fast_lzma2_dir.join(src);
            if path.exists() {
                build.file(path);
            }
        }
        build.include(&fast_lzma2_dir);
    }

    // 2. lzfse
    if lzfse_dir.exists() {
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
            let path = lzfse_dir.join(src);
            if path.exists() {
                build.file(path);
            }
        }
        build.include(&lzfse_dir);
    }

    // 3. libdeflate
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
        ];
        for src in deflate_sources {
            let path = libdeflate_dir.join(src);
            if path.exists() {
                build.file(path);
            }
        }
        if target.contains("aarch64") || target.contains("arm64") {
            let arm_cpu = libdeflate_dir.join("lib/arm/cpu_features.c");
            if arm_cpu.exists() {
                build.file(arm_cpu);
            }
        } else if target.contains("x86_64") {
            let x86_cpu = libdeflate_dir.join("lib/x86/cpu_features.c");
            if x86_cpu.exists() {
                build.file(x86_cpu);
            }
        }
        build.include(libdeflate_dir.join("lib"));
        build.include(&libdeflate_dir);
    }

    // 4. lz4
    if lz4_dir.exists() {
        let lz4_sources = ["lz4.c", "lz4frame.c", "lz4hc.c"];
        for src in lz4_sources {
            let path = lz4_dir.join(src);
            if path.exists() {
                build.file(path);
            }
        }
        build.include(&lz4_dir);
    }

    // 5. zstd
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
            "decompress/huf_decompress_amd64.S",
            "decompress/zstd_ddict.c",
            "decompress/zstd_decompress.c",
            "decompress/zstd_decompress_block.c",
            "dictBuilder/cover.c",
            "dictBuilder/divsufsort.c",
            "dictBuilder/fastcover.c",
            "dictBuilder/zdict.c",
        ];
        for src in zstd_sources {
            let path = zstd_dir.join(src);
            if path.exists() {
                build.file(path);
            }
        }
        build.include(&zstd_dir);
        build.include(zstd_dir.join("common"));
        build.include(zstd_dir.join("dictBuilder"));
        build.define("ZSTD_MULTITHREAD", None);
    }

    build.compile("ttzip_native_codecs");
}

fn main() {
    // In Bazel 8 (Bzlmod) hermetic builds, C codecs and native archives are managed
    // strictly via Bzlmod cc_library targets (@libdeflate, @lz4, @zstd, //third_party/*).
    if env::var("BAZEL_BUILD").is_ok()
        || env::var("CARGO_BAZEL").is_ok()
        || env::var("BUILD_WORKSPACE_DIRECTORY").is_ok()
    {
        println!("cargo:rerun-if-changed=build.rs");
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.clone());
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

    // Compile and link in-tree native codecs (fast-lzma2, lzfse, snappy, etc.)
    compile_native_codecs(&repo_root);

    // System libraries & frameworks required on macOS
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        if Path::new("/opt/homebrew/lib").exists() {
            println!("cargo:rustc-link-search=native=/opt/homebrew/lib");
        }
        if Path::new("/usr/local/lib").exists() {
            println!("cargo:rustc-link-search=native=/usr/local/lib");
        }
        println!("cargo:rustc-link-lib=deflate");
        println!("cargo:rustc-link-lib=zstd");
        println!("cargo:rustc-link-lib=lz4");
        println!("cargo:rustc-link-lib=archive");
        println!("cargo:rustc-link-lib=bz2");
        println!("cargo:rustc-link-lib=lzma");
        println!("cargo:rustc-link-lib=iconv");
        println!("cargo:rustc-link-lib=xml2");
        println!("cargo:rustc-link-lib=expat");
        println!("cargo:rustc-link-lib=c++");
        println!("cargo:rustc-link-lib=compression");
        println!("cargo:rustc-link-lib=framework=Security");
    }

    println!("cargo:rerun-if-changed=build.rs");
}

