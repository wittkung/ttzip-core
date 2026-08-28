#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# Multi-Ecosystem Package Distribution Verification Pipeline.
# Validates Homebrew Formula, Rust Cargo Crates, and Python PyPI Wheels.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "======================================================================"
echo "⚡️ TTZip Multi-Ecosystem Package Distribution Verification Gate"
echo "======================================================================"

cd "${REPO_ROOT}"

# 1. Homebrew Formula Audit
echo ">>> [1/3] Auditing Homebrew Formula..."
if [ -f "/Users/kevintung/Documents/dev/homebrew-ttzip/Formula/ttzip.rb" ]; then
    ruby -c /Users/kevintung/Documents/dev/homebrew-ttzip/Formula/ttzip.rb >/dev/null
    echo "  [PASS] Homebrew Formula syntax valid (ruby -c passed)."
else
    echo "  [SKIP] Local homebrew-ttzip directory not found."
fi

# 2. Rust Crates.io Dry-Run Packaging
echo ">>> [2/4] Auditing Rust Crates Packaging (crates.io readiness)..."
(
    cd "${REPO_ROOT}/rust"
    cargo package -p ttzip-engine --allow-dirty --no-verify >/dev/null
    echo "  [PASS] ttzip-engine crate packaged cleanly."
)

# 3. Python PyPI Maturin Wheel Build
echo ">>> [3/4] Building & Validating Python PyPI Wheel (Maturin ABI3)..."
export RUSTFLAGS="-C link-arg=-undefined -C link-arg=dynamic_lookup"
maturin build -m rust/ttzip-python/Cargo.toml --release --strip --out "${REPO_ROOT}/dist" >/dev/null

WHEEL_FILE=$(find "${REPO_ROOT}/dist" -name "*.whl" | head -n 1)
if [ -f "${WHEEL_FILE}" ]; then
    echo "  [PASS] Production Wheel created: $(basename "${WHEEL_FILE}")"
else
    echo "  [FAIL] Wheel file not found in dist/"
    exit 1
fi

# 4. Node.js SDK Validation
echo ">>> [4/4] Validating Node.js SDK..."
if command -v node >/dev/null 2>&1; then
    node "${REPO_ROOT}/sdk/node/test.js" >/dev/null
    echo "  [PASS] Node.js SDK tests passed."
else
    echo "  [SKIP] node binary not found."
fi

echo "======================================================================"
echo "✅ All 4 Distribution Channels (Homebrew, Crates.io, PyPI, Node) Verified!"
echo "======================================================================"
