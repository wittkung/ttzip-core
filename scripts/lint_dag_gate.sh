#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# ==============================================================================
# scripts/lint_dag_gate.sh
# Static Include / Module Dependency DAG Architecture Gate
# Enforces strict unidirectional architecture invariants across TTZip microkernel:
#   1. Pure Rust microkernel (rust/ttzip-engine) has ZERO dependencies on Apple UI frameworks.
#   2. Codec compression modules and decompression modules are strictly orthogonal.
#   3. Microkernel does not import or couple to high-level Swift application layers.
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${WORKSPACE_ROOT}"

# ANSI Colors
C_RESET="\033[0m"
C_BOLD="\033[1m"
C_RED="\033[1;31m"
C_GREEN="\033[1;32m"
C_YELLOW="\033[1;33m"
C_CYAN="\033[1;36m"

VIOLATIONS=0

echo -e "${C_CYAN}${C_BOLD}======================================================================${C_RESET}"
echo -e "${C_CYAN}${C_BOLD}     TTZip Architecture & Module Dependency DAG Linter Gate           ${C_RESET}"
echo -e "${C_CYAN}${C_BOLD}======================================================================${C_RESET}"

report_violation() {
    local rule_id="$1"
    local file_path="$2"
    local line_num="$3"
    local message="$4"
    local snippet="$5"
    echo -e "  ${C_RED}❌ [${rule_id}]${C_RESET} ${file_path}:${line_num}"
    echo -e "     ${C_YELLOW}Violation: ${message}${C_RESET}"
    if [ -n "${snippet}" ]; then
        echo -e "     ${C_CYAN}Code: ${snippet}${C_RESET}"
    fi
    VIOLATIONS=$((VIOLATIONS + 1))
}

ENGINE_SRC_DIR="${WORKSPACE_ROOT}/rust/ttzip-engine/src"

# ------------------------------------------------------------------------------
# Gate 1: Assert rust/ttzip-engine has ZERO dependencies on Apple UI Frameworks
# ------------------------------------------------------------------------------
echo "--> [1/3] Verifying pure Rust microkernel zero-dependency on Apple UI frameworks..."

FORBIDDEN_UI_PATTERNS=(
    "SwiftUI"
    "AppKit"
    "UIKit"
    "Cocoa"
    "NSApplication"
    "NSViewController"
    "NSView"
    "NSWindow"
    "TTZipApp"
    "TTZipFinderSync"
    "TTZipQuickLook"
)

