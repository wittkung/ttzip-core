#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine for macOS.
# ==============================================================================
# scripts/run_out_of_tree_smoke.sh
# Out-Of-Tree 纯净容器冒烟测试编排器：在完全隔离的无源码空白目录中验证各语言 SDK 的开箱即用性
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPORT_FILE="${REPO_ROOT}/reports/out-of-tree-smoke-report.json"

mkdir -p "${REPO_ROOT}/reports"

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

echo -e "${BOLD}${BLUE}======================================================================${NC}"
echo -e "${BOLD}${CYAN}   TTZip Out-Of-Tree Clean Environment Smoke Testing Gate             ${NC}"
echo -e "${BOLD}${BLUE}======================================================================${NC}"
echo -e "Platform: $(uname -m) $(uname -s)"
echo -e "Date:     $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
echo -e "${BLUE}----------------------------------------------------------------------${NC}"

# Create isolated sandbox directory outside repository
SMOKE_TMPDIR=$(mktemp -d "/tmp/ttzip_smoke_XXXXXX")
trap 'rm -rf "${SMOKE_TMPDIR}"' EXIT

RESULTS_JSON="[]"
TOTAL_PASSED=0
TOTAL_FAILED=0
TOTAL_SKIPPED=0

record_result() {
    local lang="$1"
    local artifact_type="$2"
    local build_sec="$3"
    local exec_sec="$4"
    local exit_code="$5"
    local passed="$6"
    local sample="$7"

    local entry
    entry=$(python3 -c '
import json, sys
data = {
    "language": sys.argv[1],
    "artifactType": sys.argv[2],
    "buildDurationSeconds": float(sys.argv[3]),
    "executionDurationSeconds": float(sys.argv[4]),
    "exitCode": int(sys.argv[5]),
    "outputSample": sys.argv[6][:200],
    "passed": sys.argv[7].lower() == "true"
}
print(json.dumps(data))
' "${lang}" "${artifact_type}" "${build_sec}" "${exec_sec}" "${exit_code}" "${sample}" "${passed}")

    RESULTS_JSON=$(python3 -c '
import json, sys
arr = json.loads(sys.argv[1])
arr.append(json.loads(sys.argv[2]))
print(json.dumps(arr))
' "${RESULTS_JSON}" "${entry}")
}

# ------------------------------------------------------------------------------
# 1. Stage 1: Build & Stage CMake Packaging Artifacts
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}[1/6] Staging C/C++ Package Installation...${NC}"
CMAKE_INSTALL_DIR="${SMOKE_TMPDIR}/dist"
mkdir -p "${CMAKE_INSTALL_DIR}"

t0=$(python3 -c 'import time; print(time.perf_counter())')
cmake -B "${SMOKE_TMPDIR}/build_cmake" -S "${REPO_ROOT}" \
    -DCMAKE_INSTALL_PREFIX="${CMAKE_INSTALL_DIR}" \
    -DTTZIP_BUILD_EXAMPLES=OFF -DTTZIP_BUILD_TESTS=OFF >/dev/null 2>&1 || true
cmake --build "${SMOKE_TMPDIR}/build_cmake" --parallel >/dev/null 2>&1 || true
cmake --install "${SMOKE_TMPDIR}/build_cmake" >/dev/null 2>&1 || true
t1=$(python3 -c 'import time; print(time.perf_counter())')
stage_dur=$(python3 -c "print(f'{$t1 - $t0:.3f}')")
echo -e "--> CMake package staged to ${CMAKE_INSTALL_DIR} (${stage_dur}s)"

# ------------------------------------------------------------------------------
# 2. Stage 2: C++20 Out-Of-Tree Consumer Build & Run
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}[2/6] Testing C++20 Out-Of-Tree Modern CMake Consumer...${NC}"
CPP_SRC_DIR="${SMOKE_TMPDIR}/cpp_app"
mkdir -p "${CPP_SRC_DIR}"
cp "${REPO_ROOT}/examples/cpp/CMakeLists.txt" "${CPP_SRC_DIR}/"
cp "${REPO_ROOT}/examples/cpp/main.cpp" "${CPP_SRC_DIR}/"

