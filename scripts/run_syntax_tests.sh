#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# ==============================================================================
# scripts/run_syntax_tests.sh
# Automated Tree-sitter Syntax, AST Fuzzing & Invariant 6 Anti-Regression Runner
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
echo "📦 TTZip Tree-sitter Syntax Suite & Anti-Regression Gate Runner"
echo "   Working Directory: ${RUST_DIR}"
echo "   Profile:           $(if [ "${USE_RELEASE}" = true ]; then echo 'Release'; else echo 'Debug'; fi)"
echo "======================================================================"

CARGO_FLAGS=()
if [ "${USE_RELEASE}" = true ]; then
    CARGO_FLAGS+=("--release")
fi

# 1. In-crate library unit tests
echo "--> Executing in-crate syntax module unit tests..."
cargo test "${CARGO_FLAGS[@]}" -p ttzip-engine --lib standards::syntax_highlight -- --nocapture
cargo test "${CARGO_FLAGS[@]}" -p ttzip-engine --lib uniffi_api::syntax -- --nocapture
cargo test "${CARGO_FLAGS[@]}" -p ttzip-engine --lib syntax -- --nocapture

# 2. Integration test targets
SYNTAX_TEST_TARGETS=(
    "syntax_fuzz_tests"
    "syntax_performance_regression_tests"
)

TOTAL_TARGETS=${#SYNTAX_TEST_TARGETS[@]}
CARGO_TEST_ARGS=()
for target in "${SYNTAX_TEST_TARGETS[@]}"; do
    CARGO_TEST_ARGS+=("--test" "${target}")
done

BUILD_DIR="$(if [ "${USE_RELEASE}" = true ]; then echo "${RUST_DIR}/target/release/deps"; else echo "${RUST_DIR}/target/debug/deps"; fi)"
ALL_BINS_EXIST=true
DIRECT_BINS=()
for target in "${SYNTAX_TEST_TARGETS[@]}"; do
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
    echo "--> Executing ${TOTAL_TARGETS} Syntax test suites directly from pre-compiled binary cache..."
    for bin in "${DIRECT_BINS[@]}"; do
        if ! "${bin}" --nocapture; then
            echo "❌ Syntax test suite failed: $(basename "${bin}")"
            exit 1
        fi
    done
else
    echo "--> Executing ${TOTAL_TARGETS} Syntax test suites via unified test matrix..."
    if ! cargo test "${CARGO_FLAGS[@]}" -p ttzip-engine "${CARGO_TEST_ARGS[@]}" -- --nocapture; then
        echo "❌ One or more Syntax test suites failed."
        exit 1
    fi
fi

echo ""
echo "======================================================================"
echo "🎉 ALL ${TOTAL_TARGETS}/${TOTAL_TARGETS} SYNTAX TEST SUITES PASSED (INVARIANT 6 <= 3.0% OK)!"
echo "======================================================================"