if [ -d "${ENGINE_SRC_DIR}" ]; then
    for pattern in "${FORBIDDEN_UI_PATTERNS[@]}"; do
        while IFS=: read -r file line match; do
            if [ -n "${file}" ]; then
                trimmed=$(echo "${match}" | sed 's/^[ \t]*//')
                if [[ "${trimmed}" =~ ^// ]] || [[ "${trimmed}" =~ ^\* ]]; then
                    continue
                fi
                report_violation "NO_APPLE_UI_IN_MICROKERNEL" "${file}" "${line}" \
                    "Rust microkernel must not reference Apple UI framework symbol '${pattern}'" \
                    "${trimmed}"
            fi
        done < <(grep -rnE "\b${pattern}\b" "${ENGINE_SRC_DIR}" 2>/dev/null || true)
    done
fi

# ------------------------------------------------------------------------------
# Gate 2: Assert Codec Compression & Decompression modules are strictly orthogonal
# ------------------------------------------------------------------------------
echo "--> [2/3] Verifying strict orthogonality between compression and decompression modules..."

CODECS_DIR="${ENGINE_SRC_DIR}/codecs"

if [ -d "${CODECS_DIR}" ]; then
    # 2a. Deflate compressor vs decompressor
    DEFLATE_COMPRESSOR="${CODECS_DIR}/deflate/compressor.rs"
    DEFLATE_DECOMPRESSOR="${CODECS_DIR}/deflate/decompressor.rs"

    if [ -f "${DEFLATE_COMPRESSOR}" ]; then
        while IFS=: read -r line match; do
            [ -n "${line}" ] && report_violation "CODEC_DAG_CYCLIC_DEPENDENCY" "${DEFLATE_COMPRESSOR}" "${line}" \
                "Deflate compressor must not depend on decompressor module" "${match}"
        done < <(grep -nE "(decompressor::|DeflateDecompressor)" "${DEFLATE_COMPRESSOR}" 2>/dev/null | grep -v "^\s*//" || true)
    fi

    if [ -f "${DEFLATE_DECOMPRESSOR}" ]; then
        while IFS=: read -r line match; do
            [ -n "${line}" ] && report_violation "CODEC_DAG_CYCLIC_DEPENDENCY" "${DEFLATE_DECOMPRESSOR}" "${line}" \
                "Deflate decompressor must not depend on compressor module" "${match}"
        done < <(grep -nE "(\bDeflateCompressor\b|compressor::)" "${DEFLATE_DECOMPRESSOR}" 2>/dev/null | grep -v "^\s*//" || true)
    fi

    # 2b. LZMA2 compress vs decompress
    LZMA2_COMPRESS="${CODECS_DIR}/lzma2/compress.rs"
    LZMA2_DECOMPRESS="${CODECS_DIR}/lzma2/decompress.rs"

    if [ -f "${LZMA2_COMPRESS}" ]; then
        while IFS=: read -r line match; do
            [ -n "${line}" ] && report_violation "CODEC_DAG_CYCLIC_DEPENDENCY" "${LZMA2_COMPRESS}" "${line}" \
                "LZMA2 compress module must not depend on decompress module" "${match}"
        done < <(grep -nE "(decompress::|\bFl2DCtx\b|\bFl2DStream\b|\bfl2_decompress\b)" "${LZMA2_COMPRESS}" 2>/dev/null | grep -v "^\s*//" || true)
    fi

    if [ -f "${LZMA2_DECOMPRESS}" ]; then
        while IFS=: read -r line match; do
            [ -n "${line}" ] && report_violation "CODEC_DAG_CYCLIC_DEPENDENCY" "${LZMA2_DECOMPRESS}" "${line}" \
                "LZMA2 decompress module must not depend on compress module" "${match}"
        done < <(grep -nE "(compress::|\bFl2CCtx\b|\bFl2CStream\b|\bfl2_compress\b)" "${LZMA2_DECOMPRESS}" 2>/dev/null | grep -v "^\s*//" || true)
    fi

    # 2c. Zstd cctx vs dctx
    ZSTD_CCTX="${CODECS_DIR}/zstd/cctx.rs"
    ZSTD_DCTX="${CODECS_DIR}/zstd/dctx.rs"

    if [ -f "${ZSTD_CCTX}" ]; then
        while IFS=: read -r line match; do
            [ -n "${line}" ] && report_violation "CODEC_DAG_CYCLIC_DEPENDENCY" "${ZSTD_CCTX}" "${line}" \
                "Zstd cctx (compress) module must not depend on dctx (decompress) module" "${match}"
        done < <(grep -nE "(dctx::|\bZstdDCtx\b|\bZstdDecompressor\b)" "${ZSTD_CCTX}" 2>/dev/null | grep -v "^\s*//" || true)
    fi

    if [ -f "${ZSTD_DCTX}" ]; then
        while IFS=: read -r line match; do
            [ -n "${line}" ] && report_violation "CODEC_DAG_CYCLIC_DEPENDENCY" "${ZSTD_DCTX}" "${line}" \
                "Zstd dctx (decompress) module must not depend on cctx (compress) module" "${match}"
        done < <(grep -nE "(cctx::|\bZstdCCtx\b|\bZstdCompressor\b)" "${ZSTD_DCTX}" 2>/dev/null | grep -v "^\s*//" || true)
    fi
fi

# ------------------------------------------------------------------------------
# Gate 3: Assert Microkernel does not couple to Swift facade layers
# ------------------------------------------------------------------------------
echo "--> [3/3] Verifying clean boundary between Rust microkernel and Swift facade..."

if [ -d "${ENGINE_SRC_DIR}" ]; then
    while IFS=: read -r file line match; do
        if [ -n "${file}" ]; then
            trimmed=$(echo "${match}" | sed 's/^[ \t]*//')
            if [[ "${trimmed}" =~ ^// ]] || [[ "${trimmed}" =~ ^\* ]]; then
                continue
            fi
            report_violation "MICROKERNEL_SWIFT_INVERSION" "${file}" "${line}" \
                "Rust microkernel must not import or depend on Swift bridge Sources" "${trimmed}"
        fi
    done < <(grep -rnE "(import TTZipCore|import CTTZipBridge|Sources/TTZip)" "${ENGINE_SRC_DIR}" 2>/dev/null || true)
fi

echo ""
echo -e "${C_CYAN}======================================================================${C_RESET}"
if [ ${VIOLATIONS} -gt 0 ]; then
    echo -e "${C_RED}${C_BOLD}❌ Architecture DAG Gate Failed: ${VIOLATIONS} violation(s) detected.${C_RESET}"
    echo -e "${C_RED}Please resolve illegal cross-module dependencies or architecture violations.${C_RESET}"
    exit 1
else
    echo -e "${C_GREEN}${C_BOLD}✅ Architecture DAG Gate Passed: 0 violations detected.${C_RESET}"
    exit 0
fi
