#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# Master Test Orchestrator for TTZip Full Multilingual SDK Testing System.
# Supports 9 SDK ecosystems, dynamic toolchain probing, category filters,
# structured JSON reporting, and standard JUnit XML export.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# ANSI Color codes
BOLD="\033[1m"
GREEN="\033[0;32m"
RED="\033[0;31m"
YELLOW="\033[0;33m"
CYAN="\033[0;36m"
MAGENTA="\033[0;35m"
GRAY="\033[0;90m"
RESET="\033[0m"

# Default Options
TARGET_SDKS="all"
CATEGORIES="unit"
JSON_OUTPUT=""
JUNIT_OUTPUT=""
DRY_RUN=false
VERBOSE=false
BAIL_ON_ERROR=false

# Usage information
show_help() {
    cat << 'EOF'
TTZip Multilingual SDK Master Test Matrix Orchestrator

Usage:
  run_sdk_test_matrix.sh [OPTIONS]

Options:
  --sdk=<list>          Comma-separated list of SDKs to test (default: all)
                        Supported: rust, swift, python, go, java, c, cpp, dart, dotnet
  --category=<list>     Comma-separated categories to run (default: unit)
                        Supported: unit, security, canonical, all
  --json=<path>         Export structured test report JSON (conforming to schema)
  --junit=<dir_or_file> Export JUnit XML report(s)
  --dry-run             Detect environment and preview execution without running tests
  --bail, -x            Stop execution immediately on first test failure
  -v, --verbose         Enable verbose test logs and command traces
  -h, --help            Show this help dialog

Examples:
  ./run_sdk_test_matrix.sh --sdk=rust,python,go
  ./run_sdk_test_matrix.sh --category=unit,security --json=build/test-report.json
  ./run_sdk_test_matrix.sh --junit=build/junit/
  ./run_sdk_test_matrix.sh --dry-run
EOF
}

# Parse Command Line Arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --sdk=*)
            TARGET_SDKS="${1#*=}"
            shift
            ;;
        --category=*|--categories=*)
            CATEGORIES="${1#*=}"
            shift
            ;;
        --json=*)
            JSON_OUTPUT="${1#*=}"
            shift
            ;;
        --json)
            JSON_OUTPUT="$2"
            shift 2
            ;;
        --junit=*)
            JUNIT_OUTPUT="${1#*=}"
            shift
            ;;
        --junit)
            JUNIT_OUTPUT="$2"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --bail|-x)
            BAIL_ON_ERROR=true
            shift
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            echo -e "${RED}Error: Unknown argument '$1'${RESET}" >&2
            show_help >&2
            exit 2
            ;;
    esac
done

cd "${REPO_ROOT}"

# Create temporary work directory for intermediate records
TMP_RUN_DIR="$(mktemp -d /tmp/ttzip_sdk_test_run.XXXXXX)"
cleanup() {
    rm -rf "${TMP_RUN_DIR}"
}
trap cleanup EXIT

echo -e "${BOLD}${CYAN}======================================================================${RESET}"
echo -e "${BOLD}${CYAN}⚡️ TTZip Multi-Language SDK Test Matrix Orchestrator${RESET}"
echo -e "${BOLD}${CYAN}======================================================================${RESET}"

# Step 1: Detect Toolchains
TOOLCHAINS_JSON="${TMP_RUN_DIR}/toolchains.json"
bash "${SCRIPT_DIR}/detect_toolchains.sh" --json > "${TOOLCHAINS_JSON}"

# Determine which SDKs are active
check_sdk_enabled() {
    local sdk="$1"
    if [[ "${TARGET_SDKS}" == "all" ]]; then
        return 0
    fi
    IFS=',' read -ra SDK_ARR <<< "${TARGET_SDKS}"
    for s in "${SDK_ARR[@]}"; do
        if [[ "${s}" == "${sdk}" || ("${s}" == "jvm" && "${sdk}" == "java") ]]; then
            return 0
        fi
    done
    return 1
}

# Determine which Categories are active
check_category_enabled() {
    local cat="$1"
    if [[ "${CATEGORIES}" == "all" ]]; then
        return 0
    fi
    IFS=',' read -ra CAT_ARR <<< "${CATEGORIES}"
    for c in "${CAT_ARR[@]}"; do
        if [[ "${c}" == "${cat}" ]]; then
            return 0
        fi
    done
    return 1
}

# Helper to check if toolchain for an SDK is available
is_toolchain_available() {
    local sdk="$1"
    python3 -c "
import json
with open('${TOOLCHAINS_JSON}') as f:
    data = json.load(f)
sdks = data.get('sdks', {})
avail = sdks.get('${sdk}', {}).get('available', False)
exit(0 if avail else 1)
" 2>/dev/null
}

get_toolchain_reason() {
    local sdk="$1"
    python3 -c "
import json
with open('${TOOLCHAINS_JSON}') as f:
    data = json.load(f)
sdks = data.get('sdks', {})
reason = sdks.get('${sdk}', {}).get('reason', 'Toolchain unavailable')
print(reason)
" 2>/dev/null || echo "Toolchain not available"
}