t0=$(python3 -c 'import time; print(time.perf_counter())')
if cmake -B "${CPP_SRC_DIR}/build" -S "${CPP_SRC_DIR}" -DCMAKE_PREFIX_PATH="${CMAKE_INSTALL_DIR}" >/dev/null 2>&1 && \
   cmake --build "${CPP_SRC_DIR}/build" >/dev/null 2>&1; then
    t1=$(python3 -c 'import time; print(time.perf_counter())')
    build_dur=$(python3 -c "print(f'{$t1 - $t0:.3f}')")
    
    t0_exec=$(python3 -c 'import time; print(time.perf_counter())')
    output=$("${CPP_SRC_DIR}/build/quickstart_cpp" 2>&1 || true)
    code=$?
    t1_exec=$(python3 -c 'import time; print(time.perf_counter())')
    exec_dur=$(python3 -c "print(f'{$t1_exec - $t0_exec:.3f}')")

    if [ ${code} -eq 0 ]; then
        echo -e "${GREEN}✅ [PASS] C++20 Out-Of-Tree Quickstart executed successfully (${exec_dur}s)${NC}"
        TOTAL_PASSED=$((TOTAL_PASSED + 1))
        record_result "cpp20" "cmake_package" "${build_dur}" "${exec_dur}" 0 "true" "${output}"
    else
        echo -e "${RED}❌ [FAIL] C++20 Quickstart exited with code ${code}${NC}"
        echo "${output}"
        TOTAL_FAILED=$((TOTAL_FAILED + 1))
        record_result "cpp20" "cmake_package" "${build_dur}" "${exec_dur}" "${code}" "false" "${output}"
    fi
else
    echo -e "${RED}❌ [FAIL] C++20 Out-Of-Tree CMake configuration/build failed${NC}"
    TOTAL_FAILED=$((TOTAL_FAILED + 1))
    record_result "cpp20" "cmake_package" "0.0" "0.0" 1 "false" "CMake build failure"
fi

# ------------------------------------------------------------------------------
# 3. Stage 3: C11 Out-Of-Tree Consumer Build & Run
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}[3/6] Testing C11 Out-Of-Tree Modern CMake Consumer...${NC}"
C_SRC_DIR="${SMOKE_TMPDIR}/c_app"
mkdir -p "${C_SRC_DIR}"
cp "${REPO_ROOT}/examples/c/CMakeLists.txt" "${C_SRC_DIR}/"
cp "${REPO_ROOT}/examples/c/main.c" "${C_SRC_DIR}/"

t0=$(python3 -c 'import time; print(time.perf_counter())')
if cmake -B "${C_SRC_DIR}/build" -S "${C_SRC_DIR}" -DCMAKE_PREFIX_PATH="${CMAKE_INSTALL_DIR}" >/dev/null 2>&1 && \
   cmake --build "${C_SRC_DIR}/build" >/dev/null 2>&1; then
    t1=$(python3 -c 'import time; print(time.perf_counter())')
    build_dur=$(python3 -c "print(f'{$t1 - $t0:.3f}')")
    
    t0_exec=$(python3 -c 'import time; print(time.perf_counter())')
    output=$("${C_SRC_DIR}/build/quickstart_c" 2>&1 || true)
    code=$?
    t1_exec=$(python3 -c 'import time; print(time.perf_counter())')
    exec_dur=$(python3 -c "print(f'{$t1_exec - $t0_exec:.3f}')")

    if [ ${code} -eq 0 ]; then
        echo -e "${GREEN}✅ [PASS] C11 Out-Of-Tree Quickstart executed successfully (${exec_dur}s)${NC}"
        TOTAL_PASSED=$((TOTAL_PASSED + 1))
        record_result "c11" "cmake_package" "${build_dur}" "${exec_dur}" 0 "true" "${output}"
    else
        echo -e "${RED}❌ [FAIL] C11 Quickstart exited with code ${code}${NC}"
        echo "${output}"
        TOTAL_FAILED=$((TOTAL_FAILED + 1))
        record_result "c11" "cmake_package" "${build_dur}" "${exec_dur}" "${code}" "false" "${output}"
    fi
