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
FRAMEWORKS_DIR="${REPO_ROOT}/Frameworks"
XCFRAMEWORK_DIR="${FRAMEWORKS_DIR}/TTZipVendor.xcframework"
XCFRAMEWORK_MAC_DIR="${XCFRAMEWORK_DIR}/macos-arm64"
SLICE_DIR="${XCFRAMEWORK_DIR}/macos-arm64_x86_64"

BUILD_MODE="release"
CARGO_FLAGS="--release"
BUILD_TARGET=""
OFFLINE_FLAG=""
SWIFT_ONLY=0
FORCE_REBUILD=0

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo "Options:"
    echo "  --release        Build in release mode with ThinLTO and -O3 (default)"
    echo "  --debug          Build in debug mode"
    echo "  --target <TRGT>  Build specific target (e.g. aarch64-apple-darwin)"
    echo "  --swift-only     Generate only Swift UniFFI bindings (skip Python/Kotlin)"
    echo "  --force, -f      Force full rebuild regardless of incremental fingerprint cache"
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
        --swift-only)
            SWIFT_ONLY=1
            shift
            ;;
        --force|-f)
            FORCE_REBUILD=1
            shift
            ;;
        --offline)
            OFFLINE_FLAG="--offline"
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

if command -v sccache >/dev/null 2>&1; then
    export RUSTC_WRAPPER="sccache"
fi

export PATH="$HOME/.cargo/bin:$PATH"
export MACOSX_DEPLOYMENT_TARGET="14.0"

EFFECTIVE_TARGET_DIR="${CARGO_TARGET_DIR:-${RUST_DIR}/target}"
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

TARGETS_KEY="$(echo "${TARGETS[*]}" | tr ' ' '_')"
FINGERPRINT_FILE="${EFFECTIVE_TARGET_DIR}/.rust_fingerprint_${BUILD_MODE}_swift${SWIFT_ONLY}_${TARGETS_KEY}"

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
# 1. 计算源码与构建环境 Merkle SHA-256 增量指纹 (Fast Incrementality Gate)
# ------------------------------------------------------------------------------
compute_fingerprint() {
    local git_tree dirty_diff top_vendor_diff rustc_ver script_hash
    git_tree="$(git -C "${REPO_ROOT}" rev-parse HEAD:rust 2>/dev/null || echo "no-git")"
    dirty_diff="$( { git -C "${REPO_ROOT}" diff HEAD -- rust vendor 2>/dev/null; git -C "${REPO_ROOT}" ls-files --others --exclude-standard rust vendor 2>/dev/null; } | shasum -a 256 | awk '{print $1}')"
    top_vendor_diff="$( { git -C "${REPO_ROOT}/.." diff HEAD -- vendor 2>/dev/null; git -C "${REPO_ROOT}/.." ls-files --others --exclude-standard vendor 2>/dev/null; } | shasum -a 256 | awk '{print $1}')"
    rustc_ver="$(rustc -Vv 2>/dev/null | shasum -a 256 | awk '{print $1}')"
    script_hash="$(shasum -a 256 "${BASH_SOURCE[0]}" "${REPO_ROOT}/scripts/postprocess_uniffi_swift.py" 2>/dev/null | shasum -a 256 | awk '{print $1}')"
    printf "%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s" \
        "${git_tree}" "${dirty_diff}" "${top_vendor_diff}" "${rustc_ver}" "${script_hash}" "${BUILD_MODE}" "${SWIFT_ONLY}" "${TARGETS_KEY}" "${OFFLINE_FLAG}" \
        | shasum -a 256 | awk '{print $1}'
}

CURRENT_FINGERPRINT="$(compute_fingerprint)"

check_artifacts_exist() {
    [ -f "${XCFRAMEWORK_DIR}/Info.plist" ] || return 1
    [ -f "${XCFRAMEWORK_MAC_DIR}/libTTZipVendor.a" ] || return 1
    [ -f "${XCFRAMEWORK_MAC_DIR}/Headers/ttzip_engineFFI.h" ] || return 1
    [ -f "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engine.swift" ] || return 1
    [ -f "${REPO_ROOT}/Sources/CTTZipBridge/include/ttzip_engineFFI.h" ] || return 1
    if [ "${SWIFT_ONLY}" = "0" ]; then
        [ -f "${REPO_ROOT}/sdk/python/ttzip/ttzip_engine.py" ] || return 1
        [ -f "${REPO_ROOT}/sdk/jvm/src/main/kotlin/com/ttzip/ttzip_engine.kt" ] || return 1
    fi
    return 0
}

