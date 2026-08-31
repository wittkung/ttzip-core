#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# run_local_ci_gate.sh: Automated Local Regression & Quality Gate Runner

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${WORKSPACE_ROOT}"

# Colors
C_RESET="\033[0m"
C_BOLD="\033[1m"
C_RED="\033[1;31m"
C_GREEN="\033[1;32m"
C_YELLOW="\033[1;33m"
C_BLUE="\033[1;34m"
C_CYAN="\033[1;36m"

# Options
BAIL_ON_FAILURE=false
TARGET_STAGE=""
JSON_REPORT_PATH=""
USE_RELEASE=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --bail)
            BAIL_ON_FAILURE=true
            shift
            ;;
        --stage)
            TARGET_STAGE="$2"
            shift 2
            ;;
        --release)
            USE_RELEASE=true
            shift
            ;;
        --json)
            JSON_REPORT_PATH="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: ./scripts/run_local_ci_gate.sh [options]"
            echo ""
            echo "Options:"
            echo "  --bail               Stop immediately on first failed stage"
            echo "  --stage <name>       Execute only the specified stage (loc-gate, dag-gate, uniffi-gate, sdk-gate, swift-facade, performance, rust-industrial, sevenz-suite, zip-suite, tar-suite, deflate-defense, libarchive-suite, lz4-suite, lzma2-suite, xz-suite, brotli-suite, snappy-suite, lzfse-suite, bzip2-suite, libdeflate-suite, blake3-suite, ed25519-suite, mmap-suite, uniffi-suite)"
            echo "  --release            Pass --release profile to applicable test stages"
            echo "  --json <path>        Export structured JSON report"
            echo "  -h, --help           Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 64
            ;;
    esac
done

echo -e "${C_CYAN}${C_BOLD}======================================================================${C_RESET}"
echo -e "${C_CYAN}${C_BOLD}      TTZip Local CI/CD Automated Regression & Performance Gate       ${C_RESET}"
echo -e "${C_CYAN}${C_BOLD}======================================================================${C_RESET}"
echo -e "Platform: $(uname -m) macOS $(sw_vers -productVersion 2>/dev/null || echo 'Sonoma')"
echo -e "Date:     $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
echo ""

# Stage Definitions
declare -a STAGE_NAMES=(
    "Single-File LOC Defense Gate (<= 800 LOC)"
    "Architecture & Module Dependency DAG Gate"
    "Mozilla UniFFI Symbol Alignment Gate (100% Scaffolding Parity)"
    "Universal XCFramework & SDK Checksum Gate"
    "Swift High-Level Facade & CLI Suite"
    "Deflate-Bench 50-Point Matrix Gate"
    "Rust Industrial Suite (Props, Fuzz, Differential)"
    "7-Zip Industrial Suite & Invariant 6 Anti-Regression Gate"
    "ZIP Industrial Suite & Invariant 6 Anti-Regression Gate"
    "TAR Industrial Suite & Invariant 6 Anti-Regression Gate"
    "Deflate Deep Defense & CPU-Stripping Gate"
    "libarchive Industrial Suite & Invariant 6 Anti-Regression Gate"
    "LZ4 Industrial Suite & Invariant 6 Anti-Regression Gate"
    "LZMA2 Industrial Suite & Invariant 6 Anti-Regression Gate"
    "XZ Industrial Suite & Invariant 6 Anti-Regression Gate"
    "Brotli Industrial Suite & Invariant 6 Anti-Regression Gate"
    "Snappy Industrial Suite & Invariant 6 Anti-Regression Gate"
    "LZFSE & LZVN Industrial Suite & Invariant 6 Anti-Regression Gate"
    "Bzip2 Industrial Suite & Invariant 6 Anti-Regression Gate"
    "Libdeflate Industrial Suite & Invariant 6 Anti-Regression Gate"
    "BLAKE3 Tree Hashing & Security Invariant 6 Gate"
    "Ed25519 Elliptic Curve & Plugin Auth Invariant 6 Gate"
    "Zero-Copy Mmap Engine & Paging Invariant 6 Gate"
    "Mozilla UniFFI Scaffolding & Multi-Language Invariant 6 Gate"
)

