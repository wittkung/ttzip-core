#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

set -euo pipefail

echo "========================================================"
echo "TTZip CI Clean Sandbox SDK Test (Zero Subprocess Gate)"
echo "========================================================"

# Verify Java and Dart SDKs do NOT invoke Process/ProcessBuilder
echo "[1/2] Auditing Java SDK source for ProcessBuilder..."
if grep -rn "ProcessBuilder" core/sdk/jvm/src/main/java/; then
    echo "❌ FATAL: Found ProcessBuilder in Java SDK source!"
    exit 1
fi
echo "✅ Java SDK is clean of ProcessBuilder."

echo "[2/2] Auditing Dart SDK source for Process.run..."
if grep -rn "Process\.run" core/sdk/dart/lib/; then
    echo "❌ FATAL: Found Process.run in Dart SDK source!"
    exit 1
fi
echo "✅ Dart SDK is clean of Process.run."

echo "🎉 All SDKs strictly adhere to the Zero-Subprocess Native Policy."