# ------------------------------------------------------------------------------
# 2. 增量短路守卫判断
# ------------------------------------------------------------------------------
if [ "${FORCE_REBUILD}" = "0" ] && [ -f "${FINGERPRINT_FILE}" ]; then
    SAVED_FINGERPRINT="$(cat "${FINGERPRINT_FILE}" 2>/dev/null || true)"
    if [ "${SAVED_FINGERPRINT}" = "${CURRENT_FINGERPRINT}" ] && check_artifacts_exist; then
        echo "======================================================================"
        echo "⚡ [CACHE] TTZip Rust Engine artifacts up-to-date (fingerprint: ${CURRENT_FINGERPRINT:0:12})"
        echo "   Target : ${XCFRAMEWORK_MAC_DIR}"
        echo "   Info   : Skipping rebuild. Use --force to rebuild unconditionally."
        echo "======================================================================"
        exit 0
    fi
fi

echo "=========================================="
echo "📦 Building TTZip Rust Engine (${BUILD_MODE})"
echo "=========================================="

mkdir -p "${XCFRAMEWORK_MAC_DIR}/Headers" "${SLICE_DIR}/Headers" "${REPO_ROOT}/Sources/CTTZipBridge/include" "${REPO_ROOT}/Sources/TTZipCore/Generated"

echo "--> Target architectures: ${TARGETS[*]}"

BUILT_LIBS=()
for target in "${TARGETS[@]}"; do
    echo "--> [INFO] Building ttzip-engine for ${target} (${BUILD_MODE})..."
    cargo build --manifest-path "${RUST_DIR}/Cargo.toml" --package ttzip-engine --features full --target "${target}" ${CARGO_FLAGS} ${OFFLINE_FLAG}
    
    TARGET_LIB="${EFFECTIVE_TARGET_DIR}/${target}/${BUILD_MODE}/libttzip_engine.a"
    if [ -f "${TARGET_LIB}" ]; then
        BUILT_LIBS+=("${TARGET_LIB}")
    else
        echo "❌ Error: Expected static library not found at ${TARGET_LIB}"
        exit 1
    fi
done

# ------------------------------------------------------------------------------
# 3. 生成并幂等部署静态库 libTTZipVendor.a
# ------------------------------------------------------------------------------
echo "--> [INFO] Packaging static library into ${XCFRAMEWORK_MAC_DIR}/libTTZipVendor.a..."
mkdir -p "${XCFRAMEWORK_MAC_DIR}" "${SLICE_DIR}"

NATIVE_CODECS_LIBS=()
for target in "${TARGETS[@]}"; do
    CODEC_LIB="$(ls -t ${EFFECTIVE_TARGET_DIR}/${target}/${BUILD_MODE}/build/ttzip-engine-*/out/libttzip_native_codecs.a 2>/dev/null | head -n 1 || true)"
    if [ -z "${CODEC_LIB}" ]; then
        CODEC_LIB="$(find "${EFFECTIVE_TARGET_DIR}/${target}" -name "libttzip_native_codecs.a" -exec ls -t {} + 2>/dev/null | head -n 1 || true)"
    fi
    if [ -n "${CODEC_LIB}" ] && [ -f "${CODEC_LIB}" ]; then
        NATIVE_CODECS_LIBS+=("${CODEC_LIB}")
        break
    fi
done