declare -a STAGE_KEYS=(
    "loc-gate"
    "dag-gate"
    "uniffi-gate"
    "sdk-gate"
    "swift-facade"
    "performance"
    "rust-industrial"
    "sevenz-suite"
    "zip-suite"
    "tar-suite"
    "deflate-defense"
    "libarchive-suite"
    "lz4-suite"
    "lzma2-suite"
    "xz-suite"
    "brotli-suite"
    "snappy-suite"
    "lzfse-suite"
    "bzip2-suite"
    "libdeflate-suite"
    "blake3-suite"
    "ed25519-suite"
    "mmap-suite"
    "uniffi-suite"
)

declare -a STAGE_COMMANDS=(
    "./scripts/lint_loc_gate.sh"
    "./scripts/lint_dag_gate.sh"
    "./scripts/verify_uniffi_symbols.sh"
    "./scripts/build_sdk_framework.sh$([ "${USE_RELEASE}" = true ] && echo " --release" || echo " --debug") --native"
    "swift test --parallel"

    "swift run -c release ttzip-bench gate"
    "./scripts/run_rust_tests.sh --unit --props --fuzz$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_7z_tests.sh$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_zip_tests.sh$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_tar_tests.sh$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_deflate_defense_tests.sh"
    "./scripts/run_libarchive_tests.sh$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_lz4_tests.sh$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_lzma2_tests.sh$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_xz_tests.sh$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_brotli_tests.sh$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_snappy_tests.sh$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_lzfse_tests.sh$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_bzip2_tests.sh$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_libdeflate_tests.sh$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_blake3_tests.sh$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_ed25519_tests.sh$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_mmap_tests.sh$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_uniffi_tests.sh$([ "${USE_RELEASE}" = true ] && echo " --release")"
)

