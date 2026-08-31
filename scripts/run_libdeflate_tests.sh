#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# ==============================================================================
# scripts/run_libdeflate_tests.sh
# Automated Libdeflate Industrial Test Suite & Invariant 6 Anti-Regression Gate Runner
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
echo "📦 TTZip Libdeflate Industrial Suite & Anti-Regression Gate Runner"
echo "   Working Directory: ${RUST_DIR}"
echo "   Profile:           $(if [ "${USE_RELEASE}" = true ]; then echo 'Release'; else echo 'Debug'; fi)"
echo "======================================================================"

CARGO_FLAGS=()
if [ "${USE_RELEASE}" = true ]; then
    CARGO_FLAGS+=("--release")
fi

# Canonical Libdeflate test targets (12 test suites)
LIBDEFLATE_TEST_TARGETS=(
    "libdeflate_matchfinder_tests"
    "libdeflate_bt_tests"
    "libdeflate_huffman_tests"
    "libdeflate_checksum_tests"
    "libdeflate_decompress_tests"
    "libdeflate_container_tests"
    "libdeflate_streaming_tests"
    "libdeflate_facade_tests"
    "libdeflate_defense_tests"
    "libdeflate_compliance_tests"
    "libdeflate_fuzz_tests"
    "libdeflate_performance_regression_tests"
)

TOTAL_TARGETS=${#LIBDEFLATE_TEST_TARGETS[@]}

echo "--> Precompiling ${TOTAL_TARGETS} Libdeflate test suites..."
CARGO_TEST_ARGS=()
for target in "${LIBDEFLATE_TEST_TARGETS[@]}"; do
    CARGO_TEST_ARGS+=("--test" "${target}")
done

cd "${RUST_DIR}"

cargo test "${CARGO_FLAGS[@]}" -p ttzip-engine "${CARGO_TEST_ARGS[@]}" --no-run --quiet

BUILD_DIR="$(if [ "${USE_RELEASE}" = true ]; then echo "${RUST_DIR}/target/release/deps"; else echo "${RUST_DIR}/target/debug/deps"; fi)"
MAX_CONCURRENT=4
PIDS=()
LOGS=()
TARGET_NAMES=()
FAILED=0

echo "--> Executing ${TOTAL_TARGETS} Libdeflate test suites concurrently (pool size: ${MAX_CONCURRENT})..."
for target in "${LIBDEFLATE_TEST_TARGETS[@]}"; do
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
    echo "❌ One or more Libdeflate test suites failed."
    exit 1
fi

echo ""
echo "======================================================================"
echo "🎉 ALL ${TOTAL_TARGETS}/${TOTAL_TARGETS} LIBDEFLATE TEST SUITES PASSED (INVARIANT 6 <= 3.0% OK)!"
echo "======================================================================"