TMP_LIB="$(mktemp /tmp/libTTZipVendor_XXXXXX)"
if [ ${#NATIVE_CODECS_LIBS[@]} -gt 0 ]; then
    echo "--> Merging engine library with native codecs: ${NATIVE_CODECS_LIBS[*]}"
    libtool -static -no_warning_for_no_symbols -o "${TMP_LIB}" "${BUILT_LIBS[@]}" "${NATIVE_CODECS_LIBS[@]}"
elif [ ${#BUILT_LIBS[@]} -eq 1 ]; then
    cp "${BUILT_LIBS[0]}" "${TMP_LIB}"
else
    echo "--> Combining slices via lipo: ${BUILT_LIBS[*]}"
    lipo -create "${BUILT_LIBS[@]}" -output "${TMP_LIB}"
fi

strip -x "${TMP_LIB}" 2>/dev/null || true
copy_if_changed "${TMP_LIB}" "${XCFRAMEWORK_MAC_DIR}/libTTZipVendor.a"
copy_if_changed "${TMP_LIB}" "${SLICE_DIR}/libTTZipVendor.a"
rm -f "${TMP_LIB}"

# ------------------------------------------------------------------------------
# 4. 幂等生成 XCFramework 必需的 Info.plist
# ------------------------------------------------------------------------------
TMP_PLIST="$(mktemp /tmp/ttzip_plist_XXXXXX)"
cat > "${TMP_PLIST}" << 'EOF'
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

# ------------------------------------------------------------------------------
# 5. 在临时沙盒中生成 Mozilla UniFFI 绑定并幂等同步
# ------------------------------------------------------------------------------
echo "--> [INFO] Generating Mozilla UniFFI bindings..."
FIRST_TARGET="${TARGETS[0]}"
FIRST_DYLIB="${EFFECTIVE_TARGET_DIR}/${FIRST_TARGET}/${BUILD_MODE}/libttzip_engine.dylib"
if [ ! -f "${FIRST_DYLIB}" ]; then
    FIRST_DYLIB="${EFFECTIVE_TARGET_DIR}/${BUILD_MODE}/libttzip_engine.dylib"
fi

if [ -f "${FIRST_DYLIB}" ]; then
    UNIFFI_BIN=""
    for candidate in \
        "${EFFECTIVE_TARGET_DIR}/${FIRST_TARGET}/${BUILD_MODE}/uniffi-bindgen" \
        "${EFFECTIVE_TARGET_DIR}/${BUILD_MODE}/uniffi-bindgen" \
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
                --no-format \
                --metadata-no-deps

            if [ "${SWIFT_ONLY}" = "0" ]; then
                mkdir -p "${REPO_ROOT}/sdk/python/ttzip"
                "${UNIFFI_BIN}" generate \
                    --library "${FIRST_DYLIB}" \
                    --language python \
                    --out-dir "${REPO_ROOT}/sdk/python/ttzip" \
                    --no-format \
                    --metadata-no-deps

                mkdir -p "${REPO_ROOT}/sdk/jvm/src/main/kotlin/com/ttzip"
                "${UNIFFI_BIN}" generate \
                    --library "${FIRST_DYLIB}" \
                    --language kotlin \
                    --out-dir "${REPO_ROOT}/sdk/jvm/src/main/kotlin/com/ttzip" \
                    --no-format \
                    --metadata-no-deps
            fi
        )
    else
        (
            cd "${RUST_DIR}"
            cargo run ${OFFLINE_FLAG} --bin uniffi-bindgen --features full generate \
                --library "${FIRST_DYLIB}" \
                --language swift \
                --out-dir "${TMP_UNIFFI_DIR}" \
                --no-format \
                --metadata-no-deps

            if [ "${SWIFT_ONLY}" = "0" ]; then
                mkdir -p "${REPO_ROOT}/sdk/python/ttzip"
                cargo run ${OFFLINE_FLAG} --bin uniffi-bindgen --features full generate \
                    --library "${FIRST_DYLIB}" \
                    --language python \
                    --out-dir "${REPO_ROOT}/sdk/python/ttzip" \
                    --metadata-no-deps

                mkdir -p "${REPO_ROOT}/sdk/jvm/src/main/kotlin/com/ttzip"
                cargo run ${OFFLINE_FLAG} --bin uniffi-bindgen --features full generate \
                    --library "${FIRST_DYLIB}" \
                    --language kotlin \
                    --out-dir "${REPO_ROOT}/sdk/jvm/src/main/kotlin/com/ttzip" \
                    --metadata-no-deps
            fi
        )
    fi
    
    # 执行 Swift 6 并发安全后处理
    if [ -f "${TMP_UNIFFI_DIR}/ttzip_engine.swift" ]; then
        python3 "${REPO_ROOT}/scripts/postprocess_uniffi_swift.py" "${TMP_UNIFFI_DIR}/ttzip_engine.swift"
        copy_if_changed "${TMP_UNIFFI_DIR}/ttzip_engine.swift" "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engine.swift"
    fi

    # 幂等同步 Scaffolding 头文件
    if [ -f "${TMP_UNIFFI_DIR}/ttzip_engineFFI.h" ]; then
        copy_if_changed "${TMP_UNIFFI_DIR}/ttzip_engineFFI.h" "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engineFFI.h"
        copy_if_changed "${TMP_UNIFFI_DIR}/ttzip_engineFFI.h" "${REPO_ROOT}/Sources/CTTZipBridge/include/ttzip_engineFFI.h"
        copy_if_changed "${TMP_UNIFFI_DIR}/ttzip_engineFFI.h" "${XCFRAMEWORK_MAC_DIR}/Headers/ttzip_engineFFI.h"
        copy_if_changed "${TMP_UNIFFI_DIR}/ttzip_engineFFI.h" "${SLICE_DIR}/Headers/ttzip_engineFFI.h"
    fi
    if [ -f "${TMP_UNIFFI_DIR}/ttzip_engineFFI.modulemap" ]; then
        copy_if_changed "${TMP_UNIFFI_DIR}/ttzip_engineFFI.modulemap" "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engineFFI.modulemap"
    fi

    rm -rf "${TMP_UNIFFI_DIR}"
fi

# ------------------------------------------------------------------------------
# 6. 落盘成功构建指纹
# ------------------------------------------------------------------------------
echo "${CURRENT_FINGERPRINT}" > "${FINGERPRINT_FILE}"

echo "=========================================="
echo "✅ [SUCCESS] Pure UniFFI engine & universal library generated successfully."
echo "=========================================="