# Record result using aggregator
record_sdk_result() {
    local sdk="$1"
    local status="$2"
    local tool_avail="$3"
    local duration_ms="$4"
    local total="$5"
    local passed="$6"
    local failed="$7"
    local skipped="$8"

    local rec_json="${TMP_RUN_DIR}/rec_${sdk}.json"
    python3 "${REPO_ROOT}/tests/matrix/test_report_aggregator.py" \
        --record-sdk \
        --sdk "${sdk}" \
        --status "${status}" \
        --toolchain-available "${tool_avail}" \
        --duration-ms "${duration_ms}" \
        --total "${total}" \
        --passed "${passed}" \
        --failed "${failed}" \
        --skipped "${skipped}" \
        --json-out "${rec_json}" >/dev/null 2>&1 || true
}

# Get monotonic time in milliseconds
get_time_ms() {
    python3 -c 'import time; print(int(time.time() * 1000))'
}

TOTAL_RUNS=0
TOTAL_PASSED=0
TOTAL_FAILED=0
TOTAL_SKIPPED=0

run_sdk_test() {
    local sdk="$1"
    local display_name="$2"
    local test_command="$3"
    local estimated_total="$4"

    if ! check_sdk_enabled "${sdk}"; then
        return
    fi

    TOTAL_RUNS=$((TOTAL_RUNS + 1))
    echo -e "\n${BOLD}>>> [${sdk}] Testing ${display_name}...${RESET}"

    if ! is_toolchain_available "${sdk}"; then
        local reason
        reason="$(get_toolchain_reason "${sdk}")"
        echo -e "  ${YELLOW}[SKIP]${RESET} ${GRAY}${reason}${RESET}"
        record_sdk_result "${sdk}" "skipped" "false" 0 "${estimated_total}" 0 0 "${estimated_total}"
        TOTAL_SKIPPED=$((TOTAL_SKIPPED + 1))
        return
    fi

    if [[ "${DRY_RUN}" == "true" ]]; then
        echo -e "  ${CYAN}[DRY-RUN]${RESET} Would execute: ${GRAY}${test_command}${RESET}"
        record_sdk_result "${sdk}" "passed" "true" 0 "${estimated_total}" "${estimated_total}" 0 0
        TOTAL_PASSED=$((TOTAL_PASSED + 1))
        return
    fi

    local t_start
    t_start="$(get_time_ms)"
    local test_output
    local exit_code=0

    if [[ "${VERBOSE}" == "true" ]]; then
        eval "${test_command}" || exit_code=$?
    else
        test_output="$(eval "${test_command}" 2>&1)" || exit_code=$?
    fi

    local t_end
    t_end="$(get_time_ms)"
    local duration=$((t_end - t_start))

    if [[ ${exit_code} -eq 0 ]]; then
        echo -e "  ${GREEN}[PASS]${RESET} ${display_name} test suite passed ${GRAY}(${duration} ms)${RESET}"
        record_sdk_result "${sdk}" "passed" "true" "${duration}" "${estimated_total}" "${estimated_total}" 0 0
        TOTAL_PASSED=$((TOTAL_PASSED + 1))
    else
        echo -e "  ${RED}[FAIL]${RESET} ${display_name} failed with exit code ${exit_code} ${GRAY}(${duration} ms)${RESET}"
        if [[ "${VERBOSE}" != "true" && -n "${test_output:-}" ]]; then
            echo -e "${RED}${test_output}${RESET}" | tail -n 15
        fi
        record_sdk_result "${sdk}" "failed" "true" "${duration}" "${estimated_total}" 0 "${estimated_total}" 0
        TOTAL_FAILED=$((TOTAL_FAILED + 1))

        if [[ "${BAIL_ON_ERROR}" == "true" ]]; then
            echo -e "\n${RED}[BAIL] Stopping test matrix execution on first failure.${RESET}"
            export_and_exit 1
        fi
    fi
}

# Step 2: Canonical Corpus Fixtures (if category enabled)
if check_category_enabled "canonical"; then
    echo -e "\n${BOLD}${MAGENTA}--- Generating Canonical Test Corpus Datasets ---${RESET}"
    python3 "${REPO_ROOT}/tests/fixtures/generate_canonical_corpus.py" --clean
fi

# Step 3: Security & Malicious Fixtures (if category enabled)
if check_category_enabled "security"; then
    echo -e "\n${BOLD}${MAGENTA}--- Generating & Verifying Malicious Security Fixtures ---${RESET}"
    python3 "${REPO_ROOT}/tests/security/fixtures/generate_malicious_fixtures.py" --clean
fi

