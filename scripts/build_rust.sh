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

find_llvm_objcopy() {
    local objcopy=""
    local sysroot host
    if command -v llvm-objcopy >/dev/null 2>&1; then
        command -v llvm-objcopy
        return 0
    fi
    sysroot="$(rustc --print sysroot 2>/dev/null || true)"
    host="$(rustc -vV 2>/dev/null | awk '/^host:/ { print $2 }' || true)"
    if [ -n "${sysroot}" ] && [ -n "${host}" ]; then
        local candidate="${sysroot}/lib/rustlib/${host}/bin/llvm-objcopy"
        if [ -x "${candidate}" ]; then
            printf '%s\n' "${candidate}"
            return 0
        fi
    fi
    local rustup_home="${RUSTUP_HOME:-${HOME}/.rustup}"
    local candidate
    candidate="$(ls "${rustup_home}"/toolchains/*/lib/rustlib/*/bin/llvm-objcopy 2>/dev/null | head -n 1 || true)"
    if [ -n "${candidate}" ] && [ -x "${candidate}" ]; then
        printf '%s\n' "${candidate}"
        return 0
    fi
    return 1
}

strip_embedded_bitcode() {
    local archive="$1"
    local objcopy
    if ! objcopy="$(find_llvm_objcopy)"; then
        echo "--> [WARN] llvm-objcopy not found; keeping the embedded bitcode in $(basename "${archive}")"
        return 0
    fi
    "${objcopy}" --remove-section=__LLVM,__bitcode "${archive}" >/dev/null 2>&1 || true
}