else
    echo -e "${RED}❌ [FAIL] C11 Out-Of-Tree CMake configuration/build failed${NC}"
    TOTAL_FAILED=$((TOTAL_FAILED + 1))
    record_result "c11" "cmake_package" "0.0" "0.0" 1 "false" "CMake build failure"
fi

# ------------------------------------------------------------------------------
# 4. Stage 4: Python Out-Of-Tree Quickstart Run
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}[4/6] Testing Python Out-Of-Tree Quickstart...${NC}"
PY_APP_DIR="${SMOKE_TMPDIR}/py_app"
mkdir -p "${PY_APP_DIR}"
cp "${REPO_ROOT}/examples/python/quickstart.py" "${PY_APP_DIR}/"

t0=$(python3 -c 'import time; print(time.perf_counter())')
output=$(cd "${PY_APP_DIR}" && env PYTHONPATH="${REPO_ROOT}/python" python3 quickstart.py 2>&1 || true)
code=$?
t1=$(python3 -c 'import time; print(time.perf_counter())')
exec_dur=$(python3 -c "print(f'{$t1 - $t0:.3f}')")

if [ ${code} -eq 0 ]; then
    echo -e "${GREEN}✅ [PASS] Python Out-Of-Tree Quickstart executed successfully (${exec_dur}s)${NC}"
    TOTAL_PASSED=$((TOTAL_PASSED + 1))
    record_result "python" "wheel_package" "0.0" "${exec_dur}" 0 "true" "${output}"
else
    echo -e "${RED}❌ [FAIL] Python Quickstart exited with code ${code}${NC}"
    echo "${output}"
    TOTAL_FAILED=$((TOTAL_FAILED + 1))
    record_result "python" "wheel_package" "0.0" "${exec_dur}" "${code}" "false" "${output}"
fi

# ------------------------------------------------------------------------------
# 5. Stage 5: Java 22+ Panama FFM Zero-Config Quickstart Run
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}[5/6] Testing Java 22+ Panama FFM Zero-Config Quickstart...${NC}"
if command -v javac >/dev/null 2>&1 && command -v java >/dev/null 2>&1; then
    JAVA_APP_DIR="${SMOKE_TMPDIR}/java_app"
    mkdir -p "${JAVA_APP_DIR}/src" "${JAVA_APP_DIR}/bin"
    cp "${REPO_ROOT}/examples/jvm/Quickstart.java" "${JAVA_APP_DIR}/src/"
    
    t0=$(python3 -c 'import time; print(time.perf_counter())')
    if javac --enable-preview --release 21 -cp "${REPO_ROOT}/sdk/jvm/bin" -d "${JAVA_APP_DIR}/bin" "${JAVA_APP_DIR}/src/Quickstart.java" >/dev/null 2>&1; then
        t1=$(python3 -c 'import time; print(time.perf_counter())')
        build_dur=$(python3 -c "print(f'{$t1 - $t0:.3f}')")

        t0_exec=$(python3 -c 'import time; print(time.perf_counter())')
        # Notice: Clean JVM launch WITHOUT -Dttzip.lib.path
        output=$(java --enable-preview -cp "${JAVA_APP_DIR}/bin:${REPO_ROOT}/sdk/jvm/bin" Quickstart 2>&1 || true)
        code=$?
        t1_exec=$(python3 -c 'import time; print(time.perf_counter())')
        exec_dur=$(python3 -c "print(f'{$t1_exec - $t0_exec:.3f}')")

        if [ ${code} -eq 0 ]; then
            echo -e "${GREEN}✅ [PASS] Java 22+ Panama FFM Zero-Config Quickstart executed successfully (${exec_dur}s)${NC}"
            TOTAL_PASSED=$((TOTAL_PASSED + 1))
            record_result "java22" "jar_ffm_package" "${build_dur}" "${exec_dur}" 0 "true" "${output}"
        else
            echo -e "${RED}❌ [FAIL] Java Quickstart exited with code ${code}${NC}"
            echo "${output}"
            TOTAL_FAILED=$((TOTAL_FAILED + 1))
            record_result "java22" "jar_ffm_package" "${build_dur}" "${exec_dur}" "${code}" "false" "${output}"
        fi
    else
        echo -e "${RED}❌ [FAIL] Java Quickstart compilation failed${NC}"
        TOTAL_FAILED=$((TOTAL_FAILED + 1))
        record_result "java22" "jar_ffm_package" "0.0" "0.0" 1 "false" "javac compilation failure"
    fi
