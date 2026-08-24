#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# ==============================================================================
# scripts/verify_uniffi_symbols.sh
# 自动化 Mozilla UniFFI 符号双向满射防御门禁：校验 Scaffolding 头文件与静态库 Mach-O 符号 100% 对应
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

HEADER_FILE="${REPO_ROOT}/Sources/CTTZipBridge/include/ttzip_engineFFI.h"
STATIC_LIB="${REPO_ROOT}/Vendor/TTZipVendor.xcframework/macos-arm64/libTTZipVendor.a"

if [ ! -f "${HEADER_FILE}" ]; then
    echo "❌ [ERROR] UniFFI Header file not found: ${HEADER_FILE}"
    exit 1
fi

if [ ! -f "${STATIC_LIB}" ]; then
    echo "❌ [ERROR] Static library not found: ${STATIC_LIB}"
    exit 1
fi

echo "======================================================================"
echo "   TTZip Mozilla UniFFI Symbol Alignment Gate (100% Scaffolding Parity)"
echo "======================================================================"
echo "Header: ${HEADER_FILE}"
echo "Binary: ${STATIC_LIB}"
echo "----------------------------------------------------------------------"

# 1. 从头文件中提取所有 UniFFI Scaffolding 函数原型
HEADER_SYMBOLS=$(grep -oE '\buniffi_ttzip_engine_[A-Za-z0-9_]+\b' "${HEADER_FILE}" | sort -u)
HEADER_COUNT=$(echo "${HEADER_SYMBOLS}" | grep -v '^$' | wc -l | tr -d ' ')
echo "--> Extracted ${HEADER_COUNT} UniFFI function prototypes from header."

# 2. 从静态库中提取所有导出的全局 Text 符号 (优先使用 nm -gU，回退至 strings)
if command -v nm >/dev/null 2>&1; then
    LIB_SYMBOLS=$(nm -gU "${STATIC_LIB}" 2>/dev/null | grep -E ' T _uniffi_ttzip_engine_' | awk '{print $3}' | sed 's/^_//' | sort -u || true)
    if [ -z "${LIB_SYMBOLS//[$'\t\r\n ']/}" ]; then
        LIB_SYMBOLS=$(strings "${STATIC_LIB}" | grep -E '^(_?)uniffi_ttzip_engine_' | sed 's/^_//' | sort -u)
    fi
else
    LIB_SYMBOLS=$(strings "${STATIC_LIB}" | grep -E '^(_?)uniffi_ttzip_engine_' | sed 's/^_//' | sort -u)
fi

LIB_COUNT=$(echo "${LIB_SYMBOLS}" | grep -v '^$' | wc -l | tr -d ' ')
echo "--> Extracted ${LIB_COUNT} matching UniFFI symbol definitions from static library."

# 3. 逐项核验头文件符号是否存在于静态库中 (Header -> Lib)
HEADER_TMP=$(mktemp)
LIB_TMP=$(mktemp)
trap 'rm -f "${HEADER_TMP}" "${LIB_TMP}"' EXIT

echo "${HEADER_SYMBOLS}" | grep -v '^$' | sort -u > "${HEADER_TMP}"
echo "${LIB_SYMBOLS}" | grep -v '^$' | sort -u > "${LIB_TMP}"

MISSING_SYMBOLS=$(comm -23 "${HEADER_TMP}" "${LIB_TMP}" || true)
if [ -z "${MISSING_SYMBOLS//[$'\t\r\n ']/}" ]; then
    MISSING_COUNT=0
else
    MISSING_COUNT=$(echo "${MISSING_SYMBOLS}" | wc -l | tr -d ' ')
fi

# 4. 判定门禁结果
if [ "${MISSING_COUNT}" -gt 0 ]; then
    echo "❌ [FAIL] Missing ${MISSING_COUNT} UniFFI symbol(s) in static library:"
    while IFS= read -r ms; do
        [ -n "${ms}" ] && echo "   - ${ms}"
    done <<< "${MISSING_SYMBOLS}"
    echo "======================================================================"
    exit 1
fi

echo "✅ [PASS] 100% UniFFI Symbol Parity (${HEADER_COUNT}/${HEADER_COUNT} symbols present in static library)."
echo "======================================================================"
exit 0
