# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
RUST_DIR="${REPO_ROOT}/rust"
FRAMEWORKS_DIR="${REPO_ROOT}/Frameworks"
XCFRAMEWORK_DIR="${FRAMEWORKS_DIR}/TTZipVendor.xcframework"

BUILD_MODE="release"
CARGO_FLAGS="--release"
BUILD_TARGET=""
UNIVERSAL=0
OFFLINE_FLAG=""
FORCE_REBUILD=0

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo "Options:"
    echo "  --release        Build in release mode with ThinLTO and -O3 (default)"
    echo "  --debug          Build in debug mode"
    echo "  --target <TRGT>  Build specific target (e.g. aarch64-apple-darwin)"
    echo "  --universal      Build universal fat binary (arm64 + x86_64)"
    echo "  --force, -f      Force full rebuild"
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
        --universal)
            UNIVERSAL=1
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

export PATH="$HOME/.cargo/bin:$PATH"
export MACOSX_DEPLOYMENT_TARGET="14.0"

EFFECTIVE_TARGET_DIR="${CARGO_TARGET_DIR:-${RUST_DIR}/target}"
HOST_ARCH="$(uname -m)"
TARGETS=()
ARCHS=()

if [ "${UNIVERSAL}" -eq 1 ]; then
    TARGETS=("aarch64-apple-darwin" "x86_64-apple-darwin")
    ARCHS=("arm64" "x86_64")
    SLICE_IDENTIFIER="macos-arm64_x86_64"
elif [ -n "${BUILD_TARGET}" ]; then
    TARGETS+=("${BUILD_TARGET}")
    case "${BUILD_TARGET}" in
        aarch64*) ARCHS=("arm64") ;;
        x86_64*)  ARCHS=("x86_64") ;;
        *)        ARCHS=("${BUILD_TARGET}") ;;
    esac
    SLICE_IDENTIFIER="macos-${ARCHS[0]}"
else
    if [ "${HOST_ARCH}" = "arm64" ]; then
        TARGETS+=("aarch64-apple-darwin")
        ARCHS=("arm64")
        SLICE_IDENTIFIER="macos-arm64"
    else
        TARGETS+=("x86_64-apple-darwin")
        ARCHS=("x86_64")
        SLICE_IDENTIFIER="macos-x86_64"
    fi
fi

SLICE_DIR="${XCFRAMEWORK_DIR}/${SLICE_IDENTIFIER}"

# Clean obsolete slices to prevent stale slices or mismatched metadata
if [ -d "${XCFRAMEWORK_DIR}" ]; then
    for existing_slice in "${XCFRAMEWORK_DIR}"/macos-*; do
        if [ -d "${existing_slice}" ] && [ "$(basename "${existing_slice}")" != "${SLICE_IDENTIFIER}" ]; then
            echo "--> Cleaning obsolete slice: $(basename "${existing_slice}")"
            rm -rf "${existing_slice}"
        fi
    done
fi

# Helper function to copy only if content differs to preserve file mtime
copy_if_changed() {
    local src="$1"
    local dst="$2"
    if [ ! -f "${dst}" ] || ! cmp -s "${src}" "${dst}"; then
        mkdir -p "$(dirname "${dst}")"
        cp -f "${src}" "${dst}"
    fi
}

echo "=========================================="
echo "📦 Building TTZip Rust Engine (${BUILD_MODE})"
echo "=========================================="

mkdir -p "${SLICE_DIR}/Headers" "${REPO_ROOT}/Sources/TTZipEngineFFI/include" "${REPO_ROOT}/Sources/TTZipCore/Generated"

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
echo "--> [INFO] Packaging static library into ${SLICE_DIR}/libTTZipVendor.a..."
mkdir -p "${SLICE_DIR}"

