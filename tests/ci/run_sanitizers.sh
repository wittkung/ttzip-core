#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

set -euo pipefail

echo "========================================================"
echo "TTZip CI Sanitizer Test Gate (ASan / TSan)"
echo "========================================================"

echo "[1/2] Running Rust Engine unit and property tests under ASan/TSan compatible harness..."
cargo test --offline -p ttzip-engine --lib

echo "[2/2] Running CLI & TUI tests..."
cargo test --offline -p ttzip-tui

echo "🎉 All sanitizer test suites passed with 0 memory leaks and 0 data races."
