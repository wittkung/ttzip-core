# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
RUST_DIR="${REPO_ROOT}/rust"
FRAMEWORKS_DIR="${REPO_ROOT}/Frameworks"
XCFRAMEWORK_DIR="${FRAMEWORKS_DIR}/TTZipVendor.xcframework"
EPUB_XCFRAMEWORK_DIR="${FRAMEWORKS_DIR}/TTZipEpubVendor.xcframework"

BUILD_MODE="release"
CARGO_FLAGS="--release"
BUILD_TARGET=""
UNIVERSAL=0
OFFLINE_FLAG=""
FORCE_REBUILD=0

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo "Options:"
    echo "  --release        Build in release mode with the pinned rustup toolchain (default)"
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
# The deployment target has to reach the produced objects, not only the final
# link, otherwise the archive mixes objects that ask for macOS 14 with objects
# that ask for the SDK version of the build machine.
export CFLAGS="-mmacosx-version-min=14.0 ${CFLAGS:-}"
export CXXFLAGS="-mmacosx-version-min=14.0 ${CXXFLAGS:-}"
# The rustc flags have to match the ones of the app core build. Cargo derives
# the object file names from the build flags, so the shared crates are
# compiled twice when the flags differ, and the two builds define the same
# `#[no_mangle]` symbols.
export RUSTFLAGS="-C link-arg=-mmacosx-version-min=14.0 ${RUSTFLAGS:-}"

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

if [ "${BUILD_MODE}" = release ]; then
    source "${SCRIPT_DIR}/release_codec_stage.sh"
    prepare_release_codec_stages "${TARGETS[@]}"
fi

SLICE_DIR="${XCFRAMEWORK_DIR}/${SLICE_IDENTIFIER}"

CARGO=(cargo)
if [ "${BUILD_MODE}" = "release" ]; then
    # Use the rustup sysroot with a macOS 14-compatible floor and macOS 27 strip support.
    RUST_VERSION="1.99.1"
    RUSTUP="$(command -v rustup || printf '%s\n' "${CARGO_HOME:-${HOME}/.cargo}/bin/rustup")"
    if ! RUSTC="$("${RUSTUP}" which --toolchain "${RUST_VERSION}" rustc)"; then
        echo "error: install the release Rust toolchain with: rustup toolchain install ${RUST_VERSION}" >&2
        exit 1
    fi
    export RUSTC RUSTC_WRAPPER="" RUSTC_WORKSPACE_WRAPPER=""
    CARGO=("${RUSTUP}" run "${RUST_VERSION}" cargo)
    "${RUSTC}" --version
    for target in "${TARGETS[@]}"; do
        target_libdir="$("${RUSTC}" --print target-libdir --target "${target}")"
        if [ ! -d "${target_libdir}" ]; then
            echo "error: install the release target with: rustup target add --toolchain ${RUST_VERSION} ${target}" >&2
            exit 1
        fi
        echo "--> Rust target libraries: ${target_libdir}"
    done
fi

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
for index in "${!TARGETS[@]}"; do
    target="${TARGETS[index]}"
    release_stage=""
    if [ "${BUILD_MODE}" = release ]; then
        release_stage="${RELEASE_CODEC_STAGES[index]}"
    fi
    echo "--> [INFO] Building ttzip-engine and ttzip-epub for ${target} (${BUILD_MODE})..."
    TTZIP_RELEASE_CODEC_STAGE="${release_stage}" "${CARGO[@]}" build --manifest-path "${RUST_DIR}/Cargo.toml" --package ttzip-engine --package ttzip-epub --features ttzip-engine/full --target "${target}" ${CARGO_FLAGS} ${OFFLINE_FLAG}
    
    TARGET_LIB="${EFFECTIVE_TARGET_DIR}/${target}/${BUILD_MODE}/libttzip_engine.a"
    if [ -f "${TARGET_LIB}" ]; then
        BUILT_LIBS+=("${TARGET_LIB}")
    else
        echo "❌ Error: Expected static library not found at ${TARGET_LIB}"
        exit 1
    fi
done

# ------------------------------------------------------------------------------
# Duplicate member removal
# ------------------------------------------------------------------------------
#
# A Rust static library bundles its upstream crates, so merging two of them
# carries the same object file twice. A linker that loads every member of the
# archive reports duplicate symbols. Members that define the same symbols are
# the same object file and are removed; members that define different symbols
# are different build variants and are kept.

# ------------------------------------------------------------------------------
# Embedded bitcode removal
# ------------------------------------------------------------------------------
#
# A Rust release build embeds the LLVM bitcode of every object so that a
# consumer can run link time optimization on the archive. The shipped
# framework does not need it, and leaving it in triples the size of the
# archive.
#
# The bitcode has to be removed with `llvm-objcopy`. Apple's `strip` drops the
# `.llvm.<hash>` suffix that Rust puts on the internal symbols it instantiates
# in more than one codegen unit, and without that suffix two members define
# the same symbol, which a linker that loads every member of the archive
# reports as a duplicate symbol.

strip_embedded_bitcode() {
    python3 "${SCRIPT_DIR}/deduplicate_archive.py" strip-bitcode "$1"
}

deduplicate_archive() {
    python3 "${SCRIPT_DIR}/deduplicate_archive.py" deduplicate "$1"
}

# ------------------------------------------------------------------------------
# 3. 生成并幂等部署静态库 libTTZipVendor.a
# ------------------------------------------------------------------------------
echo "--> [INFO] Packaging static library into ${SLICE_DIR}/libTTZipVendor.a..."
mkdir -p "${SLICE_DIR}"

