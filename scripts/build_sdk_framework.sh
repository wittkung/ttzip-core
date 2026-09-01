#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# ==============================================================================
# scripts/build_sdk_framework.sh
# 自动化编译 TTZip 跨平台微内核并打包 Universal (arm64 + x86_64) XCFramework SDK
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
RUST_DIR="${REPO_ROOT}/rust"
FRAMEWORKS_DIR="${REPO_ROOT}/Frameworks"
XCFRAMEWORK_DIR="${FRAMEWORKS_DIR}/TTZipVendor.xcframework"
DIST_DIR="${REPO_ROOT}/dist"

FORCE_REBUILD=0
BUILD_MODE="release"
CARGO_FLAGS="--release"
VERSION="1.0.0"
OFFLINE_FLAG=""
BUILD_NATIVE_ONLY=0
NO_ZIP=""

if [ "${TTZIP_FAST_SDK:-0}" = "1" ]; then
    BUILD_NATIVE_ONLY=1
fi

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo "Options:"
    echo "  --release        Build in release mode with ThinLTO and -O3 (default)"
    echo "  --debug          Build in debug mode"
    echo "  --native         Fast path: build only host native architecture (e.g. arm64, skips zip by default)"
    echo "  --force, -f      Force full rebuild regardless of incremental fingerprint cache"
    echo "  --zip            Force creating .xcframework.zip archive even in native mode"
    echo "  --no-zip         Skip creating .xcframework.zip archive and sha256 checksum"
    echo "  --version <VER>  Set SDK version (default: 1.0.0)"
    echo "  --offline        Build offline without network access"
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
        --native)
            BUILD_NATIVE_ONLY=1
            shift
            ;;
        --force|-f)
            FORCE_REBUILD=1
            shift
            ;;
        --zip)
            NO_ZIP=0
            shift
            ;;
        --no-zip)
            NO_ZIP=1
            shift
            ;;
        --version)
            VERSION="$2"
            shift 2
            ;;
        --offline)
            OFFLINE_FLAG="--offline"
            shift
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

# Default NO_ZIP behavior: in native mode skip zip unless explicitly requested
if [ -z "${NO_ZIP}" ]; then
    if [ "${BUILD_NATIVE_ONLY}" = "1" ]; then
        NO_ZIP=1
    else
        NO_ZIP=0
    fi
fi

export PATH="$HOME/.cargo/bin:$PATH"
export MACOSX_DEPLOYMENT_TARGET="14.0"

if command -v sccache >/dev/null 2>&1; then
    export RUSTC_WRAPPER="sccache"
fi

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${RUST_DIR}/target}"
EFFECTIVE_TARGET_DIR="${CARGO_TARGET_DIR}"
HOST_ARCH="$(uname -m)"
if [ "${HOST_ARCH}" = "arm64" ]; then
    HOST_TARGET="aarch64-apple-darwin"
else
    HOST_TARGET="x86_64-apple-darwin"
fi

SLICE_DIR="${XCFRAMEWORK_DIR}/macos-arm64_x86_64"
FINGERPRINT_FILE="${EFFECTIVE_TARGET_DIR}/.sdk_fingerprint_${BUILD_MODE}_${BUILD_NATIVE_ONLY}"

# Helper function to copy only if content differs to preserve file mtime
copy_if_changed() {
    local src="$1"
    local dst="$2"
    if [ ! -f "${dst}" ] || ! cmp -s "${src}" "${dst}"; then
        mkdir -p "$(dirname "${dst}")"
        cp -f "${src}" "${dst}"
    fi
}

