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
VENDOR_DIR="${REPO_ROOT}/Vendor"
XCFRAMEWORK_DIR="${VENDOR_DIR}/TTZipVendor.xcframework"
DIST_DIR="${REPO_ROOT}/dist"

BUILD_MODE="release"
CARGO_FLAGS="--release"
VERSION="1.0.0"
OFFLINE_FLAG=""
BUILD_NATIVE_ONLY=0

if [ "${TTZIP_FAST_SDK:-0}" = "1" ]; then
    BUILD_NATIVE_ONLY=1
fi

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo "Options:"
    echo "  --release        Build in release mode with LTO and -O3 (default)"
    echo "  --debug          Build in debug mode"
    echo "  --native         Fast path: build only host native architecture (e.g. arm64)"
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

echo "======================================================================"
echo "📦 Building TTZipCore Standalone SDK (${BUILD_MODE} v${VERSION})"
echo "======================================================================"

export PATH="$HOME/.cargo/bin:$PATH"
export MACOSX_DEPLOYMENT_TARGET="14.0"

if command -v sccache >/dev/null 2>&1; then
    export RUSTC_WRAPPER="sccache"
fi

EFFECTIVE_TARGET_DIR="${CARGO_TARGET_DIR:-${RUST_DIR}/target}"
HOST_ARCH="$(uname -m)"
if [ "${HOST_ARCH}" = "arm64" ]; then
    HOST_TARGET="aarch64-apple-darwin"
else
    HOST_TARGET="x86_64-apple-darwin"
fi

mkdir -p "${DIST_DIR}" "${REPO_ROOT}/Sources/CTTZipBridge/include" "${REPO_ROOT}/Sources/TTZipCore/Generated"

SLICE_DIR="${XCFRAMEWORK_DIR}/macos-arm64_x86_64"
mkdir -p "${SLICE_DIR}/Headers" "${XCFRAMEWORK_DIR}/macos-arm64/Headers"

if [ "${BUILD_NATIVE_ONLY}" = "1" ]; then
    echo "--> [INFO] Fast Path: Building native architecture only (${HOST_TARGET})..."
    cargo build --manifest-path "${RUST_DIR}/Cargo.toml" --package ttzip-engine --target "${HOST_TARGET}" ${CARGO_FLAGS} ${OFFLINE_FLAG}
    
    TARGET_LIB="${EFFECTIVE_TARGET_DIR}/${HOST_TARGET}/${BUILD_MODE}/libttzip_engine.a"
    CODEC_LIB="$(find "${EFFECTIVE_TARGET_DIR}/${HOST_TARGET}" -name "libttzip_native_codecs.a" -exec ls -t {} + 2>/dev/null | head -n 1 || true)"
    
    if [ -n "${CODEC_LIB}" ] && [ -f "${CODEC_LIB}" ]; then
        libtool -static -no_warning_for_no_symbols -o "${SLICE_DIR}/libTTZipVendor.a" "${TARGET_LIB}" "${CODEC_LIB}"
    else
        cp -f "${TARGET_LIB}" "${SLICE_DIR}/libTTZipVendor.a"
    fi
    cp -f "${SLICE_DIR}/libTTZipVendor.a" "${XCFRAMEWORK_DIR}/macos-arm64/libTTZipVendor.a"
else
    TARGETS=("aarch64-apple-darwin" "x86_64-apple-darwin")
    for target in "${TARGETS[@]}"; do
        if ! rustup target list --installed | grep -q "${target}"; then
            echo "--> [INFO] Adding rust target: ${target}..."
            rustup target add "${target}" || true
        fi
    done

    echo "--> [INFO] Building ttzip-engine for arm64 & x86_64 in parallel (${BUILD_MODE})..."
    cargo build --manifest-path "${RUST_DIR}/Cargo.toml" --package ttzip-engine --target "aarch64-apple-darwin" ${CARGO_FLAGS} ${OFFLINE_FLAG} &
    PID_ARM64=$!
    cargo build --manifest-path "${RUST_DIR}/Cargo.toml" --package ttzip-engine --target "x86_64-apple-darwin" ${CARGO_FLAGS} ${OFFLINE_FLAG} &
    PID_X86=$!

    wait ${PID_ARM64}
    wait ${PID_X86}

    BUILT_ARM64_LIB="${EFFECTIVE_TARGET_DIR}/aarch64-apple-darwin/${BUILD_MODE}/libttzip_engine.a"
    BUILT_X86_64_LIB="${EFFECTIVE_TARGET_DIR}/x86_64-apple-darwin/${BUILD_MODE}/libttzip_engine.a"

    ARM64_COMBINED="/tmp/libTTZip_arm64.a"
    X86_64_COMBINED="/tmp/libTTZip_x86_64.a"

    CODEC_LIB_ARM64="$(find "${EFFECTIVE_TARGET_DIR}/aarch64-apple-darwin" -name "libttzip_native_codecs.a" -exec ls -t {} + 2>/dev/null | head -n 1 || true)"
    CODEC_LIB_X86_64="$(find "${EFFECTIVE_TARGET_DIR}/x86_64-apple-darwin" -name "libttzip_native_codecs.a" -exec ls -t {} + 2>/dev/null | head -n 1 || true)"

    if [ -n "${CODEC_LIB_ARM64}" ] && [ -f "${CODEC_LIB_ARM64}" ]; then
        libtool -static -no_warning_for_no_symbols -o "${ARM64_COMBINED}" "${BUILT_ARM64_LIB}" "${CODEC_LIB_ARM64}"
    else
        cp -f "${BUILT_ARM64_LIB}" "${ARM64_COMBINED}"
    fi

    if [ -n "${CODEC_LIB_X86_64}" ] && [ -f "${CODEC_LIB_X86_64}" ]; then
        libtool -static -no_warning_for_no_symbols -o "${X86_64_COMBINED}" "${BUILT_X86_64_LIB}" "${CODEC_LIB_X86_64}"
    else
        cp -f "${BUILT_X86_64_LIB}" "${X86_64_COMBINED}"
    fi

    echo "--> [INFO] Combining arm64 + x86_64 via lipo into universal slice..."
    lipo -create "${ARM64_COMBINED}" "${X86_64_COMBINED}" -output "${SLICE_DIR}/libTTZipVendor.a"
    strip -S "${SLICE_DIR}/libTTZipVendor.a" 2>/dev/null || true
    cp -f "${SLICE_DIR}/libTTZipVendor.a" "${XCFRAMEWORK_DIR}/macos-arm64/libTTZipVendor.a"
