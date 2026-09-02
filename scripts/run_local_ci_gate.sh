#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# run_local_ci_gate.sh: High-Performance DAG-Based Multi-Worker Quality Gate Runner

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${WORKSPACE_ROOT}"

export SDKROOT="${SDKROOT:-$(xcrun --show-sdk-path 2>/dev/null || true)}"
export DEVELOPER_DIR="${DEVELOPER_DIR:-$(xcode-select -p 2>/dev/null || true)}"
export GATE_MODE=1
export CLANG_MODULE_CACHE_PATH="${WORKSPACE_ROOT}/.build/clang-module-cache"
export SWIFT_MODULE_CACHE_PATH="${WORKSPACE_ROOT}/.build/swift-module-cache"
mkdir -p "${CLANG_MODULE_CACHE_PATH}" "${SWIFT_MODULE_CACHE_PATH}" 2>/dev/null || true

# Colors
C_RESET="\033[0m"
C_BOLD="\033[1m"
C_RED="\033[1;31m"
C_GREEN="\033[1;32m"
C_YELLOW="\033[1;33m"
C_BLUE="\033[1;34m"
C_CYAN="\033[1;36m"

# Options & Defaults
DEFAULT_JOBS=6
MAX_JOBS="${DEFAULT_JOBS}"
BAIL_ON_FAILURE=false
TARGET_STAGE=""
JSON_REPORT_PATH=""
USE_RELEASE=false

show_help() {
    echo "Usage: ./scripts/run_local_ci_gate.sh [options]"
    echo ""
    echo "Options:"
    echo "  -j, --jobs <N>       Maximum number of concurrent worker processes (default: ${DEFAULT_JOBS})"
    echo "  --bail               Stop immediately on first failed stage"
    echo "  -s, --stage <name|idx> Execute only the specified stage (loc-gate, dag-gate, uniffi-gate, sdk-gate, swift-facade, performance, rust-industrial, sevenz-suite, zip-suite, tar-suite, deflate-defense, libarchive-suite, lz4-suite, lzma2-suite, xz-suite, brotli-suite, snappy-suite, lzfse-suite, bzip2-suite, libdeflate-suite, blake3-suite, ed25519-suite, mmap-suite, uniffi-suite, zlib-ng-suite, zopfli-suite, text-encoding-suite, xml-suite, syntax-suite, image-suite, pdf-suite, audio-suite, ebook-suite, office-suite, html-suite, video-suite, system-suite)"
    echo "  --release            Pass --release profile to applicable test stages"
    echo "  --json <path>        Export structured JSON report"
    echo "  -h, --help           Show this help message"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        -j|--jobs)
            MAX_JOBS="$2"
            shift 2
            ;;
        -j*)
            MAX_JOBS="${1#-j}"
            shift
            ;;
        --jobs=*)
            MAX_JOBS="${1#--jobs=}"
            shift
            ;;
        --bail)
            BAIL_ON_FAILURE=true
            shift
            ;;
        -s|--stage)
            TARGET_STAGE="$2"
            shift 2
            ;;
        --stage=*)
            TARGET_STAGE="${1#--stage=}"
            shift
            ;;
        --release)
            USE_RELEASE=true
            shift
            ;;
        --json)
            JSON_REPORT_PATH="$2"
            shift 2
            ;;
        --json=*)
            JSON_REPORT_PATH="${1#--json=}"
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            show_help
            exit 64
            ;;
    esac
done

if ! [[ "${MAX_JOBS}" =~ ^[1-9][0-9]*$ ]]; then
    MAX_JOBS="${DEFAULT_JOBS}"
fi

# Stage Definitions (37 Total)
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
    "zlib-ng Modern Deflate & 8-Corpus Invariant 6 Gate"
    "Zopfli Optimal Deflate & Ground Truth Invariant 6 Gate"
    "Text Encoding Detection & Transcoding Invariant 6 Gate"
    "Streaming XML Parser & Document Metadata Invariant 6 Gate"
    "Tree-sitter Incremental Syntax & AST Highlight Invariant 6 Gate"
    "Pure-Rust Image Decoder & Viewport Rendering Invariant 6 Gate"
    "Pure-Rust PDF Parser & Streaming Text Invariant 6 Gate"
    "Pure-Rust Audio Decoder & Waveform Invariant 6 Gate"
    "Pure-Rust E-Book Parser & Spine Navigation Invariant 6 Gate"
    "Pure-Rust Office Suite Parser & Formula Engine Invariant 6 Gate"
    "Pure-Rust HTML Streaming Rewriter & VFS Router Invariant 6 Gate"
    "Pure-Rust Video Demuxer & Metadata Extraction Invariant 6 Gate"
    "Pure-Rust BinaryDelta Engine & System Security Invariant 6 Gate"
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
    "zlib-ng-suite"
    "zopfli-suite"
    "text-encoding-suite"
    "xml-suite"
    "syntax-suite"
    "image-suite"
    "pdf-suite"
    "audio-suite"
    "ebook-suite"
    "office-suite"
    "html-suite"
    "video-suite"
    "system-suite"
)