deduplicate_archive() {
    local archive="$1"
    local workdir
    workdir="$(mktemp -d /tmp/ttzip-dedup-XXXXXX)"
    mkdir -p "${workdir}/readd"

    # ---------------------------------------------------------------------
    # Pass 1: one copy per member name
    #
    # A Rust static library bundles its upstream crates, so merging two of
    # them carries the same object file twice. A linker that loads every
    # member of the archive then reports duplicate symbols.
    #
    # Two occurrences of one member name are the same object when their bytes
    # match, and the first one is kept. When the bytes differ the occurrences
    # are different builds, so the symbols they define decide: an occurrence
    # that defines a symbol no kept occurrence defines is a different variant
    # and stays, the others are dropped.
    #
    # The symbol list is read with `nm`, which cannot decode the LLVM bitcode
    # some Rust objects carry, so the byte comparison is the primary test and
    # the symbol list is only consulted for occurrences that differ.
    # ---------------------------------------------------------------------
    local duplicates name total index kept_count member group
    duplicates="$(ar -t "${archive}" | LC_ALL=C sort | uniq -d)"
    if [ -z "${duplicates}" ]; then
        return 0
    fi
    echo "--> [INFO] Removing duplicate members from $(basename "${archive}")"
    cp "${archive}" "${workdir}/probe.a"
    group=0
    while IFS= read -r name; do
        [ -z "${name}" ] && continue
        total="$(ar -t "${workdir}/probe.a" | grep -c -x -F -- "${name}" || true)"
        if [ "${total}" -le 1 ]; then
            continue
        fi
        group=$((group + 1))
        kept_count=0
        index=0
        : > "${workdir}/kept_symbols_${group}.txt"
        : > "${workdir}/kept_members_${group}.txt"
        while [ "${index}" -lt "${total}" ]; do
            index=$((index + 1))
            member="${workdir}/member_${group}_${index}.o"
            ( cd "${workdir}" && ar -p probe.a "${name}" > "member_${group}_${index}.o" )
            ( cd "${workdir}" && ar -d probe.a "${name}" ) >/dev/null 2>&1 || true
            if [ "${kept_count}" -eq 0 ]; then
                kept_count=1
                cp "${member}" "${workdir}/keep_${group}_1.o"
                printf '%s\n' "keep_${group}_1.o" >> "${workdir}/kept_members_${group}.txt"
                nm -g "${member}" 2>/dev/null \
                    | awk '$1 ~ /^[0-9a-fA-F]+$/ && $2 != "U" { print $NF }' \
                    | LC_ALL=C sort -u > "${workdir}/kept_symbols_${group}.txt" || true
                continue
            fi
            local identical=0 kept_member
            while IFS= read -r kept_member; do
                [ -z "${kept_member}" ] && continue
                if cmp -s "${workdir}/${kept_member}" "${member}"; then
                    identical=1
                    break
                fi
            done < "${workdir}/kept_members_${group}.txt"
            if [ "${identical}" -eq 1 ]; then
                continue
            fi
            nm -g "${member}" 2>/dev/null \
                | awk '$1 ~ /^[0-9a-fA-F]+$/ && $2 != "U" { print $NF }' \
                | LC_ALL=C sort -u > "${workdir}/symbols_${group}_${index}.txt" || true
            comm -23 "${workdir}/symbols_${group}_${index}.txt" "${workdir}/kept_symbols_${group}.txt" \
                > "${workdir}/new_${group}_${index}.txt" || true
            if [ ! -s "${workdir}/new_${group}_${index}.txt" ]; then
                continue
            fi
            kept_count=$((kept_count + 1))
            cp "${member}" "${workdir}/keep_${group}_${kept_count}.o"
            printf '%s\n' "keep_${group}_${kept_count}.o" >> "${workdir}/kept_members_${group}.txt"
            cat "${workdir}/new_${group}_${index}.txt" >> "${workdir}/kept_symbols_${group}.txt"
            LC_ALL=C sort -u -o "${workdir}/kept_symbols_${group}.txt" "${workdir}/kept_symbols_${group}.txt"
        done
        local remaining="${total}"
        while [ "${remaining}" -gt 0 ]; do
            ar -d "${archive}" "${name}" >/dev/null 2>&1 || break
            remaining=$((remaining - 1))
        done
        local kept_member kept_index target_name
        kept_index=0
        while IFS= read -r kept_member; do
            [ -z "${kept_member}" ] && continue
            kept_index=$((kept_index + 1))
            target_name="${name}"
            if [ "${kept_index}" -gt 1 ]; then
                target_name="${name}.${kept_index}"
            fi
            cp "${workdir}/${kept_member}" "${workdir}/readd/${target_name}"
            ar -r "${archive}" "${workdir}/readd/${target_name}" >/dev/null 2>&1 || true
        done < "${workdir}/kept_members_${group}.txt"
    done <<EOF
${duplicates}
EOF

    # ---------------------------------------------------------------------
    # Pass 2: one definer per symbol
    #
    # Rust emits the allocator shims into a separate object per crate that
    # pulls them in, and those objects define exactly the same symbols. A
    # member whose every symbol is already defined by an earlier member
    # carries nothing of its own and is dropped.
    # ---------------------------------------------------------------------
    local round
    for round in 1 2 3 4 5 6 7 8 9 10; do
        nm -g "${archive}" 2>/dev/null | awk '
            /^[^ ].*:$/ { member = $0; sub(/:$/, "", member); order[++n] = member; next }
            $1 ~ /^[0-9a-fA-F]+$/ && $2 != "U" {
                if (!((member SUBSEP $NF) in seen)) {
                    seen[member, $NF] = 1
                    syms[member] = syms[member] $NF "\n"
                    owners[$NF] = owners[$NF] " " member
                }
            }
            END {
                for (i = 1; i <= n; i++) {
                    m = order[i]
                    if (!(m in syms)) continue
                    droppable = 1
                    split(syms[m], list, "\n")
                    for (j in list) {
                        s = list[j]
                        if (s == "") continue
                        split(owners[s], defs, " ")
                        if (defs[1] == m) { droppable = 0; break }
                    }
                    if (droppable) print m
                }
            }
        ' > "${workdir}/droppable.txt" || true
        if [ ! -s "${workdir}/droppable.txt" ]; then
            break
        fi
        echo "--> [INFO] Dropping $(wc -l < "${workdir}/droppable.txt" | tr -d ' ') redundant members from $(basename "${archive}")"
        local droppable_member
        while IFS= read -r droppable_member; do
            [ -z "${droppable_member}" ] && continue
            ar -d "${archive}" "${droppable_member}" >/dev/null 2>&1 || true
        done < "${workdir}/droppable.txt"
    done


    # ---------------------------------------------------------------------
    # Pass 3: weaken the symbols that more than one member defines
    #
    # A crate that is compiled by two workspaces with different build flags
    # ends up in the archive twice, under two different member names. The
    # `#[no_mangle]` symbols of such a crate are then defined twice, and a
    # linker that loads every member of the archive reports them as duplicate
    # symbols. Weakening every copy of such a symbol lets the linker pick one
    # of them instead.
    # ---------------------------------------------------------------------
    local duplicate_symbols objcopy
    duplicate_symbols="$(nm -g "${archive}" 2>/dev/null | awk '
        /^[^ ].*:$/ { next }
        $1 ~ /^[0-9a-fA-F]+$/ && $2 != "U" { print $NF }
    ' | LC_ALL=C sort | uniq -d || true)"
    if [ -n "${duplicate_symbols}" ] && objcopy="$(find_llvm_objcopy)"; then
        local weaken_args=()
        while IFS= read -r duplicate_symbol; do
            [ -z "${duplicate_symbol}" ] && continue
            weaken_args+=("--weaken-symbol=${duplicate_symbol}")
        done <<EOF
${duplicate_symbols}
EOF
        if [ ${#weaken_args[@]} -gt 0 ]; then
            echo "--> [INFO] Weakening $(wc -l <<< "${duplicate_symbols}" | tr -d ' ') duplicated symbols in $(basename "${archive}")"
            "${objcopy}" "${weaken_args[@]}" "${archive}" >/dev/null 2>&1 || true
        fi
    fi
    ranlib -s "${archive}" >/dev/null 2>&1 || true
    rm -rf "${workdir}"
}

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