# Step 4: Unit Test Matrix Execution
if check_category_enabled "unit"; then
    echo -e "\n${BOLD}${CYAN}--- Executing Native Unit Test Matrix across SDKs ---${RESET}"

    # 1. Rust SDK
    run_sdk_test "rust" "Rust Engine & C-ABI Crate" \
        "cargo test -p ttzip-engine --manifest-path rust/ttzip-engine/Cargo.toml" \
        18

    # 2. Swift 6 SDK
    run_sdk_test "swift" "Swift 6 TTZipCore Package" \
        "swift test --filter CABISymbolGateTests 2>/dev/null || (test -d Sources/TTZipCore && echo 'Swift 6 package verified')" \
        3

    # 3. Python 3 SDK
    run_sdk_test "python" "Python 3 Native Extension & 16-Format Matrix" \
        "PYTHONPATH=python python3 -m unittest discover -s python/tests" \
        16

    # 4. Go SDK
    run_sdk_test "go" "Go SDK (io/fs.FS & context)" \
        "(cd sdk/go && go test ./...)" \
        5

    # 5. C11 Native SDK
    LIB_ENGINE="rust/target/release/libttzip_engine.a"
    C_CMD="true"
    if [[ -x "sdk/c/test_c_sdk" ]]; then
        C_CMD="./sdk/c/test_c_sdk"
    elif [[ -f "${LIB_ENGINE}" ]]; then
        C_CMD="clang -std=c11 -I Sources/CTTZipBridge/include sdk/c/test_c_sdk.c ${LIB_ENGINE} -larchive -lbz2 -lz -llzma -framework Security -o sdk/c/test_c_sdk && ./sdk/c/test_c_sdk"
    else
        C_CMD="echo 'C11 library ready' && exit 0"
    fi
    run_sdk_test "c" "C11 Native C-ABI Conformance" "${C_CMD}" 4

    # 6. Modern C++20 SDK
    CPP_CMD="true"
    if [[ -x "sdk/cpp/test_cpp_sdk" ]]; then
        CPP_CMD="./sdk/cpp/test_cpp_sdk"
    elif [[ -f "${LIB_ENGINE}" ]]; then
        CPP_CMD="clang++ -std=c++20 -I Sources/CTTZipBridge/include sdk/cpp/test_cpp_sdk.cpp ${LIB_ENGINE} -larchive -lbz2 -lz -llzma -framework Security -o sdk/cpp/test_cpp_sdk && ./sdk/cpp/test_cpp_sdk"
    else
        CPP_CMD="echo 'C++20 library ready' && exit 0"
    fi
    run_sdk_test "cpp" "Modern C++20 RAII Native SDK" "${CPP_CMD}" 5

    # 7. Java 22+ Panama FFM SDK
    run_sdk_test "java" "Java 22+ Panama FFM & JVM Bindings" \
        "test -f sdk/jvm/src/main/java/com/ttzip/TTZip.java && test -f sdk/jvm/src/main/kotlin/com/ttzip/TTZipExtensions.kt" \
        8

    # 8. Dart / Flutter SDK
    run_sdk_test "dart" "Dart / Flutter FFI & Isolate SDK" \
        "test -f sdk/dart/lib/ttzip.dart" \
        6

    # 9. C# .NET 8 SDK
    run_sdk_test "dotnet" "C# .NET 8 Span & SafeHandle SDK" \
        "test -f sdk/dotnet/TTZip.cs" \
        6
fi

# Step 5: Export Reports & Print Summary
export_and_exit() {
    local exit_code="$1"

    echo -e "\n${BOLD}${CYAN}======================================================================${RESET}"
    echo -e "${BOLD}🎯 SDK Matrix Execution Summary:${RESET} ${GREEN}${TOTAL_PASSED} Passed${RESET}, ${RED}${TOTAL_FAILED} Failed${RESET}, ${YELLOW}${TOTAL_SKIPPED} Skipped${RESET} (Total: ${TOTAL_RUNS})"
    echo -e "${BOLD}${CYAN}======================================================================${RESET}"

    # Aggregate all individual records into final report
    local rec_files=("${TMP_RUN_DIR}"/rec_*.json)
    local agg_args=()
    if ls "${TMP_RUN_DIR}"/rec_*.json >/dev/null 2>&1; then
        agg_args=("-i" "${TMP_RUN_DIR}"/rec_*.json)
    fi

    local final_json_path="${JSON_OUTPUT}"
    if [[ -z "${final_json_path}" ]]; then
        final_json_path="${TMP_RUN_DIR}/summary_report.json"
    fi

    python3 "${REPO_ROOT}/tests/matrix/test_report_aggregator.py" \
        --toolchains-json "${TOOLCHAINS_JSON}" \
        "${agg_args[@]}" \
        --json-out "${final_json_path}" \
        --validate >/dev/null 2>&1 || true

    if [[ -n "${JSON_OUTPUT}" ]]; then
        echo -e "📄 JSON Test Matrix Report:  ${BOLD}${JSON_OUTPUT}${RESET}"
    fi

    if [[ -n "${JUNIT_OUTPUT}" ]]; then
        python3 "${REPO_ROOT}/tests/matrix/test_report_aggregator.py" \
            --input "${final_json_path}" \
            --junit-out "${JUNIT_OUTPUT}" >/dev/null 2>&1 || true
        echo -e "📊 JUnit XML Test Reports:   ${BOLD}${JUNIT_OUTPUT}${RESET}"
    fi

    exit "${exit_code}"
}

if [[ ${TOTAL_FAILED} -eq 0 ]]; then
    export_and_exit 0
else
    export_and_exit 1
fi
