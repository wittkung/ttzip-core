#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# Builds TTZip native PyO3 C-extension and installs it into local python/ttzip package.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "======================================================================"
echo "⚡️ Building TTZip Native Python C-Extension (PyO3 ABI3)"
echo "======================================================================"

cd "${REPO_ROOT}"

# 1. Build release library via Cargo with dynamic lookup for PyO3 on macOS
export RUSTFLAGS="-C link-arg=-undefined -C link-arg=dynamic_lookup"
cargo build --release --manifest-path rust/Cargo.toml -p ttzip-python

# 2. Locate built dylib and copy as _ttzip.so for direct local import
TARGET_LIB="${REPO_ROOT}/rust/target/release/lib_ttzip.dylib"
if [ ! -f "${TARGET_LIB}" ]; then
    TARGET_LIB="${REPO_ROOT}/rust/target/release/lib_ttzip.so"
fi

if [ -f "${TARGET_LIB}" ]; then
    cp "${TARGET_LIB}" "${REPO_ROOT}/python/ttzip/_ttzip.so"
    if command -v codesign >/dev/null 2>&1; then
        codesign --force --sign - "${REPO_ROOT}/python/ttzip/_ttzip.so" >/dev/null 2>&1 || true
    fi
    echo "✅ Successfully installed & signed native module to python/ttzip/_ttzip.so"
else
    echo "❌ Error: Could not locate built dynamic library at ${TARGET_LIB}"
    exit 1
fi
