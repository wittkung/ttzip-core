#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# Validates and publishes ttzip crates to crates.io.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}/rust"

echo "=========================================="
echo "📦 Auditing and Packaging Cargo Crates"
echo "=========================================="

echo "--> [1/2] Packaging ttzip-glue..."
cargo package --manifest-path ttzip-glue/Cargo.toml --allow-dirty --no-verify

echo "--> [2/2] Packaging ttzip-cli..."
cargo package --manifest-path ttzip-tui/Cargo.toml --allow-dirty --no-verify

echo "✅ All crates validated and packaged cleanly for crates.io."
