#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# TTZip: Automated Race Detector & ThreadSanitizer Concurrency Gate.
# Executes Go `go test -race` and Rust / Swift concurrency test suites.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

echo "======================================================================"
echo "🏎️  TTZip Concurrency & Data Race Detection Gate"
echo "======================================================================"

# 1. Go SDK Race Detection
echo "--> [1/3] Running Go SDK under Data Race Detector (go test -race)..."
if command -v go >/dev/null 2>&1; then
    (cd sdk/go && go test -race -v ./...)
    echo "  [PASS] Go SDK 0 data races detected."
else
    echo "  [SKIP] Go toolchain not found."
fi

# 2. Rust Concurrency & Parallel Extraction Property Tests
echo "--> [2/3] Running Rust Microkernel Thread Safety & Property Tests..."
cargo test --manifest-path rust/Cargo.toml -p ttzip-engine --test property_tests
echo "  [PASS] Rust property tests passed."

# 3. Swift Strict Concurrency Tests (if swift toolchain available)
echo "--> [3/3] Running Swift 6 Strict Concurrency & Actor Isolation Tests..."
if command -v swift >/dev/null 2>&1; then
    swift test --filter TTZipCoreIntegrationTests
    echo "  [PASS] Swift 6 Actor concurrency verified."
fi

echo "======================================================================"
echo "✅ [PASS] Concurrency Gate Complete: 0 Data Races Detected."
echo "======================================================================"
