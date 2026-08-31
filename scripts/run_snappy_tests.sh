#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# ==============================================================================
# scripts/run_snappy_tests.sh
# Automated Snappy Industrial Test Suite & Invariant 6 Anti-Regression Gate Runner
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
echo "📦 TTZip Snappy Industrial Suite & Anti-Regression Gate Runner"
echo "   Working Directory: ${RUST_DIR}"
echo "   Profile:           $(if [ "${USE_RELEASE}" = true ]; then echo 'Release'; else echo 'Debug'; fi)"
echo "======================================================================"

CARGO_FLAGS=()
if [ "${USE_RELEASE}" = true ]; then
    CARGO_FLAGS+=("--release")
fi

# Canonical Snappy test targets (12 test suites)
SNAPPY_TEST_TARGETS=(
    "snappy_tag_tests"
    "snappy_crc_tests"
    "snappy_raw_encoder_tests"
    "snappy_raw_decoder_tests"
    "snappy_frame_tests"
    "snappy_framed_reader_tests"
    "snappy_framed_writer_tests"
    "snappy_facade_tests"
    "snappy_defense_tests"
    "snappy_compliance_tests"
    "snappy_fuzz_tests"
    "snappy_performance_regression_tests"
)

TOTAL_TARGETS=${#SNAPPY_TEST_TARGETS[@]}
CARGO_TEST_ARGS=()
for target in "${SNAPPY_TEST_TARGETS[@]}"; do
    CARGO_TEST_ARGS+=("--test" "${target}")
done

cd "${RUST_DIR}"

BUILD_DIR="$(if [ "${USE_RELEASE}" = true ]; then echo "${RUST_DIR}/target/release/deps"; else echo "${RUST_DIR}/target/debug/deps"; fi)"
ALL_BINS_EXIST=true
DIRECT_BINS=()
for target in "${SNAPPY_TEST_TARGETS[@]}"; do
    target_bin=""
    for candidate in "${BUILD_DIR}/${target}-"*; do
        if [ -x "${candidate}" ] && [[ ! "${candidate}" =~ \.(d|dSYM)$ ]]; then
            target_bin="${candidate}"
            break
        fi
    done
    if [ -n "${target_bin}" ]; then
        DIRECT_BINS+=("${target_bin}")
    else
        ALL_BINS_EXIST=false
        break
    fi
done

if [ "${ALL_BINS_EXIST}" = true ] && [ "${FORCE_REBUILD}" = false ]; then
    echo "--> Executing ${TOTAL_TARGETS} Snappy test suites directly from pre-compiled binary cache..."
    for bin in "${DIRECT_BINS[@]}"; do
        if ! "${bin}" --nocapture; then
            echo "❌ Snappy test suite failed: $(basename "${bin}")"
            exit 1
        fi
    done
else
    echo "--> Executing ${TOTAL_TARGETS} Snappy test suites via unified test matrix..."
    if ! cargo test "${CARGO_FLAGS[@]}" -p ttzip-engine "${CARGO_TEST_ARGS[@]}" -- --nocapture; then
        echo "❌ One or more Snappy test suites failed."
        exit 1
    fi
fi

echo ""
echo "======================================================================"
echo "🎉 ALL ${TOTAL_TARGETS}/${TOTAL_TARGETS} SNAPPY TEST SUITES PASSED (INVARIANT 6 <= 3.0% OK)!"
echo "======================================================================"
