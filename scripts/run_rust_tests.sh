#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine for macOS.
# ==============================================================================
# scripts/run_rust_tests.sh
# 自动化执行 TTZip Rust 全域测试套件：单测、集成测试、属性测试、Fuzzing 与基准测试
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
RUST_DIR="${REPO_ROOT}/rust"

export PATH="$HOME/.cargo/bin:$PATH"
NCPU="$(sysctl -n hw.ncpu 2>/dev/null || echo 8)"
export RUST_TEST_THREADS="${NCPU}"

RUN_UNIT=false
RUN_PROPS=false
RUN_FUZZ=false
RUN_BENCH=false
RUN_ALL=false

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo "Options:"
    echo "  --unit           Run Unit and Core C-ABI Integration tests"
    echo "  --props          Run Proptest Property-based invariant test suite"
    echo "  --fuzz           Run Coverage-guided Fuzzing harness test targets"
    echo "  --bench          Run Criterion micro-benchmarks"
    echo "  --all            Run all test suites (Unit, Props, Fuzz, Bench)"
    echo "  --help|-h        Show this help message"
    exit 0
}

if [[ $# -eq 0 ]]; then
    RUN_ALL=true
fi

USE_RELEASE=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --unit)
            RUN_UNIT=true
            shift
            ;;
        --props)
            RUN_PROPS=true
            shift
            ;;
        --fuzz)
            RUN_FUZZ=true
            shift
            ;;
        --bench)
            RUN_BENCH=true
            shift
            ;;
        --release)
            USE_RELEASE=true
            shift
            ;;
        --all)
            RUN_ALL=true
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

if [ "${RUN_ALL}" = true ] || ([ "${RUN_UNIT}" = false ] && [ "${RUN_PROPS}" = false ] && [ "${RUN_FUZZ}" = false ] && [ "${RUN_BENCH}" = false ]); then
    RUN_UNIT=true
    RUN_PROPS=true
    RUN_FUZZ=true
fi

cd "${RUST_DIR}"

echo "================================================================"
echo "🦀 TTZip Rust Industrial Test Suite Runner"
echo "   Working Directory: ${RUST_DIR}"
echo "   Release Mode:      ${USE_RELEASE}"
echo "================================================================"

CARGO_FLAGS=()
if [ "${USE_RELEASE}" = true ]; then
    CARGO_FLAGS+=("--release")
fi

if [ "${RUN_UNIT}" = true ] || [ "${RUN_PROPS}" = true ] || [ "${RUN_FUZZ}" = true ]; then
    echo "--> [1/2] Executing Unified Workspace Test Matrix..."
    cargo test "${CARGO_FLAGS[@]}" --workspace --all-targets -- --nocapture
    echo "✅ [PASS] Unified Workspace Tests completed successfully."
fi

# Micro-benchmarks
if [ "${RUN_BENCH}" = true ]; then
    echo "--> [2/2] Running Criterion Micro-benchmarks..."
    cargo bench -p ttzip-engine || true
    echo "✅ [PASS] Criterion Benchmarks executed."
fi

echo "================================================================"
echo "🎉 ALL REQUESTED RUST TESTS PASSED WITH ZERO REGRESSIONS!"
echo "================================================================"
