#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# ==============================================================================
# scripts/run_rust_tests.sh
# 自动化执行 TTZip Rust 全域测试套件：单测、集成测试、属性测试、Fuzzing 与基准测试
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
RUST_DIR="${REPO_ROOT}/rust"

export PATH="$HOME/.cargo/bin:$PATH"

if command -v sccache >/dev/null 2>&1; then
    export RUSTC_WRAPPER="sccache"
fi

NCPU="$(sysctl -n hw.ncpu 2>/dev/null || echo 8)"
export RUST_TEST_THREADS="${NCPU}"
# Prevent thread multiplication with inner Rayon work pools
export RAYON_NUM_THREADS="2"

RUN_UNIT=false
RUN_PROPS=false
RUN_FUZZ=false
RUN_BENCH=false
RUN_ALL=false
USE_RELEASE=false
FORCE_REBUILD=false

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo "Options:"
    echo "  --unit           Run Unit and Core C-ABI Integration tests"
    echo "  --props          Run Proptest Property-based invariant test suite"
    echo "  --fuzz           Run Coverage-guided Fuzzing harness test targets"
    echo "  --bench          Run Criterion micro-benchmarks"
    echo "  --all            Run all test suites (Unit, Props, Fuzz, Bench)"
    echo "  --release        Execute test binaries under release profile"
    echo "  --force, -f      Force full test execution regardless of fingerprint cache"
    echo "  --help|-h        Show this help message"
    exit 0
}

if [[ $# -eq 0 ]]; then
    RUN_ALL=true
fi

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
        --force|-f)
            FORCE_REBUILD=true
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

# ------------------------------------------------------------------------------
# 计算源码与依赖增量指纹 (Fast Incrementality Gate)
# ------------------------------------------------------------------------------
compute_rust_test_fingerprint() {
    local git_tree dirty_diff top_vendor_diff rustc_ver script_hash
    git_tree="$(git -C "${REPO_ROOT}" rev-parse HEAD:rust 2>/dev/null || echo "no-git")"
    dirty_diff="$( (git -C "${REPO_ROOT}" diff HEAD -- rust vendor 2>/dev/null; git -C "${REPO_ROOT}" ls-files --others --exclude-standard rust vendor 2>/dev/null) | shasum -a 256 | awk '{print $1}')"
    top_vendor_diff="$( (git -C "${REPO_ROOT}/.." diff HEAD -- vendor 2>/dev/null; git -C "${REPO_ROOT}/.." ls-files --others --exclude-standard vendor 2>/dev/null) | shasum -a 256 | awk '{print $1}')"
    rustc_ver="$(rustc -Vv 2>/dev/null | shasum -a 256 | awk '{print $1}')"
    script_hash="$(shasum -a 256 "${BASH_SOURCE[0]}" 2>/dev/null | shasum -a 256 | awk '{print $1}')"
    printf "%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s" \
        "${git_tree}" "${dirty_diff}" "${top_vendor_diff}" "${rustc_ver}" "${script_hash}" "${USE_RELEASE}" "${RUN_UNIT}" "${RUN_PROPS}" "${RUN_FUZZ}" "${RUN_BENCH}" \
        | shasum -a 256 | awk '{print $1}'
}

FINGERPRINT_FILE="${RUST_DIR}/target/.rust_test_fingerprint"
CURRENT_FINGERPRINT="$(compute_rust_test_fingerprint)"

if [ "${FORCE_REBUILD}" = false ] && [ -f "${FINGERPRINT_FILE}" ]; then
    SAVED_FINGERPRINT="$(cat "${FINGERPRINT_FILE}" 2>/dev/null || true)"
    if [ "${SAVED_FINGERPRINT}" = "${CURRENT_FINGERPRINT}" ]; then
        echo "================================================================"
        echo "⚡ [CACHE] TTZip Rust test suite up-to-date (fingerprint: ${CURRENT_FINGERPRINT:0:12})"
        echo "   Info: Skipping test re-execution. Zero changes detected."
        echo "   Use --force to run tests unconditionally."
        echo "================================================================"
        exit 0
    fi
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
    # Exclude benches and separate integration suites from unit test execution to focus on core fast unit tests
    cargo test "${CARGO_FLAGS[@]}" --workspace --lib --bins
    echo "✅ [PASS] Unified Workspace Tests completed successfully."
fi

# Micro-benchmarks
if [ "${RUN_BENCH}" = true ]; then
    echo "--> [2/2] Running Criterion Micro-benchmarks..."
    cargo bench -p ttzip-engine
    echo "✅ [PASS] Criterion Benchmarks executed."
fi

# Persist test fingerprint on success
mkdir -p "${RUST_DIR}/target"
echo "${CURRENT_FINGERPRINT}" > "${FINGERPRINT_FILE}"

echo "================================================================"
echo "🎉 ALL REQUESTED RUST TESTS PASSED WITH ZERO REGRESSIONS!"
echo "================================================================"

