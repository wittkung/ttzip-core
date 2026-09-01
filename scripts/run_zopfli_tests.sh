#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# ==============================================================================
# scripts/run_zopfli_tests.sh
# Automated Google Zopfli Industrial Test Suite & Invariant 6 Anti-Regression Gate Runner
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
GATE_MODE="${GATE_MODE:-0}"

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo "Options:"
    echo "  --release        Execute test binaries under release profile"
    echo "  --force, -f      Force full test execution regardless of cache"
    echo "  --gate           CI/Gate mode (skip redundant lib unit tests when already run by workspace runner)"
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
        --gate)
            GATE_MODE="1"
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
echo "📦 TTZip Zopfli Optimal Deflate Suite & Anti-Regression Gate Runner"
echo "   Working Directory: ${RUST_DIR}"
echo "   Profile:           $(if [ "${USE_RELEASE}" = true ]; then echo 'Release'; else echo 'Debug'; fi)"
echo "======================================================================"

PROFILE="$(if [ "${USE_RELEASE}" = true ]; then echo 'release'; else echo 'debug'; fi)"
CARGO_FLAGS=("--target" "aarch64-apple-darwin")
if [ "${USE_RELEASE}" = true ]; then
    CARGO_FLAGS+=("--release")
fi

# Canonical Zopfli test targets (2 test suites)
ZOPFLI_TEST_TARGETS=(
    "zopfli_fuzz_tests"
    "zopfli_performance_regression_tests"
)

TOTAL_TARGETS=${#ZOPFLI_TEST_TARGETS[@]}
CARGO_TEST_ARGS=()
for target in "${ZOPFLI_TEST_TARGETS[@]}"; do
    CARGO_TEST_ARGS+=("--test" "${target}")
done

BUILD_DIR="${RUST_DIR}/target/aarch64-apple-darwin/${PROFILE}/deps"
if [ ! -d "${BUILD_DIR}" ]; then
    BUILD_DIR="${RUST_DIR}/target/${PROFILE}/deps"
fi
ALL_BINS_EXIST=true
DIRECT_BINS=()
for target in "${ZOPFLI_TEST_TARGETS[@]}"; do
    target_bin=""
    for candidate in $(ls -t "${BUILD_DIR}/${target}-"* 2>/dev/null || true); do
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
    echo "--> Executing ${TOTAL_TARGETS} Zopfli test suites directly from pre-compiled binary cache..."
    for bin in "${DIRECT_BINS[@]}"; do
        if ! "${bin}" --nocapture; then
            echo "❌ Zopfli test suite failed: $(basename "${bin}")"
            exit 1
        fi
    done
else
    echo "--> Executing ${TOTAL_TARGETS} Zopfli test suites via unified test matrix..."
    if ! cargo test "${CARGO_FLAGS[@]}" -p ttzip-engine "${CARGO_TEST_ARGS[@]}" -- --nocapture; then
        echo "❌ One or more Zopfli test suites failed."
        exit 1
    fi
fi

echo ""
echo "======================================================================"
echo "🎉 ALL ${TOTAL_TARGETS}/${TOTAL_TARGETS} ZOPFLI TEST SUITES PASSED (INVARIANT 6 <= 3.0% OK)!"
echo "======================================================================"
