#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${ROOT_DIR}"

echo "================================================================"
echo "  TTZip Automated Code Standards & Zero-Warning Lint Latch"
echo "================================================================"

# 1. Check for SPDX Copyright Headers and Tier Compliance
echo "▶ [Step 1/3] Verifying SPDX License & Copyright Headers on TTZip sources..."
python3 "${SCRIPT_DIR}/audit_licenses.py" --dir Sources --license LICENSE-BSD
python3 "${SCRIPT_DIR}/audit_licenses.py" --dir Tests --license LICENSE-BSD
python3 "${SCRIPT_DIR}/clean_license_headers.py" --check --repo-root .

echo "✅ SPDX Headers & License Tiers 100% verified on all TTZip-authored files."

# 2. Check for Zero Non-ASCII in C Bridge and Native Deflate
echo "▶ [Step 2/3] Verifying 100% Libarchive English in Native C & Core Bridge..."
NON_ASCII_C=$(python3 -c '
import os, glob, sys

c_files = glob.glob("Sources/CTTZipBridge/*.c") + glob.glob("Sources/CTTZipBridge/*.h") + glob.glob("Sources/CTTZipBridge/include/*.h")
has_err = False
for f in c_files:
    if os.path.isfile(f) and not f.endswith("ttzip_engineFFI.h"):
        with open(f, "rb") as fp:
            for idx, line in enumerate(fp, 1):
                try:
                    line.decode("ascii")
                except UnicodeDecodeError:
                    print(f"{f}:{idx}: contains non-ASCII characters")
                    has_err = True
if has_err:
    sys.exit(1)
' || true)
if [ -n "$NON_ASCII_C" ]; then
    echo "❌ Found non-ASCII characters in C bridge files:"
    echo "$NON_ASCII_C"
    exit 1
fi
echo "✅ C Bridge 100% English Doxygen compliant."

# 3. Verify Zero-Warning Build
echo "▶ [Step 3/3] Compiling Targets & Tests with -warnings-as-errors..."
swift build --build-tests -Xswiftc -warnings-as-errors

echo "================================================================"
echo "🎉 ALL CODEBASE STANDARDS & ZERO-WARNING GATES PASSED (100% OK)"
echo "================================================================"
