#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# ==============================================================================
# scripts/build_rust.sh
# 自动化编译 TTZip Rust 引擎并生成 100% Mozilla UniFFI 绑定与 XCFramework
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
RUST_DIR="${REPO_ROOT}/rust"
VENDOR_DIR="${REPO_ROOT}/Vendor"
XCFRAMEWORK_MAC_DIR="${VENDOR_DIR}/TTZipVendor.xcframework/macos-arm64"

BUILD_MODE="release"
CARGO_FLAGS="--release"
BUILD_TARGET=""

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo "Options:"
    echo "  --release        Build in release mode with LTO and -O3 (default)"
    echo "  --debug          Build in debug mode"
    echo "  --target <TRGT>  Build specific target (e.g. aarch64-apple-darwin)"
    echo "  --help           Show this help message"
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --release)
            BUILD_MODE="release"
            CARGO_FLAGS="--release"
            shift
            ;;
        --debug)
            BUILD_MODE="debug"
            CARGO_FLAGS=""
            shift
            ;;
        --target)
            BUILD_TARGET="$2"
            shift 2
            ;;
        --help|-h)
            usage
            ;;
        *)
            echo "Unknown option: $1"
            usage
            ;;
    esac
done

echo "=========================================="
echo "📦 Building TTZip Rust Engine (${BUILD_MODE})"
echo "=========================================="

export PATH="$HOME/.cargo/bin:$PATH"
export MACOSX_DEPLOYMENT_TARGET="14.0"

mkdir -p "${XCFRAMEWORK_MAC_DIR}/Headers" "${REPO_ROOT}/Sources/CTTZipBridge/include"

# 1. 探测支持的编译目标架构
HOST_ARCH="$(uname -m)"
TARGETS=()

if [ -n "${BUILD_TARGET}" ]; then
    TARGETS+=("${BUILD_TARGET}")
else
    if [ "${HOST_ARCH}" = "arm64" ]; then
        TARGETS+=("aarch64-apple-darwin")
    else
        TARGETS+=("x86_64-apple-darwin")
    fi
fi

echo "--> Target architectures: ${TARGETS[*]}"

BUILT_LIBS=()
EFFECTIVE_TARGET_DIR="${CARGO_TARGET_DIR:-${RUST_DIR}/target}"

for target in "${TARGETS[@]}"; do
    echo "--> [INFO] Building ttzip-engine for ${target} (${BUILD_MODE})..."
    cargo build --manifest-path "${RUST_DIR}/Cargo.toml" --package ttzip-engine --target "${target}" ${CARGO_FLAGS}
    
    TARGET_LIB="${EFFECTIVE_TARGET_DIR}/${target}/${BUILD_MODE}/libttzip_engine.a"
    if [ -f "${TARGET_LIB}" ]; then
        BUILT_LIBS+=("${TARGET_LIB}")
    else
        echo "❌ Error: Expected static library not found at ${TARGET_LIB}"
        exit 1
    fi
done

# 2. 生成并部署静态库 libTTZipVendor.a 到 XCFramework
echo "--> [INFO] Packaging static library directly into ${XCFRAMEWORK_MAC_DIR}/libTTZipVendor.a..."
mkdir -p "${XCFRAMEWORK_MAC_DIR}"

NATIVE_CODECS_LIBS=()
NEWEST_CODEC_LIB="$(find "${EFFECTIVE_TARGET_DIR}" -name "libttzip_native_codecs.a" -exec ls -t {} + 2>/dev/null | head -n 1 || true)"
if [ -n "${NEWEST_CODEC_LIB}" ] && [ -f "${NEWEST_CODEC_LIB}" ]; then
    NATIVE_CODECS_LIBS+=("${NEWEST_CODEC_LIB}")
fi

if [ ${#NATIVE_CODECS_LIBS[@]} -gt 0 ]; then
    echo "--> Merging engine library with native codecs: ${NATIVE_CODECS_LIBS[*]}"
    libtool -static -no_warning_for_no_symbols -o "${XCFRAMEWORK_MAC_DIR}/libTTZipVendor.a" "${BUILT_LIBS[@]}" "${NATIVE_CODECS_LIBS[@]}"
elif [ ${#BUILT_LIBS[@]} -eq 1 ]; then
    cp "${BUILT_LIBS[0]}" "${XCFRAMEWORK_MAC_DIR}/libTTZipVendor.a"
else
    echo "--> Combining slices via lipo: ${BUILT_LIBS[*]}"
    lipo -create "${BUILT_LIBS[@]}" -output "${XCFRAMEWORK_MAC_DIR}/libTTZipVendor.a"
fi

# In-place strip DWARF debug info to optimize static library size
strip -S "${XCFRAMEWORK_MAC_DIR}/libTTZipVendor.a" 2>/dev/null || true

echo "    libTTZipVendor.a architecture: $(lipo -info "${XCFRAMEWORK_MAC_DIR}/libTTZipVendor.a")"
echo "    libTTZipVendor.a size: $(ls -lh "${XCFRAMEWORK_MAC_DIR}/libTTZipVendor.a" | awk '{print $5}')"

# 3. 生成 Mozilla UniFFI 绑定与 Scaffolding C 头文件
echo "--> [INFO] Generating Mozilla UniFFI bindings..."
mkdir -p "${REPO_ROOT}/Sources/TTZipCore/Generated"
FIRST_TARGET="${TARGETS[0]}"
FIRST_DYLIB="${EFFECTIVE_TARGET_DIR}/${FIRST_TARGET}/${BUILD_MODE}/libttzip_engine.dylib"
if [ ! -f "${FIRST_DYLIB}" ]; then
    FIRST_DYLIB="${EFFECTIVE_TARGET_DIR}/${BUILD_MODE}/libttzip_engine.dylib"
fi

if [ -f "${FIRST_DYLIB}" ]; then
    (
        cd "${RUST_DIR}"
        cargo run --bin uniffi-bindgen generate \
            --library "${FIRST_DYLIB}" \
            --language swift \
            --out-dir "${REPO_ROOT}/Sources/TTZipCore/Generated" \
            --metadata-no-deps

        mkdir -p "${REPO_ROOT}/python/ttzip"
        cargo run --bin uniffi-bindgen generate \
            --library "${FIRST_DYLIB}" \
            --language python \
            --out-dir "${REPO_ROOT}/python/ttzip" \
            --metadata-no-deps

        mkdir -p "${REPO_ROOT}/sdk/jvm/src/main/kotlin/com/ttzip"
        cargo run --bin uniffi-bindgen generate \
            --library "${FIRST_DYLIB}" \
            --language kotlin \
            --out-dir "${REPO_ROOT}/sdk/jvm/src/main/kotlin/com/ttzip" \
            --metadata-no-deps
    )
    
    # 执行 Swift 6 并发安全后处理
    if [ -f "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engine.swift" ]; then
        python3 "${REPO_ROOT}/scripts/postprocess_uniffi_swift.py" "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engine.swift"
    fi

    # 拷贝 Scaffolding 头文件
    if [ -f "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engineFFI.h" ]; then
        cp "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engineFFI.h" "${REPO_ROOT}/Sources/CTTZipBridge/include/"
        [ -d "${XCFRAMEWORK_MAC_DIR}/Headers" ] && cp "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engineFFI.h" "${XCFRAMEWORK_MAC_DIR}/Headers/"
    fi
fi

echo "=========================================="
echo "✅ [SUCCESS] Pure UniFFI engine & universal library generated successfully."
echo "=========================================="