# ------------------------------------------------------------------------------
# 1. 计算源码与配置增量指纹 (Fast Incrementality Gate)
# ------------------------------------------------------------------------------
compute_fingerprint() {
    local git_tree dirty_diff top_vendor_diff rustc_ver script_hash
    git_tree="$(git -C "${REPO_ROOT}" rev-parse HEAD:rust 2>/dev/null || echo "no-git")"
    dirty_diff="$( (git -C "${REPO_ROOT}" diff HEAD -- rust vendor 2>/dev/null; git -C "${REPO_ROOT}" ls-files --others --exclude-standard rust vendor 2>/dev/null) | shasum -a 256 | awk '{print $1}')"
    top_vendor_diff="$( (git -C "${REPO_ROOT}/.." diff HEAD -- vendor 2>/dev/null; git -C "${REPO_ROOT}/.." ls-files --others --exclude-standard vendor 2>/dev/null) | shasum -a 256 | awk '{print $1}')"
    rustc_ver="$(rustc -Vv 2>/dev/null | shasum -a 256 | awk '{print $1}')"
    script_hash="$(shasum -a 256 "${BASH_SOURCE[0]}" "${REPO_ROOT}/scripts/postprocess_uniffi_swift.py" 2>/dev/null | shasum -a 256 | awk '{print $1}')"
    printf "%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s" \
        "${git_tree}" "${dirty_diff}" "${top_vendor_diff}" "${rustc_ver}" "${script_hash}" "${BUILD_MODE}" "${BUILD_NATIVE_ONLY}" "${VERSION}" "${NO_ZIP}" "${HOST_TARGET}" \
        | shasum -a 256 | awk '{print $1}'
}


CURRENT_FINGERPRINT="$(compute_fingerprint)"

check_artifacts_exist() {
    [ -f "${XCFRAMEWORK_DIR}/Info.plist" ] || return 1
    [ -f "${SLICE_DIR}/libTTZipVendor.a" ] || return 1
    [ -f "${SLICE_DIR}/Headers/ttzip_engineFFI.h" ] || return 1
    [ -f "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engine.swift" ] || return 1
    [ -f "${REPO_ROOT}/Sources/CTTZipBridge/include/ttzip_engineFFI.h" ] || return 1
    if [ "${NO_ZIP}" = "0" ]; then
        [ -f "${DIST_DIR}/TTZipVendor-v${VERSION}.xcframework.zip" ] || return 1
        [ -f "${DIST_DIR}/TTZipVendor-v${VERSION}.xcframework.zip.sha256" ] || return 1
    fi
    return 0
}

if [ "${FORCE_REBUILD}" = "0" ] && [ -f "${FINGERPRINT_FILE}" ]; then
    SAVED_FINGERPRINT="$(cat "${FINGERPRINT_FILE}" 2>/dev/null || true)"
    if [ "${SAVED_FINGERPRINT}" = "${CURRENT_FINGERPRINT}" ] && check_artifacts_exist; then
        echo "======================================================================"
        echo "⚡ [CACHE] TTZipCore SDK artifacts up-to-date (fingerprint: ${CURRENT_FINGERPRINT:0:12})"
        echo "   Target : ${XCFRAMEWORK_DIR}"
        echo "   Info   : Skipping rebuild. Use --force to rebuild unconditionally."
        echo "======================================================================"
        exit 0
    fi
fi

echo "======================================================================"
echo "📦 Building TTZipCore Standalone SDK (${BUILD_MODE} v${VERSION})"
echo "======================================================================"

mkdir -p "${DIST_DIR}" "${REPO_ROOT}/Sources/CTTZipBridge/include" "${REPO_ROOT}/Sources/TTZipCore/Generated"
mkdir -p "${SLICE_DIR}/Headers" "${XCFRAMEWORK_DIR}/macos-arm64/Headers"

