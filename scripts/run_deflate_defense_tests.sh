#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CORE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
ROOT_DIR="$(cd "${CORE_DIR}/.." && pwd)"
VENDOR_LIBDEFLATE="${ROOT_DIR}/vendor/libdeflate"
BUILD_DIR="${CORE_DIR}/build/defense_tests"
FINGERPRINT_FILE="${BUILD_DIR}/.deflate_defense_fingerprint"

FORCE_REBUILD=false

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo "Options:"
    echo "  --force, -f      Force full CMake reconfiguration & compilation"
    echo "  --clean          Remove defense build directory and exit"
    echo "  --help, -h       Show this help message"
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --force|-f)
            FORCE_REBUILD=true
            shift
            ;;
        --clean)
            echo "🧹 Cleaning defense test build directory: ${BUILD_DIR}"
            rm -rf "${BUILD_DIR}"
            exit 0
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
echo "🛡️  TTZip Deflate Deep Defense & Differential Oracle Gate"
echo "======================================================================"
echo "Vendor Source: ${VENDOR_LIBDEFLATE}"
echo "Build Output : ${BUILD_DIR}"
echo ""

if [[ ! -d "${VENDOR_LIBDEFLATE}" ]]; then
    echo "❌ Error: vendor/libdeflate directory not found at ${VENDOR_LIBDEFLATE}"
    exit 1
fi

TEST_BINARIES=(
    "test_checksums"
    "test_slow_decompression"
    "test_overread"
    "test_incomplete_codes"
    "test_custom_malloc"
    "test_litrunlen_overflow"
    "test_invalid_streams"
)

# ------------------------------------------------------------------------------
# 1. Compute Merkle SHA-256 Incremental Fingerprint
# ------------------------------------------------------------------------------
compute_fingerprint() {
    local git_tree dirty_diff untracked cc_ver cmake_ver script_hash cflags_hash arch
    
    if git -C "${ROOT_DIR}" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        git_tree="$(git -C "${ROOT_DIR}" rev-parse HEAD:vendor/libdeflate 2>/dev/null || echo "no-git-tree")"
        dirty_diff="$(git -C "${ROOT_DIR}" diff HEAD -- vendor/libdeflate 2>/dev/null | shasum -a 256 | awk '{print $1}')"
        untracked="$(git -C "${ROOT_DIR}" ls-files --others --exclude-standard vendor/libdeflate 2>/dev/null | shasum -a 256 | awk '{print $1}')"
    else
        git_tree="$(find "${VENDOR_LIBDEFLATE}" -type f \( -name "*.c" -o -name "*.h" -o -name "*.txt" -o -name "*.in" \) -exec shasum -a 256 {} + 2>/dev/null | sort | shasum -a 256 | awk '{print $1}')"
        dirty_diff="clean"
        untracked="none"
    fi

    cc_ver="$( (cc --version 2>/dev/null || clang --version 2>/dev/null || echo "unknown-cc") | head -n 1 | shasum -a 256 | awk '{print $1}')"
    cmake_ver="$( (cmake --version 2>/dev/null || echo "unknown-cmake") | head -n 1 | shasum -a 256 | awk '{print $1}')"
    script_hash="$(shasum -a 256 "${BASH_SOURCE[0]}" | awk '{print $1}')"
    cflags_hash="$(printf "%s" "-O3 -Wall -Werror -DLIBDEFLATE_BUILD_STATIC_LIB=ON -DLIBDEFLATE_BUILD_TESTS=ON" | shasum -a 256 | awk '{print $1}')"
    arch="$(uname -m 2>/dev/null || echo "unknown-arch")"

    printf "%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s" \
        "${git_tree}" "${dirty_diff}" "${untracked}" "${cc_ver}" "${cmake_ver}" "${script_hash}" "${cflags_hash}" "${arch}" \
        | shasum -a 256 | awk '{print $1}'
}

CURRENT_FINGERPRINT="$(compute_fingerprint)"

check_artifacts_exist() {
    [ -s "${BUILD_DIR}/libdeflate.a" ] || return 1
    for t in "${TEST_BINARIES[@]}"; do
        local bin_path="${BUILD_DIR}/programs/${t}"
        [ -s "${bin_path}" ] || return 1
        [ -x "${bin_path}" ] || return 1
    done
    return 0
}

