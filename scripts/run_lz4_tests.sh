#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# ==============================================================================
# scripts/run_lz4_tests.sh
# Automated LZ4 Industrial Test Suite & Invariant 6 Anti-Regression Gate Runner
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CORE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
RUST_DIR="${CORE_DIR}/rust"

export PATH="$HOME/.cargo/bin:$PATH"

if command -v sccache >/dev/null 2>&1; then
    export RUSTC_WRAPPER="sccache"
fi

NCPU="$(sysctl -n hw.ncpu 2>/dev/null || echo 8)"
export RUST_TEST_THREADS="${NCPU}"
export RAYON_NUM_THREADS="2"

USE_RELEASE=false
FORCE_REBUILD=false

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo "Options:"
    echo "  --release        Execute test binaries under release profile"
    echo "  --force, -f      Force full test execution regardless of cache"
    echo "  --help, -h       Show this help message"
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --release)
            USE_RELEASE=true
            shift
            ;;
        --force|-f)
            FORCE_REBUILD=true
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

cd "${RUST_DIR}"

echo "======================================================================"
echo "📦 TTZip LZ4 Industrial Suite & Anti-Regression Gate Runner"
echo "   Working Directory: ${RUST_DIR}"
echo "   Profile:           $(if [ "${USE_RELEASE}" = true ]; then echo 'Release'; else echo 'Debug'; fi)"
echo "======================================================================"

CARGO_FLAGS=()
if [ "${USE_RELEASE}" = true ]; then
    CARGO_FLAGS+=("--release")
fi

# Canonical LZ4 test targets (12 test suites)
LZ4_TEST_TARGETS=(
    "lz4_constants_and_frame_desc_tests"
    "lz4_hash_and_lut_tests"
    "lz4_frametest_cascade_tests"
    "lz4_corrupted_fuzzer_tests"
    "lz4_decompress_wildcopy_tests"
    "lz4_matchfinder_tests"
    "lz4_hc_optimal_tests"
    "lz4_streaming_frame_tests"
    "lz4_partial_decompression_tests"
    "lz4_dictionary_modes_tests"
    "lz4_single_byte_streaming_tests"
    "lz4_performance_regression_tests"
)

TOTAL_TARGETS=${#LZ4_TEST_TARGETS[@]}

echo "--> Pre-building ${TOTAL_TARGETS} LZ4 test suites..."
CARGO_TEST_ARGS=()
for target in "${LZ4_TEST_TARGETS[@]}"; do
    CARGO_TEST_ARGS+=("--test" "${target}")
done

cd "${RUST_DIR}"

cargo test "${CARGO_FLAGS[@]}" -p ttzip-engine "${CARGO_TEST_ARGS[@]}" --no-run

BUILD_DIR="$(if [ "${USE_RELEASE}" = true ]; then echo "${RUST_DIR}/target/release/deps"; else echo "${RUST_DIR}/target/debug/deps"; fi)"
MAX_CONCURRENT=4
PIDS=()
LOGS=()
TARGET_NAMES=()
FAILED=0

echo "--> Executing ${TOTAL_TARGETS} LZ4 test suites concurrently (pool size: ${MAX_CONCURRENT})..."
for target in "${LZ4_TEST_TARGETS[@]}"; do
    bin="$(ls -t "${BUILD_DIR}/${target}-"* 2>/dev/null | grep -v '\.d$' | grep -v '\.dSYM' | head -n 1 || true)"
    log="$(mktemp)"
    if [ -x "${bin}" ]; then
        "${bin}" --nocapture > "${log}" 2>&1 &
    else
        cargo test "${CARGO_FLAGS[@]}" -p ttzip-engine --test "${target}" -- --nocapture > "${log}" 2>&1 &
    fi
    PIDS+=("$!")
    LOGS+=("${log}")
    TARGET_NAMES+=("${target}")

    if [ ${#PIDS[@]} -ge ${MAX_CONCURRENT} ]; then
        pid="${PIDS[0]}"
        log="${LOGS[0]}"
        tname="${TARGET_NAMES[0]}"
        if ! wait "${pid}"; then
            echo "❌ [FAILED] ${tname}"
            cat "${log}"
            FAILED=1
        fi
        rm -f "${log}"
        PIDS=("${PIDS[@]:1}")
        LOGS=("${LOGS[@]:1}")
        TARGET_NAMES=("${TARGET_NAMES[@]:1}")
    fi
done

for i in "${!PIDS[@]}"; do
    pid="${PIDS[$i]}"
    log="${LOGS[$i]}"
    tname="${TARGET_NAMES[$i]}"
    if ! wait "${pid}"; then
        echo "❌ [FAILED] ${tname}"
        cat "${log}"
        FAILED=1
    fi
    rm -f "${log}"
done

if [ ${FAILED} -ne 0 ]; then
    echo "❌ One or more LZ4 test suites failed."
    exit 1
fi

echo ""
echo "======================================================================"
echo "🎉 ALL ${TOTAL_TARGETS}/${TOTAL_TARGETS} LZ4 TEST SUITES PASSED (INVARIANT 6 <= 3.0% OK)!"
echo "======================================================================"