if [ "${BUILD_NATIVE_ONLY}" = "1" ]; then
    echo "--> [INFO] Fast Path: Building native architecture only (${HOST_ARCH} ${BUILD_MODE})..."
    cargo build --manifest-path "${RUST_DIR}/Cargo.toml" --package ttzip-engine ${CARGO_FLAGS} ${OFFLINE_FLAG}
    
    TARGET_LIB="${EFFECTIVE_TARGET_DIR}/${BUILD_MODE}/libttzip_engine.a"
    if [ ! -f "${TARGET_LIB}" ]; then
        TARGET_LIB="${EFFECTIVE_TARGET_DIR}/${HOST_TARGET}/${BUILD_MODE}/libttzip_engine.a"
    fi
    CODEC_LIB="$(ls -t ${EFFECTIVE_TARGET_DIR}/${BUILD_MODE}/build/ttzip-engine-*/out/libttzip_native_codecs.a 2>/dev/null | head -n 1 || true)"
    if [ -z "${CODEC_LIB}" ]; then
        CODEC_LIB="$(find "${EFFECTIVE_TARGET_DIR}" -name "libttzip_native_codecs.a" -exec ls -t {} + 2>/dev/null | head -n 1 || true)"
    fi
    
    if [ -n "${CODEC_LIB}" ] && [ -f "${CODEC_LIB}" ]; then
        libtool -static -no_warning_for_no_symbols -o "${SLICE_DIR}/libTTZipVendor.a" "${TARGET_LIB}" "${CODEC_LIB}"
    else
        copy_if_changed "${TARGET_LIB}" "${SLICE_DIR}/libTTZipVendor.a"
    fi
    copy_if_changed "${SLICE_DIR}/libTTZipVendor.a" "${XCFRAMEWORK_DIR}/macos-arm64/libTTZipVendor.a"
else
    TARGETS=("aarch64-apple-darwin" "x86_64-apple-darwin")
    for target in "${TARGETS[@]}"; do
        if ! rustup target list --installed | grep -q "${target}"; then
            echo "--> [INFO] Adding rust target: ${target}..."
            rustup target add "${target}" || true
        fi
    done

    echo "--> [INFO] Building ttzip-engine for arm64 & x86_64 via unified Cargo Jobserver (${BUILD_MODE})..."
    cargo build --manifest-path "${RUST_DIR}/Cargo.toml" --package ttzip-engine \
        --target "aarch64-apple-darwin" \
        --target "x86_64-apple-darwin" \
        ${CARGO_FLAGS} ${OFFLINE_FLAG}


    BUILT_ARM64_LIB="${EFFECTIVE_TARGET_DIR}/aarch64-apple-darwin/${BUILD_MODE}/libttzip_engine.a"
    BUILT_X86_64_LIB="${EFFECTIVE_TARGET_DIR}/x86_64-apple-darwin/${BUILD_MODE}/libttzip_engine.a"

    ARM64_COMBINED="/tmp/libTTZip_arm64.a"
    X86_64_COMBINED="/tmp/libTTZip_x86_64.a"

    CODEC_LIB_ARM64="$(ls -t ${EFFECTIVE_TARGET_DIR}/aarch64-apple-darwin/${BUILD_MODE}/build/ttzip-engine-*/out/libttzip_native_codecs.a 2>/dev/null | head -n 1 || true)"
    if [ -z "${CODEC_LIB_ARM64}" ]; then
        CODEC_LIB_ARM64="$(find "${EFFECTIVE_TARGET_DIR}/aarch64-apple-darwin" -name "libttzip_native_codecs.a" -exec ls -t {} + 2>/dev/null | head -n 1 || true)"
    fi

    CODEC_LIB_X86_64="$(ls -t ${EFFECTIVE_TARGET_DIR}/x86_64-apple-darwin/${BUILD_MODE}/build/ttzip-engine-*/out/libttzip_native_codecs.a 2>/dev/null | head -n 1 || true)"
    if [ -z "${CODEC_LIB_X86_64}" ]; then
        CODEC_LIB_X86_64="$(find "${EFFECTIVE_TARGET_DIR}/x86_64-apple-darwin" -name "libttzip_native_codecs.a" -exec ls -t {} + 2>/dev/null | head -n 1 || true)"
    fi

    if [ -n "${CODEC_LIB_ARM64}" ] && [ -f "${CODEC_LIB_ARM64}" ]; then
        libtool -static -no_warning_for_no_symbols -o "${ARM64_COMBINED}" "${BUILT_ARM64_LIB}" "${CODEC_LIB_ARM64}"
    else
        copy_if_changed "${BUILT_ARM64_LIB}" "${ARM64_COMBINED}"
    fi

    if [ -n "${CODEC_LIB_X86_64}" ] && [ -f "${CODEC_LIB_X86_64}" ]; then
        libtool -static -no_warning_for_no_symbols -o "${X86_64_COMBINED}" "${BUILT_X86_64_LIB}" "${CODEC_LIB_X86_64}"
    else
        copy_if_changed "${BUILT_X86_64_LIB}" "${X86_64_COMBINED}"
    fi

    echo "--> [INFO] Combining arm64 + x86_64 via lipo into universal slice..."
    lipo -create "${ARM64_COMBINED}" "${X86_64_COMBINED}" -output "${SLICE_DIR}/libTTZipVendor.a"
    strip -S "${SLICE_DIR}/libTTZipVendor.a" 2>/dev/null || true
    copy_if_changed "${SLICE_DIR}/libTTZipVendor.a" "${XCFRAMEWORK_DIR}/macos-arm64/libTTZipVendor.a"