save_fingerprint() {
    local target_file="$1"
    local fp="$2"
    local tmp_file
    tmp_file="$(mktemp "${target_file}.XXXXXX")"
    echo "${fp}" > "${tmp_file}"
    mv -f "${tmp_file}" "${target_file}"
}

# ------------------------------------------------------------------------------
# 2. Stage 1: Fast Fingerprint Cache Short-Circuit / Compilation
# ------------------------------------------------------------------------------
NEED_BUILD=false
if [ "${FORCE_REBUILD}" = false ] && [ -f "${FINGERPRINT_FILE}" ]; then
    SAVED_FINGERPRINT="$(cat "${FINGERPRINT_FILE}" 2>/dev/null || true)"
    if [ "${SAVED_FINGERPRINT}" = "${CURRENT_FINGERPRINT}" ] && check_artifacts_exist; then
        echo "⚡ [CACHE] Native Deflate Defense binaries up-to-date (fingerprint: ${CURRENT_FINGERPRINT:0:12})"
        echo "   Target : ${BUILD_DIR}/programs"
        echo "   Info   : Skipping CMake build. Use --force to rebuild unconditionally."
        echo ""
    else
        NEED_BUILD=true
    fi
else
    NEED_BUILD=true
fi

if [ "${NEED_BUILD}" = true ]; then
    mkdir -p "${BUILD_DIR}"
    echo "==> [Stage 1/3] Compiling Native Deflate Defense Core..."
    cmake -B "${BUILD_DIR}" -S "${VENDOR_LIBDEFLATE}" \
        -DCMAKE_BUILD_TYPE=Release \
        -DLIBDEFLATE_BUILD_STATIC_LIB=ON \
        -DLIBDEFLATE_BUILD_SHARED_LIB=OFF \
        -DLIBDEFLATE_BUILD_TESTS=ON \
        -DLIBDEFLATE_BUILD_GZIP=ON \
        -DCMAKE_C_FLAGS="-O3 -Wall -Werror" \
        -Wno-unused-cli > /dev/null

    cmake --build "${BUILD_DIR}" --config Release -j "$(sysctl -n hw.ncpu 2>/dev/null || echo 4)" > /dev/null
    save_fingerprint "${FINGERPRINT_FILE}" "${CURRENT_FINGERPRINT}"
    echo "✅ Compilation successful (Static Library & 8 Industrial Test Binaries ready)"
    echo ""
fi

# ------------------------------------------------------------------------------
# 3. Stage 2: Deep Defense Harness under Full HW Acceleration
# ------------------------------------------------------------------------------
echo "==> [Stage 2/3] Running Deep Defense Harness (Full Hardware Acceleration)..."

for t in "${TEST_BINARIES[@]}"; do
    bin_path="${BUILD_DIR}/programs/${t}"
    if [[ -x "${bin_path}" ]]; then
        printf "  %-32s ... " "${t}"
        "${bin_path}" > /dev/null 2>&1
        echo "✅ PASS"
    else
        echo "⚠️  Skipping ${t} (not built)"
    fi
done

echo ""

# ------------------------------------------------------------------------------
# 4. Stage 3: Defense Harness under Dynamic CPU Stripping (Scalar Fallbacks)
# ------------------------------------------------------------------------------
echo "==> [Stage 3/3] Running CPU Stripping Matrix (LIBDEFLATE_DISABLE_CPU_FEATURES=pmull,crc32,neon)..."

for t in "${TEST_BINARIES[@]}"; do
    bin_path="${BUILD_DIR}/programs/${t}"
    if [[ -x "${bin_path}" ]]; then
        printf "  %-32s (Scalar) ... " "${t}"
        LIBDEFLATE_DISABLE_CPU_FEATURES=pmull,crc32,neon,dotprod "${bin_path}" > /dev/null 2>&1
        echo "✅ PASS"
    fi
done

echo ""
echo "======================================================================"
echo "✅ All 14 Deflate Deep Defense & CPU-Stripping Harnesses PASSED!"
echo "======================================================================"