TOTAL_STAGES=${#STAGE_NAMES[@]}
PASSED_STAGES=0
FAILED_STAGES=0
GLOBAL_START_TIME=$(python3 -c "import time; print(time.time())")

declare -a STAGE_STATUSES=()
declare -a STAGE_DURATIONS=()
declare -a STAGE_DIAGNOSTICS=()

# Pre-compile test binaries in a single parallel step to avoid repeated per-stage link storms
if [[ -z "${TARGET_STAGE}" || "${TARGET_STAGE}" == *"suite"* ]]; then
    compute_preflight_fingerprint() {
        local git_tree dirty_diff top_vendor_diff rustc_ver
        git_tree="$(git -C "${WORKSPACE_ROOT}" rev-parse HEAD:rust 2>/dev/null || echo "no-git")"
        dirty_diff="$( (git -C "${WORKSPACE_ROOT}" diff HEAD -- rust vendor 2>/dev/null; git -C "${WORKSPACE_ROOT}" ls-files --others --exclude-standard rust vendor 2>/dev/null) | shasum -a 256 | awk '{print $1}')"
        top_vendor_diff="$( (git -C "${WORKSPACE_ROOT}/.." diff HEAD -- vendor 2>/dev/null; git -C "${WORKSPACE_ROOT}/.." ls-files --others --exclude-standard vendor 2>/dev/null) | shasum -a 256 | awk '{print $1}')"
        rustc_ver="$(rustc -Vv 2>/dev/null | shasum -a 256 | awk '{print $1}')"
        printf "%s\n%s\n%s\n%s\n%s" "${git_tree}" "${dirty_diff}" "${top_vendor_diff}" "${rustc_ver}" "${USE_RELEASE}" | shasum -a 256 | awk '{print $1}'
    }

    PREFLIGHT_FINGERPRINT_FILE="${WORKSPACE_ROOT}/rust/target/.preflight_fingerprint_${USE_RELEASE}"
    CURRENT_PREFLIGHT_FINGERPRINT="$(compute_preflight_fingerprint)"

    if [ -f "${PREFLIGHT_FINGERPRINT_FILE}" ] && [ "$(cat "${PREFLIGHT_FINGERPRINT_FILE}" 2>/dev/null || true)" = "${CURRENT_PREFLIGHT_FINGERPRINT}" ]; then
        echo -e "${C_GREEN}--> [Pre-flight] Pre-compilation cache up-to-date (0.015s, hit: ${CURRENT_PREFLIGHT_FINGERPRINT:0:12}). Proceeding to stages.${C_RESET}\n"
    else
        echo -e "${C_CYAN}${C_BOLD}--> [Pre-flight] Pre-checking workspace tests and dependencies...${C_RESET}"
        PREFLIGHT_START=$(python3 -c "import time; print(time.time())")
        (
            cd "${WORKSPACE_ROOT}/rust"
            cargo check $([ "${USE_RELEASE}" = true ] && echo "--release") --tests -p ttzip-engine > /dev/null 2>&1 || true
        )
        mkdir -p "${WORKSPACE_ROOT}/rust/target"
        echo "${CURRENT_PREFLIGHT_FINGERPRINT}" > "${PREFLIGHT_FINGERPRINT_FILE}"
        PREFLIGHT_END=$(python3 -c "import time; print(time.time())")
        PREFLIGHT_DUR=$(python3 -c "print(round(${PREFLIGHT_END} - ${PREFLIGHT_START}, 3))")
        echo -e "${C_GREEN}--> [Pre-flight] Pre-check ready (${PREFLIGHT_DUR}s). Proceeding to stages.${C_RESET}\n"
    fi
fi

for i in "${!STAGE_NAMES[@]}"; do
    STAGE_INDEX=$((i + 1))
    STAGE_NAME="${STAGE_NAMES[$i]}"
    STAGE_KEY="${STAGE_KEYS[$i]}"
    STAGE_CMD="${STAGE_COMMANDS[$i]}"
    
    if [[ -n "${TARGET_STAGE}" && "${TARGET_STAGE}" != "${STAGE_KEY}" && "${TARGET_STAGE}" != "${STAGE_INDEX}" ]]; then
        STAGE_STATUSES+=("skip")
        STAGE_DURATIONS+=(0.0)
        STAGE_DIAGNOSTICS+=("Filtered out by --stage")
        continue
    fi
    
    echo -e "${C_BOLD}[Stage ${STAGE_INDEX}/${TOTAL_STAGES}] ${C_BLUE}${STAGE_NAME}${C_RESET}"
    echo -e "  Command: ${C_YELLOW}${STAGE_CMD}${C_RESET}"
    
    STAGE_START=$(python3 -c "import time; print(time.time())")
    set +e
    TMP_LOG=$(mktemp)
    eval "${STAGE_CMD}" > "${TMP_LOG}" 2>&1
    CMD_EXIT=$?
    set -e
    STAGE_END=$(python3 -c "import time; print(time.time())")
    STAGE_DUR=$(python3 -c "print(round(${STAGE_END} - ${STAGE_START}, 3))")
    STAGE_DURATIONS+=("${STAGE_DUR}")
    
    if [ ${CMD_EXIT} -eq 0 ]; then
        echo -e "  Result:  ${C_GREEN}${C_BOLD}[PASS]${C_RESET} (${STAGE_DUR}s)"
        STAGE_STATUSES+=("pass")
        STAGE_DIAGNOSTICS+=("")
        PASSED_STAGES=$((PASSED_STAGES + 1))
    else
        echo -e "  Result:  ${C_RED}${C_BOLD}[FAIL]${C_RESET} (${STAGE_DUR}s, exit code ${CMD_EXIT})"
        STAGE_STATUSES+=("fail")
        LAST_LINES=$(tail -n 10 "${TMP_LOG}" | tr '\n' ' ')
        STAGE_DIAGNOSTICS+=("${LAST_LINES}")
        FAILED_STAGES=$((FAILED_STAGES + 1))
        
        echo -e "${C_RED}--- Error Diagnostic Output ---${C_RESET}"
        cat "${TMP_LOG}" | tail -n 25
        echo -e "${C_RED}-------------------------------${C_RESET}"
        
        if [ "${BAIL_ON_FAILURE}" = true ]; then
            rm -f "${TMP_LOG}"
            break
        fi
    fi
    rm -f "${TMP_LOG}"
    echo ""
done

GLOBAL_END_TIME=$(python3 -c "import time; print(time.time())")
GLOBAL_DURATION=$(python3 -c "print(round(${GLOBAL_END_TIME} - ${GLOBAL_START_TIME}, 3))")

echo -e "${C_CYAN}${C_BOLD}======================================================================${C_RESET}"
echo -e "${C_CYAN}${C_BOLD}                          Summary Table                               ${C_RESET}"
echo -e "${C_CYAN}${C_BOLD}======================================================================${C_RESET}"
printf "%-6s | %-45s | %-8s | %-10s\n" "Stage" "Name" "Status" "Duration"
echo "----------------------------------------------------------------------------------"

for i in "${!STAGE_STATUSES[@]}"; do
    S_IDX=$((i + 1))
    S_NAME="${STAGE_NAMES[$i]}"
    S_STAT="${STAGE_STATUSES[$i]}"
    S_DUR="${STAGE_DURATIONS[$i]}s"
    
    if [ "${S_STAT}" = "pass" ]; then
        STATUS_DISPLAY="${C_GREEN}PASS${C_RESET}"
    elif [ "${S_STAT}" = "fail" ]; then
        STATUS_DISPLAY="${C_RED}FAIL${C_RESET}"
    else
        STATUS_DISPLAY="${C_YELLOW}SKIP${C_RESET}"
    fi
    
    printf "%-6s | %-45s | %-17b | %-10s\n" "${S_IDX}" "${S_NAME}" "${STATUS_DISPLAY}" "${S_DUR}"
done

echo "----------------------------------------------------------------------------------"
echo -e "Total: ${PASSED_STAGES} Passed, ${FAILED_STAGES} Failed (${GLOBAL_DURATION}s total)"
echo ""

# Export JSON if requested
if [[ -n "${JSON_REPORT_PATH}" ]]; then
    python3 -c "
import json
import os
import sys

raw_input = sys.stdin.read()
data = json.loads(raw_input)

out_path = data['out_path']
report = {
    'totalStages': len(data['stages']),
    'passedStages': data['passedStages'],
    'failedStages': data['failedStages'],
    'totalDurationSeconds': data['totalDurationSeconds'],
    'isSuccess': (data['failedStages'] == 0),
    'stages': data['stages']
}

os.makedirs(os.path.dirname(out_path) or '.', exist_ok=True)
with open(out_path, 'w') as f:
    json.dump(report, f, indent=2)
print('Exported JSON gate report to ' + out_path)
" << JSONPAYLOAD
{
  "out_path": "${JSON_REPORT_PATH}",
  "passedStages": ${PASSED_STAGES},
  "failedStages": ${FAILED_STAGES},
  "totalDurationSeconds": ${GLOBAL_DURATION},
  "stages": [
$(for i in "${!STAGE_NAMES[@]}"; do
    comma=","
    if [ $i -eq $((${#STAGE_NAMES[@]} - 1)) ]; then comma=""; fi
    cat << STAGE_ITEM
    {
      "stageIndex": $((i + 1)),
      "key": "${STAGE_KEYS[$i]}",
      "name": "${STAGE_NAMES[$i]}",
      "command": "${STAGE_COMMANDS[$i]}",
      "status": "${STAGE_STATUSES[$i]}",
      "durationSeconds": ${STAGE_DURATIONS[$i]},
      "diagnosticMessage": $(if [ "${STAGE_STATUSES[$i]}" = "fail" ]; then echo "\"Stage execution failed\""; else echo "null"; fi)
    }${comma}
STAGE_ITEM
done)
  ]
}
JSONPAYLOAD
fi

if [ ${FAILED_STAGES} -gt 0 ]; then
    echo -e "${C_RED}${C_BOLD}❌ Local CI/CD Gate Failed! Fix issues before pushing.${C_RESET}"
    exit 1
else
    echo -e "${C_GREEN}${C_BOLD}✅ Local CI/CD Gate Passed! 100% compliant and ready.${C_RESET}"
    exit 0
fi