fi

# 5. 生成 UniFFI 绑定与 Scaffolding C 头文件（输出至临时目录并通过内容对比幂等写入）
echo "--> [INFO] Generating Mozilla UniFFI bindings..."
FIRST_DYLIB="${EFFECTIVE_TARGET_DIR}/${BUILD_MODE}/libttzip_engine.dylib"
if [ ! -f "${FIRST_DYLIB}" ]; then
    FIRST_DYLIB="${EFFECTIVE_TARGET_DIR}/${HOST_TARGET}/${BUILD_MODE}/libttzip_engine.dylib"
fi
if [ ! -f "${FIRST_DYLIB}" ]; then
    FIRST_DYLIB="${EFFECTIVE_TARGET_DIR}/aarch64-apple-darwin/${BUILD_MODE}/libttzip_engine.dylib"
fi

UNIFFI_BIN=""
for candidate in \
    "${EFFECTIVE_TARGET_DIR}/${HOST_TARGET}/${BUILD_MODE}/uniffi-bindgen" \
    "${EFFECTIVE_TARGET_DIR}/aarch64-apple-darwin/${BUILD_MODE}/uniffi-bindgen" \
    "${EFFECTIVE_TARGET_DIR}/release/uniffi-bindgen" \
    "${EFFECTIVE_TARGET_DIR}/debug/uniffi-bindgen" \
    "${RUST_DIR}/target/release/uniffi-bindgen" \
    "${RUST_DIR}/target/debug/uniffi-bindgen"; do
    if [ -x "${candidate}" ]; then
        UNIFFI_BIN="${candidate}"
        break
    fi
done

TMP_UNIFFI_DIR="$(mktemp -d /tmp/ttzip_uniffi.XXXXXX)"

if [ -n "${UNIFFI_BIN}" ]; then
    (
        cd "${RUST_DIR}"
        "${UNIFFI_BIN}" generate \
            --library "${FIRST_DYLIB}" \
            --language swift \
            --out-dir "${TMP_UNIFFI_DIR}" \
            --metadata-no-deps
    )
else
    (
        cd "${RUST_DIR}"
        cargo run ${OFFLINE_FLAG} --bin uniffi-bindgen --features full generate \
            --library "${FIRST_DYLIB}" \
            --language swift \
            --out-dir "${TMP_UNIFFI_DIR}" \
            --metadata-no-deps
    )
fi

# 执行 Swift 6 并发安全后处理
if [ -f "${TMP_UNIFFI_DIR}/ttzip_engine.swift" ]; then
    python3 "${REPO_ROOT}/scripts/postprocess_uniffi_swift.py" "${TMP_UNIFFI_DIR}/ttzip_engine.swift"
    copy_if_changed "${TMP_UNIFFI_DIR}/ttzip_engine.swift" "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engine.swift"
fi