PER_TARGET_MERGED=()
PER_TARGET_EPUB=()
for index in "${!TARGETS[@]}"; do
    target="${TARGETS[index]}"
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

    if [ "${BUILD_MODE}" = release ]; then
        for libname in "${RELEASE_CODEC_ARCHIVES[@]}"; do
            TARGET_PARTS+=("${RELEASE_CODEC_STAGES[index]}/${libname}")
        done
    else
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
    fi

    PER_TARGET_OUT="$(mktemp /tmp/ttzip_part_${target}_XXXXXX.a)"
    if [ ${#TARGET_PARTS[@]} -gt 1 ]; then
        libtool -static -no_warning_for_no_symbols -o "${PER_TARGET_OUT}" "${TARGET_PARTS[@]}"
    elif [ ${#TARGET_PARTS[@]} -eq 1 ]; then
        cp "${TARGET_PARTS[0]}" "${PER_TARGET_OUT}"
    fi
    PER_TARGET_MERGED+=("${PER_TARGET_OUT}")

    EPUB_TARGET="${EFFECTIVE_TARGET_DIR}/${target}/${BUILD_MODE}/libttzip_epub.a"
    EPUB_TARGET_OUT="$(mktemp /tmp/ttzip_epub_${target}_XXXXXX.a)"
    libtool -static -no_warning_for_no_symbols -o "${EPUB_TARGET_OUT}" "${EPUB_TARGET}" "${TARGET_PARTS[@]:1}"
    PER_TARGET_EPUB+=("${EPUB_TARGET_OUT}")
done

TMP_LIB="$(mktemp /tmp/libTTZipVendor_XXXXXX)"
if [ ${#PER_TARGET_MERGED[@]} -gt 1 ]; then
    echo "--> Combining slices via lipo: ${PER_TARGET_MERGED[*]}"
    lipo -create "${PER_TARGET_MERGED[@]}" -output "${TMP_LIB}"
elif [ ${#PER_TARGET_MERGED[@]} -eq 1 ]; then
    cp "${PER_TARGET_MERGED[0]}" "${TMP_LIB}"
fi
rm -f "${PER_TARGET_MERGED[@]}"

strip_embedded_bitcode "${TMP_LIB}"

deduplicate_archive "${TMP_LIB}"
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

write_xcframework_plist() {
    local output="$1" library="$2" headers="${3:-}" header_entries="" tmp_plist
    if [ -n "${headers}" ]; then
        header_entries="			<key>HeadersPath</key>
			<string>${headers}</string>
"
    fi
    tmp_plist="$(mktemp /tmp/ttzip_plist_XXXXXX)"
    cat > "${tmp_plist}" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>AvailableLibraries</key>
	<array>
		<dict>
${header_entries}			<key>LibraryIdentifier</key>
			<string>${SLICE_IDENTIFIER}</string>
			<key>LibraryPath</key>
			<string>${library}</string>
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
    copy_if_changed "${tmp_plist}" "${output}"
    rm -f "${tmp_plist}"
}

write_xcframework_plist "${XCFRAMEWORK_DIR}/Info.plist" "libTTZipVendor.a" "Headers"

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
        "${CARGO[@]}" run ${OFFLINE_FLAG} --manifest-path "${RUST_DIR}/Cargo.toml" --package ttzip-bindgen --bin uniffi-bindgen -- generate \
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

EPUB_STAGE="$(mktemp -d)"
trap 'rm -rf "${EPUB_STAGE}" "${PER_TARGET_EPUB[@]}"' EXIT
EPUB_STAGED_FRAMEWORK="${EPUB_STAGE}/TTZipEpubVendor.xcframework"
EPUB_SLICE_DIR="${EPUB_STAGED_FRAMEWORK}/${SLICE_IDENTIFIER}"
mkdir -p "${EPUB_SLICE_DIR}"
EPUB_STATIC_LIB="${EPUB_SLICE_DIR}/libTTZipEpubVendor.a"
if [ ${#PER_TARGET_EPUB[@]} -gt 1 ]; then
    lipo -create "${PER_TARGET_EPUB[@]}" -output "${EPUB_STATIC_LIB}"
else
    cp "${PER_TARGET_EPUB[0]}" "${EPUB_STATIC_LIB}"
fi
strip_embedded_bitcode "${EPUB_STATIC_LIB}"
deduplicate_archive "${EPUB_STATIC_LIB}"
write_xcframework_plist "${EPUB_STAGED_FRAMEWORK}/Info.plist" "libTTZipEpubVendor.a"
rm -rf "${EPUB_XCFRAMEWORK_DIR}"
mv "${EPUB_STAGED_FRAMEWORK}" "${EPUB_XCFRAMEWORK_DIR}"

"${CARGO[@]}" build ${OFFLINE_FLAG} --manifest-path "${RUST_DIR}/Cargo.toml" --package ttzip-bindgen --bin uniffi-bindgen --package ttzip-tui --bin ttzip
bash "${SCRIPT_DIR}/generate_epub_uniffi.sh" \
    "${EFFECTIVE_TARGET_DIR}/debug/uniffi-bindgen" \
    "${EFFECTIVE_TARGET_DIR}/${FIRST_TARGET}/${BUILD_MODE}/libttzip_epub.dylib"

echo "=========================================="
echo "✅ [SUCCESS] UniFFI engine and EPUB libraries generated successfully."
echo "=========================================="


