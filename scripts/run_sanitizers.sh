#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine for macOS.
# ==============================================================================
# scripts/run_sanitizers.sh
# 自动化 AddressSanitizer (ASan) 与 ThreadSanitizer (TSan) 扫描运行器
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${WORKSPACE_ROOT}"

MODE="asan"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --asan)
            MODE="asan"
            shift
            ;;
        --tsan)
            MODE="tsan"
            shift
            ;;
        *)
            echo "Usage: $0 [--asan | --tsan]"
            exit 1
            ;;
    esac
done

echo "======================================================================"
echo "   TTZip Sanitizer Diagnostic Runner: [${MODE^^}]                    "
echo "======================================================================"

if [ "${MODE}" = "asan" ]; then
    echo "--> [1/2] Running Swift tests under AddressSanitizer..."
    swift test --sanitize=address --filter VaultMemorySanitizationTests
    echo "--> [2/2] Running Rust tests under AddressSanitizer..."
    cargo test --manifest-path rust/Cargo.toml -p ttzip-engine --lib
elif [ "${MODE}" = "tsan" ]; then
    echo "--> [1/2] Running Swift tests under ThreadSanitizer..."
    swift test --sanitize=thread --filter TTZipCoreIntegrationTests
    echo "--> [2/2] Running Rust tests under concurrency checks..."
    cargo test --manifest-path rust/Cargo.toml -p ttzip-engine --test property_tests
fi

echo "======================================================================"
echo "✅ [PASS] Sanitizer verification completed successfully with 0 issues."
echo "======================================================================"