# 幂等部署 C-Bridge 头文件（仅在变化时覆盖，保护下游 SwiftPM 编译缓存）
if [ -f "${TMP_UNIFFI_DIR}/ttzip_engineFFI.h" ]; then
    copy_if_changed "${TMP_UNIFFI_DIR}/ttzip_engineFFI.h" "${REPO_ROOT}/Sources/CTTZipBridge/include/ttzip_engineFFI.h"
    copy_if_changed "${TMP_UNIFFI_DIR}/ttzip_engineFFI.h" "${SLICE_DIR}/Headers/ttzip_engineFFI.h"
    copy_if_changed "${TMP_UNIFFI_DIR}/ttzip_engineFFI.h" "${XCFRAMEWORK_DIR}/macos-arm64/Headers/ttzip_engineFFI.h"
fi

rm -rf "${TMP_UNIFFI_DIR}"

# 6. 生成标准 XCFramework Info.plist（幂等写入）
TMP_PLIST="$(mktemp /tmp/ttzip_plist.XXXXXX)"
cat << 'EOF' > "${TMP_PLIST}"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>AvailableLibraries</key>
	<array>
		<dict>
			<key>HeadersPath</key>
			<string>Headers</string>
			<key>LibraryIdentifier</key>
			<string>macos-arm64_x86_64</string>
			<key>LibraryPath</key>
			<string>libTTZipVendor.a</string>
			<key>SupportedArchitectures</key>
			<array>
				<string>arm64</string>
				<string>x86_64</string>
			</array>
			<key>SupportedPlatform</key>
			<string>macos</string>
		</dict>
		<dict>
			<key>HeadersPath</key>
			<string>Headers</string>
			<key>LibraryIdentifier</key>
			<string>macos-arm64</string>
			<key>LibraryPath</key>
			<string>libTTZipVendor.a</string>
			<key>SupportedArchitectures</key>
			<array>
				<string>arm64</string>
			</array>
			<key>SupportedPlatform</key>
			<string>macos</string>
		</dict>
	</array>
	<key>CFBundlePackageType</key>
	<string>XFWK</string>
	<key>XCFrameworkFormatVersion</key>
	<string>1.0</string>
</dict>
</plist>
EOF
copy_if_changed "${TMP_PLIST}" "${XCFRAMEWORK_DIR}/Info.plist"
rm -f "${TMP_PLIST}"

# 7. 打包 XCFramework ZIP 并计算 SHA-256 校验和 (仅在非 --no-zip 模式下执行)
if [ "${NO_ZIP}" = "0" ]; then
    ZIP_NAME="TTZipVendor-v${VERSION}.xcframework.zip"
    ZIP_PATH="${DIST_DIR}/${ZIP_NAME}"
    mkdir -p "${DIST_DIR}"

    echo "--> [INFO] Creating ${ZIP_PATH}..."
    rm -f "${ZIP_PATH}"
    (
        cd "${FRAMEWORKS_DIR}"
        zip -qry "${ZIP_PATH}" "TTZipVendor.xcframework"
    )

    CHECKSUM="$(shasum -a 256 "${ZIP_PATH}" | awk '{print $1}')"
    echo "${CHECKSUM}" > "${DIST_DIR}/${ZIP_NAME}.sha256"

    # 落盘成功构建指纹
    echo "${CURRENT_FINGERPRINT}" > "${FINGERPRINT_FILE}"

    echo "======================================================================"
    echo "✅ [SUCCESS] TTZipCore Universal SDK built successfully!"
    echo "   Artifact : ${ZIP_PATH}"
    echo "   Size     : $(ls -lh "${ZIP_PATH}" | awk '{print $5}')"
    echo "   SHA-256  : ${CHECKSUM}"
    echo "   Slices   : $(lipo -info "${SLICE_DIR}/libTTZipVendor.a")"
    echo "======================================================================"
else
    # 落盘成功构建指纹
    echo "${CURRENT_FINGERPRINT}" > "${FINGERPRINT_FILE}"

    echo "======================================================================"
    echo "✅ [SUCCESS] TTZipCore SDK ready in ${XCFRAMEWORK_DIR}"
    echo "   Slices   : $(lipo -info "${SLICE_DIR}/libTTZipVendor.a")"
    echo "======================================================================"
fi

