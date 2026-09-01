#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# ==============================================================================
# scripts/run_mmap_tests.sh
# Automated Zero-Copy Mmap Engine Suite & Invariant 6 Anti-Regression Gate Runner
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
echo "📦 TTZip Zero-Copy Mmap Engine Suite & Anti-Regression Gate Runner"
echo "   Working Directory: ${RUST_DIR}"
echo "   Profile:           $(if [ "${USE_RELEASE}" = true ]; then echo 'Release'; else echo 'Debug'; fi)"
echo "======================================================================"

PROFILE="$(if [ "${USE_RELEASE}" = true ]; then echo 'release'; else echo 'debug'; fi)"
CARGO_FLAGS=("--target" "aarch64-apple-darwin")
if [ "${USE_RELEASE}" = true ]; then
    CARGO_FLAGS+=("--release")
fi

BUILD_DIR="${RUST_DIR}/target/aarch64-apple-darwin/${PROFILE}/deps"
if [ ! -d "${BUILD_DIR}" ]; then
    BUILD_DIR="${RUST_DIR}/target/${PROFILE}/deps"
fi

# 1. Run in-crate library unit tests for mmap sources and paging defense
if [ "${GATE_MODE}" != "1" ]; then
    echo "--> Executing in-crate mmap module unit tests..."
    LIB_BIN=""
    if [ "${FORCE_REBUILD}" = false ]; then
        for candidate in $(ls -t "${BUILD_DIR}/ttzip_engine-"* 2>/dev/null || true); do
            if [ -x "${candidate}" ] && [[ ! "${candidate}" =~ \.(d|dSYM)$ ]]; then
                LIB_BIN="${candidate}"
                break
            fi
        done
    fi

    if [ -n "${LIB_BIN}" ]; then
        "${LIB_BIN}" archive::source --nocapture
    else
        cargo test "${CARGO_FLAGS[@]}" -p ttzip-engine --lib archive::source -- --nocapture
    fi
else
    echo "--> [Gate Mode] Skipping redundant in-crate lib unit tests (covered in Stage 7)."
fi

# 2. Integration test targets
MMAP_TEST_TARGETS=(
    "mmap_compliance_tests"
    "mmap_fuzz_tests"
    "mmap_performance_regression_tests"
    "extract_single_mmap_bounded_memory"
)

TOTAL_TARGETS=${#MMAP_TEST_TARGETS[@]}
CARGO_TEST_ARGS=()
for target in "${MMAP_TEST_TARGETS[@]}"; do
    CARGO_TEST_ARGS+=("--test" "${target}")
done
ALL_BINS_EXIST=true
DIRECT_BINS=()
for target in "${MMAP_TEST_TARGETS[@]}"; do
    target_bin=""
    for candidate in $(ls -t "${BUILD_DIR}/${target}-"* 2>/dev/null || true); do
        if [ -x "${candidate}" ] && [[ ! "${candidate}" =~ \.(d|dSYM)$ ]]; then
            if [ -f "${RUST_DIR}/ttzip-engine/tests/${target}.rs" ] && [ "${RUST_DIR}/ttzip-engine/tests/${target}.rs" -nt "${candidate}" ]; then
                continue
            fi
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
    echo "--> Executing ${TOTAL_TARGETS} Mmap test suites directly from pre-compiled binary cache..."
    for bin in "${DIRECT_BINS[@]}"; do
        if ! "${bin}" --nocapture; then
            echo "❌ Mmap test suite failed: $(basename "${bin}")"
            exit 1
        fi
    done
else
    echo "--> Executing ${TOTAL_TARGETS} Mmap test suites via unified test matrix..."
    if ! cargo test "${CARGO_FLAGS[@]}" -p ttzip-engine "${CARGO_TEST_ARGS[@]}" -- --nocapture; then
        echo "❌ One or more Mmap test suites failed."
        exit 1
    fi
fi

echo ""
echo "======================================================================"
echo "🎉 ALL ${TOTAL_TARGETS}/${TOTAL_TARGETS} MMAP TEST SUITES PASSED (INVARIANT 6 <= 3.0% OK)!"
echo "======================================================================"