fi

# 5. 生成 UniFFI 绑定与 Scaffolding C 头文件
echo "--> [INFO] Generating Mozilla UniFFI bindings..."
FIRST_DYLIB="${EFFECTIVE_TARGET_DIR}/${HOST_TARGET}/${BUILD_MODE}/libttzip_engine.dylib"
if [ ! -f "${FIRST_DYLIB}" ]; then
    FIRST_DYLIB="${EFFECTIVE_TARGET_DIR}/aarch64-apple-darwin/${BUILD_MODE}/libttzip_engine.dylib"
fi
if [ ! -f "${FIRST_DYLIB}" ]; then
    FIRST_DYLIB="${EFFECTIVE_TARGET_DIR}/${BUILD_MODE}/libttzip_engine.dylib"
fi

UNIFFI_BIN=""
for candidate in "${EFFECTIVE_TARGET_DIR}/release/uniffi-bindgen" "${EFFECTIVE_TARGET_DIR}/debug/uniffi-bindgen" "${RUST_DIR}/target/release/uniffi-bindgen" "${RUST_DIR}/target/debug/uniffi-bindgen"; do
    if [ -x "${candidate}" ]; then
        UNIFFI_BIN="${candidate}"
        break
    fi
done

if [ -n "${UNIFFI_BIN}" ]; then
    (
        cd "${RUST_DIR}"
        "${UNIFFI_BIN}" generate \
            --library "${FIRST_DYLIB}" \
            --language swift \
            --out-dir "${REPO_ROOT}/Sources/TTZipCore/Generated" \
            --metadata-no-deps
    )
else
    (
        cd "${RUST_DIR}"
        cargo run ${OFFLINE_FLAG} --bin uniffi-bindgen --features full generate \
            --library "${FIRST_DYLIB}" \
            --language swift \
            --out-dir "${REPO_ROOT}/Sources/TTZipCore/Generated" \
            --metadata-no-deps
    )
fi



# 执行 Swift 6 并发安全后处理
if [ -f "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engine.swift" ]; then
    python3 "${REPO_ROOT}/scripts/postprocess_uniffi_swift.py" "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engine.swift"
fi

# 部署 C-Bridge 头文件
if [ -f "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engineFFI.h" ]; then
    cp -f "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engineFFI.h" "${REPO_ROOT}/Sources/CTTZipBridge/include/"
    cp -f "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engineFFI.h" "${SLICE_DIR}/Headers/"
    cp -f "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engineFFI.h" "${XCFRAMEWORK_DIR}/macos-arm64/Headers/"
fi

# 6. 生成标准 XCFramework Info.plist
cat << 'EOF' > "${XCFRAMEWORK_DIR}/Info.plist"
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

# 7. 打包 XCFramework ZIP 并计算 SHA-256 校验和
ZIP_NAME="TTZipVendor-v${VERSION}.xcframework.zip"
ZIP_PATH="${DIST_DIR}/${ZIP_NAME}"

echo "--> [INFO] Creating ${ZIP_PATH}..."
rm -f "${ZIP_PATH}"
(
    cd "${VENDOR_DIR}"
    zip -qry "${ZIP_PATH}" "TTZipVendor.xcframework"
)

CHECKSUM="$(shasum -a 256 "${ZIP_PATH}" | awk '{print $1}')"
echo "${CHECKSUM}" > "${DIST_DIR}/${ZIP_NAME}.sha256"


echo "======================================================================"
echo "✅ [SUCCESS] TTZipCore Universal SDK built successfully!"
echo "   Artifact : ${ZIP_PATH}"
echo "   Size     : $(ls -lh "${ZIP_PATH}" | awk '{print $5}')"
echo "   SHA-256  : ${CHECKSUM}"
echo "   Slices   : $(lipo -info "${SLICE_DIR}/libTTZipVendor.a")"
echo "======================================================================"