declare -a STAGE_COMMANDS=(
    "./scripts/lint_loc_gate.sh"
    "./scripts/lint_dag_gate.sh"
    "./scripts/verify_uniffi_symbols.sh"
    "./scripts/build_sdk_framework.sh --release --native --no-zip"
    "swift test --disable-sandbox --parallel"
    "swift run --disable-sandbox -c release ttzip-bench gate"
    "./scripts/run_rust_tests.sh --unit --props --fuzz$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_7z_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_zip_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_tar_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_deflate_defense_tests.sh"
    "./scripts/run_libarchive_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_lz4_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_lzma2_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_xz_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_brotli_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_snappy_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_lzfse_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_bzip2_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_libdeflate_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_blake3_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_ed25519_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_mmap_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_uniffi_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_zlib_ng_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_zopfli_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_text_encoding_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_xml_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_syntax_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_image_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_pdf_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_audio_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_ebook_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_office_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_html_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_video_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
    "./scripts/run_system_tests.sh --gate$([ "${USE_RELEASE}" = true ] && echo " --release")"
)

TOTAL_STAGES=${#STAGE_NAMES[@]}
GLOBAL_START_TIME=$(python3 -c "import time; print(time.time())")

# Temporary workspace for isolated stage logs and atomic metrics
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/ttzip_ci_gate_XXXXXX")"
declare -A ACTIVE_PIDS=()

cleanup() {
    for pid in "${!ACTIVE_PIDS[@]}"; do
        kill -TERM "${pid}" 2>/dev/null || true
    done
    rm -rf "${TMP_DIR}" 2>/dev/null || true
}
trap cleanup EXIT INT TERM



run_stage_bg() {
    local idx="$1"
    local cmd="${STAGE_COMMANDS[$((idx - 1))]}"
    local log_file="${TMP_DIR}/stage_${idx}.log"
    
    (
        local start_ts
        start_ts=$(python3 -c "import time; print(time.time())")
        set +e
        export GATE_MODE=1
        export RUST_TEST_THREADS=2
        export RAYON_NUM_THREADS=1
        export CLANG_MODULE_CACHE_PATH="${WORKSPACE_ROOT}/.build/module-cache"
        export SWIFTPM_MODULECACHE_OVERRIDE="${WORKSPACE_ROOT}/.build/module-cache"
        mkdir -p "${WORKSPACE_ROOT}/.build/module-cache"
        (
            cd "${WORKSPACE_ROOT}"
            eval "${cmd}"
        ) > "${log_file}" 2>&1
        local cmd_exit=$?
        local end_ts
        end_ts=$(python3 -c "import time; print(time.time())")
        local dur
        dur=$(python3 -c "print(round(${end_ts} - ${start_ts}, 3))")
        
        echo "${dur}" > "${TMP_DIR}/dur_${idx}.txt"
        echo "${cmd_exit}" > "${TMP_DIR}/exit_${idx}.txt"
        if [ ${cmd_exit} -eq 0 ]; then
            echo "pass" > "${TMP_DIR}/status_${idx}.txt"
            echo "" > "${TMP_DIR}/diag_${idx}.txt"
        else
            echo "fail" > "${TMP_DIR}/status_${idx}.txt"
            tail -n 10 "${log_file}" 2>/dev/null | tr '\n' ' ' > "${TMP_DIR}/diag_${idx}.txt" || true
        fi
        exit ${cmd_exit}
    ) &
    local pid=$!
    ACTIVE_PIDS["${pid}"]="${idx}"
}

run_stage_sync() {
    local idx="$1"
    run_stage_bg "${idx}"
    local pid
    for p in "${!ACTIVE_PIDS[@]}"; do
        if [ "${ACTIVE_PIDS[${p}]}" -eq "${idx}" ]; then
            pid="${p}"
            break
        fi
    done
    
    local exit_code=0
    if [ -n "${pid}" ]; then
        wait "${pid}" || exit_code=$?
        unset "ACTIVE_PIDS[${pid}]"
    fi
    return ${exit_code}
}

report_stage_completion() {
    local idx="$1"
    local name="${STAGE_NAMES[$((idx - 1))]}"
    local cmd="${STAGE_COMMANDS[$((idx - 1))]}"
    local log_file="${TMP_DIR}/stage_${idx}.log"
    local status="$(cat "${TMP_DIR}/status_${idx}.txt" 2>/dev/null || echo "fail")"
    local dur="$(cat "${TMP_DIR}/dur_${idx}.txt" 2>/dev/null || echo "0.0")"
    local exit_code="$(cat "${TMP_DIR}/exit_${idx}.txt" 2>/dev/null || echo "1")"
    
    echo -e "${C_BOLD}[Stage ${idx}/${TOTAL_STAGES}] ${C_BLUE}${name}${C_RESET}"
    echo -e "  Command: ${C_YELLOW}${cmd}${C_RESET}"
    
    if [ "${status}" = "pass" ]; then
        echo -e "  Result:  ${C_GREEN}${C_BOLD}[PASS]${C_RESET} (${dur}s)"
    else
        echo -e "  Result:  ${C_RED}${C_BOLD}[FAIL]${C_RESET} (${dur}s, exit code ${exit_code})"
        echo -e "${C_RED}--- Error Diagnostic Output ---${C_RESET}"
        if [ -f "${log_file}" ]; then
            tail -n 25 "${log_file}"
        fi
        echo -e "${C_RED}-------------------------------${C_RESET}"
    fi
    echo ""
}

# ------------------------------------------------------------------------------
# Banner
# ------------------------------------------------------------------------------
echo -e "${C_CYAN}${C_BOLD}======================================================================${C_RESET}"
echo -e "${C_CYAN}${C_BOLD}      TTZip Local CI/CD Automated Regression & Performance Gate       ${C_RESET}"
echo -e "${C_CYAN}${C_BOLD}======================================================================${C_RESET}"
echo -e "Platform: $(uname -m) macOS $(sw_vers -productVersion 2>/dev/null || echo 'Sonoma')"
echo -e "Workers:  ${MAX_JOBS} parallel threads (DAG topology scheduler)"
echo -e "Date:     $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
echo ""

# ------------------------------------------------------------------------------
# Targeted Single Stage Execution
# ------------------------------------------------------------------------------
if [[ -n "${TARGET_STAGE}" ]]; then
    TARGET_INDEX=""
    for ((i=0; i<TOTAL_STAGES; i++)); do
        idx=$((i + 1))
        if [[ "${TARGET_STAGE}" == "${STAGE_KEYS[$i]}" || "${TARGET_STAGE}" == "${idx}" ]]; then
            TARGET_INDEX="${idx}"
            break
        fi
    done
    
    if [[ -z "${TARGET_INDEX}" ]]; then
        echo -e "${C_RED}Unknown stage: ${TARGET_STAGE}${C_RESET}"
        show_help
        exit 64
    fi
    
    run_stage_sync "${TARGET_INDEX}" || true
    report_stage_completion "${TARGET_INDEX}"
    
    for ((i=0; i<TOTAL_STAGES; i++)); do
        idx=$((i + 1))
        if [ "${idx}" -ne "${TARGET_INDEX}" ]; then
            echo "skip" > "${TMP_DIR}/status_${idx}.txt"
            echo "0.0" > "${TMP_DIR}/dur_${idx}.txt"
            echo "Filtered out by --stage" > "${TMP_DIR}/diag_${idx}.txt"
        fi
    done
else
    # ------------------------------------------------------------------------------
    # Full DAG Topology Multi-Worker Pipeline Execution
    # ------------------------------------------------------------------------------
    PIPELINE_BAIL=false

    # --- Phase 0 (Concurrency): Static Analysis & LOC/DAG Gates ---
    run_stage_bg 1
    run_stage_bg 2
    while [[ ${#ACTIVE_PIDS[@]} -gt 0 ]]; do
        wait -n -p FIN_PID || true
        FIN_IDX="${ACTIVE_PIDS[${FIN_PID}]}"
        unset "ACTIVE_PIDS[${FIN_PID}]"
        report_stage_completion "${FIN_IDX}"
        
        FIN_STATUS="$(cat "${TMP_DIR}/status_${FIN_IDX}.txt" 2>/dev/null || echo "fail")"
        if [ "${FIN_STATUS}" != "pass" ] && [ "${BAIL_ON_FAILURE}" = true ]; then
            for p in "${!ACTIVE_PIDS[@]}"; do kill -TERM "${p}" 2>/dev/null || true; done
            ACTIVE_PIDS=()
            PIPELINE_BAIL=true
            break
        fi
    done

    # --- Phase 1: Universal SDK & XCFramework Unified Pre-build Gate ---
    if [ "${PIPELINE_BAIL}" = false ]; then
        run_stage_sync 4 || true
        report_stage_completion 4
        
        STAGE_4_STATUS="$(cat "${TMP_DIR}/status_4.txt" 2>/dev/null || echo "fail")"
        if [ "${STAGE_4_STATUS}" != "pass" ] && [ "${BAIL_ON_FAILURE}" = true ]; then
            PIPELINE_BAIL=true
        fi
    fi

    # --- Phase 2: Mozilla UniFFI Symbol Alignment Gate ---
    if [ "${PIPELINE_BAIL}" = false ]; then
        run_stage_sync 3 || true
        report_stage_completion 3
        
        STAGE_3_STATUS="$(cat "${TMP_DIR}/status_3.txt" 2>/dev/null || echo "fail")"
        if [ "${STAGE_3_STATUS}" != "pass" ] && [ "${BAIL_ON_FAILURE}" = true ]; then
            PIPELINE_BAIL=true
        fi
    fi

    # --- Phase 3: Full Concurrent Worker Pool (Track A, Track B, Track C) ---
    if [ "${PIPELINE_BAIL}" = false ]; then
        (
            cd "${WORKSPACE_ROOT}/rust"
            cargo test --target aarch64-apple-darwin -p ttzip-engine --tests --no-run >/dev/null 2>&1 || true
        )

        declare -a WORK_QUEUE=(
            5 11
            7 8 9 10
            12 13 14 15 16 17 18 19 20
            21 22 23 24
            25 26 27 28 29 30 31 32 33 34 35 36 37
        )

        # Prefill Worker Pool up to MAX_JOBS
        while [ ${#WORK_QUEUE[@]} -gt 0 ] && [ ${#ACTIVE_PIDS[@]} -lt ${MAX_JOBS} ]; do
            NEXT_STAGE="${WORK_QUEUE[0]}"
            WORK_QUEUE=("${WORK_QUEUE[@]:1}")
            run_stage_bg "${NEXT_STAGE}"
        done

        # Reactive Worker Pool Loop
        while [ ${#ACTIVE_PIDS[@]} -gt 0 ]; do
            wait -n -p FIN_PID || true
            FIN_IDX="${ACTIVE_PIDS[${FIN_PID}]}"
            unset "ACTIVE_PIDS[${FIN_PID}]"
            report_stage_completion "${FIN_IDX}"
            
            FIN_STATUS="$(cat "${TMP_DIR}/status_${FIN_IDX}.txt" 2>/dev/null || echo "fail")"
            if [ "${FIN_STATUS}" != "pass" ] && [ "${BAIL_ON_FAILURE}" = true ]; then
                for p in "${!ACTIVE_PIDS[@]}"; do kill -TERM "${p}" 2>/dev/null || true; done
                ACTIVE_PIDS=()
                WORK_QUEUE=()
                PIPELINE_BAIL=true
                break
            fi
            
            # Track A Sequential Dependency: Stage 5 -> Stage 6
            if [ "${FIN_IDX}" -eq 5 ]; then
                if [ "${FIN_STATUS}" = "pass" ]; then
                    WORK_QUEUE+=(6)
                else
                    echo "skip" > "${TMP_DIR}/status_6.txt"
                    echo "0.0" > "${TMP_DIR}/dur_6.txt"
                    echo "1" > "${TMP_DIR}/exit_6.txt"
                    echo "Skipped due to Stage 5 failure" > "${TMP_DIR}/diag_6.txt"
                fi
            fi
            
            # Refill idle workers
            while [ ${#WORK_QUEUE[@]} -gt 0 ] && [ ${#ACTIVE_PIDS[@]} -lt ${MAX_JOBS} ]; do
                NEXT_STAGE="${WORK_QUEUE[0]}"
                WORK_QUEUE=("${WORK_QUEUE[@]:1}")
                run_stage_bg "${NEXT_STAGE}"
            done
        done
    fi
fi

# ------------------------------------------------------------------------------
# Final Summary Table & JSON Export
# ------------------------------------------------------------------------------
GLOBAL_END_TIME=$(python3 -c "import time; print(time.time())")
GLOBAL_DURATION=$(python3 -c "print(round(${GLOBAL_END_TIME} - ${GLOBAL_START_TIME}, 3))")

PASSED_STAGES=0
FAILED_STAGES=0

echo -e "${C_CYAN}${C_BOLD}======================================================================${C_RESET}"
echo -e "${C_CYAN}${C_BOLD}                          Summary Table                               ${C_RESET}"
echo -e "${C_CYAN}${C_BOLD}======================================================================${C_RESET}"
printf "%-6s | %-45s | %-8s | %-10s\n" "Stage" "Name" "Status" "Duration"
echo "----------------------------------------------------------------------------------"

for ((i=0; i<TOTAL_STAGES; i++)); do
    S_IDX=$((i + 1))
    S_NAME="${STAGE_NAMES[$i]}"
    
    if [ -f "${TMP_DIR}/status_${S_IDX}.txt" ]; then
        S_STAT="$(cat "${TMP_DIR}/status_${S_IDX}.txt" 2>/dev/null || echo "skip")"
        S_DUR="$(cat "${TMP_DIR}/dur_${S_IDX}.txt" 2>/dev/null || echo "0.0")s"
    else
        S_STAT="skip"
        S_DUR="0.0s"
    fi
    
    if [ "${S_STAT}" = "pass" ]; then
        STATUS_DISPLAY="${C_GREEN}PASS${C_RESET}"
        PASSED_STAGES=$((PASSED_STAGES + 1))
    elif [ "${S_STAT}" = "fail" ]; then
        STATUS_DISPLAY="${C_RED}FAIL${C_RESET}"
        FAILED_STAGES=$((FAILED_STAGES + 1))
    else
        STATUS_DISPLAY="${C_YELLOW}SKIP${C_RESET}"
    fi
    
    printf "%-6s | %-45s | %-17b | %-10s\n" "${S_IDX}" "${S_NAME}" "${STATUS_DISPLAY}" "${S_DUR}"
done

echo "----------------------------------------------------------------------------------"
echo -e "Total: ${PASSED_STAGES} Passed, ${FAILED_STAGES} Failed (${GLOBAL_DURATION}s total)"
echo ""

if [[ -n "${JSON_REPORT_PATH}" ]]; then
    python3 -c "
import json
import os
import sys

out_path = sys.argv[1]
passed = int(sys.argv[2])
failed = int(sys.argv[3])
total_dur = float(sys.argv[4])

report = {
    'totalStages': passed + failed,
    'passedStages': passed,
    'failedStages': failed,
    'totalDurationSeconds': total_dur,
    'isSuccess': (failed == 0),
}
os.makedirs(os.path.dirname(out_path) or '.', exist_ok=True)
with open(out_path, 'w') as f:
    json.dump(report, f, indent=2)
print('Exported JSON gate report to ' + out_path)
" "${JSON_REPORT_PATH}" "${PASSED_STAGES}" "${FAILED_STAGES}" "${GLOBAL_DURATION}"
fi

# ------------------------------------------------------------------------------
# Post-Pipeline Auto-Eviction & Micro-Surgical GC Hook
# ------------------------------------------------------------------------------
if [ -f "${WORKSPACE_ROOT}/scripts/gc_target.py" ]; then
    python3 "${WORKSPACE_ROOT}/scripts/gc_target.py" --target "${WORKSPACE_ROOT}/rust/target" --keep 1 --quiet >/dev/null 2>&1 || true
fi

if [ ${FAILED_STAGES} -gt 0 ]; then
    echo -e "${C_RED}${C_BOLD}❌ Local CI/CD Gate Failed! Fix issues before pushing.${C_RESET}"
    exit 1
else
    echo -e "${C_GREEN}${C_BOLD}✅ Local CI/CD Gate Passed! 100% compliant and ready.${C_RESET}"
    exit 0
fi