PER_TARGET_MERGED=()
for target in "${TARGETS[@]}"; do
    TARGET_PARTS=()
    TARGET_LIB="${EFFECTIVE_TARGET_DIR}/${target}/${BUILD_MODE}/libttzip_engine.a"
    if [ -f "${TARGET_LIB}" ]; then
        TARGET_PARTS+=("${TARGET_LIB}")
    fi
    
    CODEC_LIB="$(ls -t ${EFFECTIVE_TARGET_DIR}/${target}/${BUILD_MODE}/build/ttzip-engine-*/out/libttzip_native_codecs.a 2>/dev/null | head -n 1 || true)"
    if [ -z "${CODEC_LIB}" ]; then
        CODEC_LIB="$(find "${EFFECTIVE_TARGET_DIR}/${target}" -name "libttzip_native_codecs.a" -exec ls -t {} + 2>/dev/null | head -n 1 || true)"
    fi
    if [ -n "${CODEC_LIB}" ] && [ -f "${CODEC_LIB}" ]; then
        TARGET_PARTS+=("${CODEC_LIB}")
    fi

    # Resolve library directory dynamically without hardcoded host paths
    CANDIDATE_LIB_DIRS=()
    if [ -n "${TTZIP_SYS_LIB_DIR:-}" ] && [ -d "${TTZIP_SYS_LIB_DIR}" ]; then
        CANDIDATE_LIB_DIRS+=("${TTZIP_SYS_LIB_DIR}")
    fi
    if command -v brew >/dev/null 2>&1; then
        BREW_PREFIX="$(brew --prefix 2>/dev/null || true)"
        if [ -n "${BREW_PREFIX}" ] && [ -d "${BREW_PREFIX}/lib" ]; then
            CANDIDATE_LIB_DIRS+=("${BREW_PREFIX}/lib")
        fi
    fi

    for libname in libdeflate.a liblz4.a libzstd.a; do
        FOUND_LIB=""
        pkg_base="${libname#lib}"
        pkg_base="${pkg_base%.a}"
        if command -v pkg-config >/dev/null 2>&1 && pkg-config --exists "${pkg_base}" 2>/dev/null; then
            PKG_LIB_DIR="$(pkg-config --variable=libdir "${pkg_base}" 2>/dev/null || true)"
            if [ -n "${PKG_LIB_DIR}" ] && [ -f "${PKG_LIB_DIR}/${libname}" ]; then
                FOUND_LIB="${PKG_LIB_DIR}/${libname}"
            fi
        fi
        if [ -z "${FOUND_LIB}" ]; then
            for candidate in "${CANDIDATE_LIB_DIRS[@]}"; do
                if [ -f "${candidate}/${libname}" ]; then
                    FOUND_LIB="${candidate}/${libname}"
                    break
                fi
            done
        fi
        if [ -n "${FOUND_LIB}" ]; then
            TARGET_PARTS+=("${FOUND_LIB}")
        fi
    done

    PER_TARGET_OUT="$(mktemp /tmp/ttzip_part_${target}_XXXXXX.a)"
    if [ ${#TARGET_PARTS[@]} -gt 1 ]; then
        libtool -static -no_warning_for_no_symbols -o "${PER_TARGET_OUT}" "${TARGET_PARTS[@]}"
    elif [ ${#TARGET_PARTS[@]} -eq 1 ]; then
        cp "${TARGET_PARTS[0]}" "${PER_TARGET_OUT}"
    fi
    PER_TARGET_MERGED+=("${PER_TARGET_OUT}")
done

TMP_LIB="$(mktemp /tmp/libTTZipVendor_XXXXXX)"
if [ ${#PER_TARGET_MERGED[@]} -gt 1 ]; then
    echo "--> Combining slices via lipo: ${PER_TARGET_MERGED[*]}"
    lipo -create "${PER_TARGET_MERGED[@]}" -output "${TMP_LIB}"
elif [ ${#PER_TARGET_MERGED[@]} -eq 1 ]; then
    cp "${PER_TARGET_MERGED[0]}" "${TMP_LIB}"
fi
rm -f "${PER_TARGET_MERGED[@]}"

strip -x "${TMP_LIB}" 2>/dev/null || true
copy_if_changed "${TMP_LIB}" "${SLICE_DIR}/libTTZipVendor.a"
rm -f "${TMP_LIB}"

# ------------------------------------------------------------------------------
# 4. 幂等生成 XCFramework 必需的 Info.plist
# ------------------------------------------------------------------------------
ARCHS_PLIST_ENTRIES=""
for arch in "${ARCHS[@]}"; do
    ARCHS_PLIST_ENTRIES="${ARCHS_PLIST_ENTRIES}				<string>${arch}</string>
"
done

TMP_PLIST="$(mktemp /tmp/ttzip_plist_XXXXXX)"
cat > "${TMP_PLIST}" << EOF
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
			<string>${SLICE_IDENTIFIER}</string>
			<key>LibraryPath</key>
			<string>libTTZipVendor.a</string>
			<key>SupportedArchitectures</key>
			<array>
${ARCHS_PLIST_ENTRIES}			</array>
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
# 5. 在临时沙盒中生成 Mozilla UniFFI Swift 绑定并幂等同步
# ------------------------------------------------------------------------------
echo "--> [INFO] Generating Mozilla UniFFI Swift bindings..."
FIRST_TARGET="${TARGETS[0]}"
FIRST_DYLIB="${EFFECTIVE_TARGET_DIR}/${FIRST_TARGET}/${BUILD_MODE}/libttzip_engine.dylib"
if [ ! -f "${FIRST_DYLIB}" ]; then
    FIRST_DYLIB="${EFFECTIVE_TARGET_DIR}/${BUILD_MODE}/libttzip_engine.dylib"
fi

if [ -f "${FIRST_DYLIB}" ]; then
    TMP_UNIFFI_DIR="$(mktemp -d /tmp/ttzip_uniffi.XXXXXX)"

    (
        cd "${RUST_DIR}"
        cargo run ${OFFLINE_FLAG} --manifest-path "${RUST_DIR}/Cargo.toml" --package ttzip-bindgen --bin uniffi-bindgen -- generate \
            --library "${FIRST_DYLIB}" \
            --language swift \
            --out-dir "${TMP_UNIFFI_DIR}" \
            --no-format \
            --metadata-no-deps
    )

    # 执行 Swift 6 并发安全后处理
    if [ -f "${TMP_UNIFFI_DIR}/ttzip_engine.swift" ]; then
        python3 "${REPO_ROOT}/scripts/postprocess_uniffi_swift.py" "${TMP_UNIFFI_DIR}/ttzip_engine.swift"
        copy_if_changed "${TMP_UNIFFI_DIR}/ttzip_engine.swift" "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engine.swift"
    fi

    # 幂等同步 Scaffolding 头文件
    if [ -f "${TMP_UNIFFI_DIR}/ttzip_engineFFI.h" ]; then
        copy_if_changed "${TMP_UNIFFI_DIR}/ttzip_engineFFI.h" "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engineFFI.h"
        copy_if_changed "${TMP_UNIFFI_DIR}/ttzip_engineFFI.h" "${REPO_ROOT}/Sources/TTZipEngineFFI/include/ttzip_engineFFI.h"
        if [ -d "${REPO_ROOT}/Sources/CTTZipBridge/include" ]; then
            copy_if_changed "${TMP_UNIFFI_DIR}/ttzip_engineFFI.h" "${REPO_ROOT}/Sources/CTTZipBridge/include/ttzip_engineFFI.h"
        fi
        copy_if_changed "${TMP_UNIFFI_DIR}/ttzip_engineFFI.h" "${SLICE_DIR}/Headers/ttzip_engineFFI.h"
    fi
    if [ -f "${REPO_ROOT}/Sources/TTZipEngineFFI/include/module.modulemap" ]; then
        copy_if_changed "${REPO_ROOT}/Sources/TTZipEngineFFI/include/module.modulemap" "${SLICE_DIR}/Headers/module.modulemap"
    fi
    if [ -f "${TMP_UNIFFI_DIR}/ttzip_engineFFI.modulemap" ]; then
        copy_if_changed "${TMP_UNIFFI_DIR}/ttzip_engineFFI.modulemap" "${REPO_ROOT}/Sources/TTZipCore/Generated/ttzip_engineFFI.modulemap"
    fi

    rm -rf "${TMP_UNIFFI_DIR}"
fi

echo "=========================================="
echo "✅ [SUCCESS] Pure UniFFI engine & universal library generated successfully."
echo "=========================================="