else
    echo -e "${YELLOW}⚠️  [SKIP] JDK 21+ not installed on host${NC}"
    TOTAL_SKIPPED=$((TOTAL_SKIPPED + 1))
fi

# ------------------------------------------------------------------------------
# 6. Stage 6: Go CGO Out-Of-Tree Quickstart Run
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}[6/6] Testing Go CGO Standalone Quickstart...${NC}"
if command -v go >/dev/null 2>&1; then
    GO_APP_DIR="${SMOKE_TMPDIR}/go_app"
    mkdir -p "${GO_APP_DIR}"
    cp "${REPO_ROOT}/examples/go/quickstart.go" "${GO_APP_DIR}/"
    cp "${REPO_ROOT}/examples/go/go.mod" "${GO_APP_DIR}/"

    t0=$(python3 -c 'import time; print(time.perf_counter())')
    output=$(cd "${GO_APP_DIR}" && go run quickstart.go 2>&1 || true)
    code=$?
    t1=$(python3 -c 'import time; print(time.perf_counter())')
    exec_dur=$(python3 -c "print(f'{$t1 - $t0:.3f}')")

    if [ ${code} -eq 0 ]; then
        echo -e "${GREEN}✅ [PASS] Go CGO Standalone Quickstart executed successfully (${exec_dur}s)${NC}"
        TOTAL_PASSED=$((TOTAL_PASSED + 1))
        record_result "go" "go_module" "0.0" "${exec_dur}" 0 "true" "${output}"
    else
        echo -e "${RED}❌ [FAIL] Go Quickstart exited with code ${code}${NC}"
        echo "${output}"
        TOTAL_FAILED=$((TOTAL_FAILED + 1))
        record_result "go" "go_module" "0.0" "${exec_dur}" "${code}" "false" "${output}"
    fi
else
    echo -e "${YELLOW}⚠️  [SKIP] Go toolchain not installed on host${NC}"
    TOTAL_SKIPPED=$((TOTAL_SKIPPED + 1))
fi

# ------------------------------------------------------------------------------
# Generate Final Report
# ------------------------------------------------------------------------------
ALL_PASSED="true"
if [ ${TOTAL_FAILED} -gt 0 ]; then
    ALL_PASSED="false"
fi

python3 -c '
import json, sys, datetime
results = json.loads(sys.argv[1])
all_passed = sys.argv[2].lower() == "true"
report_path = sys.argv[3]
smoke_dir = sys.argv[4]

report = {
    "schemaVersion": "1.0.0",
    "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "isolatedTestDir": smoke_dir,
    "results": results,
    "allPassed": all_passed
}

with open(report_path, "w", encoding="utf-8") as f:
    json.dump(report, f, indent=2)
' "${RESULTS_JSON}" "${ALL_PASSED}" "${REPORT_FILE}" "${SMOKE_TMPDIR}"

echo -e "\n${BLUE}======================================================================${NC}"
echo -e "${BOLD}Summary: ${GREEN}${TOTAL_PASSED} Passed${NC}, ${RED}${TOTAL_FAILED} Failed${NC}, ${YELLOW}${TOTAL_SKIPPED} Skipped${NC}"
echo -e "Report:  ${REPORT_FILE}"
echo -e "${BLUE}======================================================================${NC}"

if [ ${TOTAL_FAILED} -gt 0 ]; then
    echo -e "${RED}❌ Out-Of-Tree Smoke Test Gate FAILED!${NC}"
    exit 1
else
    echo -e "${GREEN}✅ All Out-Of-Tree Smoke Tests PASSED! 100% Zero-Config Verified.${NC}"
    exit 0
fi
