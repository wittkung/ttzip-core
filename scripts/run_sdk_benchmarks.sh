#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# TTZip: Cross-Language Silesia Corpus Throughput & Memory Benchmark Gate.
# Compares Rust, C11, C++20, Go, Python, Swift, and Java 22+ Panama FFM.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

OUT_MD="${REPO_ROOT}/BENCHMARK_MATRIX.md"
CORPUS_DIR="${REPO_ROOT}/tests/TTZipTests/Fixtures/Silesia"
ITERATIONS=2

while [[ $# -gt 0 ]]; do
    case "$1" in
        --output=*)
            OUT_MD="${1#*=}"
            shift
            ;;
        --output)
            OUT_MD="$2"
            shift 2
            ;;
        --corpus=*)
            CORPUS_DIR="${1#*=}"
            shift
            ;;
        --corpus)
            CORPUS_DIR="$2"
            shift 2
            ;;
        --iterations=*)
            ITERATIONS="${1#*=}"
            shift
            ;;
        --iterations)
            ITERATIONS="$2"
            shift 2
            ;;
        *)
            shift
            ;;
    esac
done

echo "======================================================================"
echo "⚡️ TTZip Cross-Language SDK Silesia Benchmark Gate"
echo "======================================================================"

# Ensure all SDK test CLIs are built
echo "--> [1/2] Verifying and building headless SDK test runners..."
python3 tests/security/sdk_drivers.py >/dev/null 2>&1 || true

# Run benchmarks
echo "--> [2/2] Running Silesia benchmark suite (${ITERATIONS} iterations)..."
python3 scripts/sdk_benchmark_runner.py \
    --corpus "${CORPUS_DIR}" \
    --output "${OUT_MD}" \
    --iterations "${ITERATIONS}"

echo "======================================================================"
echo "✅ [PASS] Cross-Language Benchmark Matrix Generated at: ${OUT_MD}"
echo "======================================================================"
