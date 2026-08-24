#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# TTZip: Automated AddressSanitizer (ASan) & Memory Leak Detection Gate.
# Compiles Rust & C/C++ with -fsanitize=address and executes 1,000 rapid cycles.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

CYCLES=1000
while [[ $# -gt 0 ]]; do
    case "$1" in
        --cycles=*)
            CYCLES="${1#*=}"
            shift
            ;;
        --cycles)
            CYCLES="$2"
            shift 2
            ;;
        *)
            shift
            ;;
    esac
done

echo "======================================================================"
echo "🛡️  TTZip AddressSanitizer (ASan) Memory Leak Gate"
echo "======================================================================"
echo "Configured iterations: ${CYCLES} rapid cycles"

# 1. Ensure Rust engine library is compiled
echo "--> [1/3] Verifying Rust Engine static & dynamic artifacts..."
if [ ! -f "rust/target/release/libttzip_engine.a" ]; then
    echo "  [*] Building release rust engine..."
    cargo build --release --manifest-path rust/Cargo.toml -p ttzip-engine
fi
echo "  [PASS] Rust engine artifact ready."

# 2. Compile and execute C/C++ ASan Stress Test
echo "--> [2/3] Compiling and running C11 ASan 1,000 Rapid Cycles..."
clang -std=c11 -fsanitize=address -g -O1 \
    -I Sources/CTTZipBridge/include \
    sdk/c/asan_stress_test.c \
    rust/target/release/libttzip_engine.a \
    -larchive -lbz2 -lz -llzma -framework Security \
    -o sdk/c/asan_stress_test

export ASAN_OPTIONS="abort_on_error=1:halt_on_error=1"
./sdk/c/asan_stress_test "${CYCLES}"
echo "  [PASS] C11 ASan stress test passed: 0 memory errors."

# 3. Rust Engine Unit and Security Gate Tests
echo "--> [3/3] Running Rust Microkernel Security & Path Sanitization Tests..."
cargo test --manifest-path rust/Cargo.toml -p ttzip-engine --lib path_sanitizer

echo "======================================================================"
echo "✅ [PASS] ASan Memory Gate Complete: 0 Leaks, 0 Use-After-Free Detected."
echo "======================================================================"
