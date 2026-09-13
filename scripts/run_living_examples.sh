#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# run_living_examples.sh: Automated Living Examples Compilation & Execution Quality Gate
# Validates in-tree sample applications across multi-language ecosystems to prevent API decay.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

export CLANG_MODULE_CACHE_PATH="${REPO_ROOT}/.build/clang-module-cache"
export SWIFT_MODULE_CACHE_PATH="${REPO_ROOT}/.build/swift-module-cache"
mkdir -p "${CLANG_MODULE_CACHE_PATH}" "${SWIFT_MODULE_CACHE_PATH}" 2>/dev/null || true

# ANSI Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

echo -e "${BOLD}${BLUE}======================================================================${NC}"
echo -e "${BOLD}${CYAN}   TTZip In-Tree Living Examples Quality & Anti-Decay Gate            ${NC}"
echo -e "${BOLD}${BLUE}======================================================================${NC}"
echo -e "Platform: $(uname -m) $(uname -s)"
echo -e "Date:     $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
echo -e "${BLUE}----------------------------------------------------------------------${NC}"

BUILD_DIR="${REPO_ROOT}/.build/living_examples"
mkdir -p "${BUILD_DIR}"

TOTAL_PASSED=0
TOTAL_FAILED=0
TOTAL_SKIPPED=0

record_pass() {
    local name="$1"
    local dur="$2"
    echo -e "${GREEN}✅ [PASS] ${name} (${dur}s)${NC}"
    TOTAL_PASSED=$((TOTAL_PASSED + 1))
}

record_fail() {
    local name="$1"
    local details="$2"
    echo -e "${RED}❌ [FAIL] ${name}${NC}"
    echo -e "${RED}${details}${NC}"
    TOTAL_FAILED=$((TOTAL_FAILED + 1))
}

record_skip() {
    local name="$1"
    local reason="$2"
    echo -e "${YELLOW}⚠️  [SKIP] ${name}: ${reason}${NC}"
    TOTAL_SKIPPED=$((TOTAL_SKIPPED + 1))
}

# ------------------------------------------------------------------------------
# 1. Rust Living Example (examples/rust)
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}[1/7] Testing Pure Safe Rust Living Example...${NC}"
if [ -f "${REPO_ROOT}/examples/rust/Cargo.toml" ]; then
    t0=$(python3 -c 'import time; print(time.perf_counter())')
    set +e
    output=$(cargo check --manifest-path "${REPO_ROOT}/examples/rust/Cargo.toml" --offline 2>&1)
    code=$?
    set -e
    t1=$(python3 -c 'import time; print(time.perf_counter())')
    dur=$(python3 -c "print(f'{$t1 - $t0:.3f}')")
    if [ ${code} -eq 0 ]; then
        record_pass "Rust Living Example (cargo check)" "${dur}"
    else
        record_fail "Rust Living Example" "${output}"
    fi
else
    record_skip "Rust Living Example" "Cargo.toml not found"
fi

# ------------------------------------------------------------------------------
# 2. Swift 6 Concurrency Living Example (examples/swift)
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}[2/7] Testing Swift 6 Concurrency & Actor Living Example...${NC}"
if [ -f "${REPO_ROOT}/examples/swift/Package.swift" ] && command -v swift >/dev/null 2>&1; then
    t0=$(python3 -c 'import time; print(time.perf_counter())')
    set +e
    build_out=$(swift build --disable-sandbox --package-path "${REPO_ROOT}/examples/swift" 2>&1)
    build_code=$?
    set -e
    if [ ${build_code} -eq 0 ]; then
        set +e
        exec_out=$(swift run --disable-sandbox --package-path "${REPO_ROOT}/examples/swift" 2>&1)
        exec_code=$?
        set -e
        t1=$(python3 -c 'import time; print(time.perf_counter())')
        dur=$(python3 -c "print(f'{$t1 - $t0:.3f}')")
        if [ ${exec_code} -eq 0 ]; then
            record_pass "Swift 6 Concurrency Living Example" "${dur}"
        else
            record_fail "Swift 6 Living Example execution" "${exec_out}"
        fi
    else
        t1=$(python3 -c 'import time; print(time.perf_counter())')
        record_fail "Swift 6 Living Example build" "${build_out}"
    fi
else
    record_skip "Swift Living Example" "Swift toolchain not available"
fi

# ------------------------------------------------------------------------------
# 3. Python 3 Living Examples (quickstart & advanced_example)
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}[3/7] Testing Python 3 Living Examples...${NC}"
if command -v python3 >/dev/null 2>&1; then
    t0=$(python3 -c 'import time; print(time.perf_counter())')
    set +e
    py1_out=$(PYTHONPATH="${REPO_ROOT}/sdk/python" python3 "${REPO_ROOT}/examples/python/quickstart.py" 2>&1)
    py1_code=$?
    py2_out=$(PYTHONPATH="${REPO_ROOT}/sdk/python" python3 "${REPO_ROOT}/examples/python/advanced_example.py" 2>&1)
    py2_code=$?
    set -e
    t1=$(python3 -c 'import time; print(time.perf_counter())')
    dur=$(python3 -c "print(f'{$t1 - $t0:.3f}')")
    if [ ${py1_code} -eq 0 ] && [ ${py2_code} -eq 0 ]; then
        record_pass "Python Living Examples (quickstart + advanced)" "${dur}"
    else
        record_fail "Python Living Examples" "${py1_out}\n${py2_out}"
    fi
else
    record_skip "Python Living Examples" "Python 3 not installed"
fi

# ------------------------------------------------------------------------------
# 4. Stage Native C/C++ Package for Consumers
# ------------------------------------------------------------------------------
STAGED_DIST="${BUILD_DIR}/dist"
if [ ! -f "${STAGED_DIST}/lib/pkgconfig/ttzip.pc" ]; then
    mkdir -p "${STAGED_DIST}"
    cmake -B "${BUILD_DIR}/cmake_stage" -S "${REPO_ROOT}" \
        -DCMAKE_INSTALL_PREFIX="${STAGED_DIST}" \
        -DTTZIP_BUILD_EXAMPLES=OFF -DTTZIP_BUILD_TESTS=OFF >/dev/null 2>&1 || true
    cmake --build "${BUILD_DIR}/cmake_stage" --parallel >/dev/null 2>&1 || true
    cmake --install "${BUILD_DIR}/cmake_stage" >/dev/null 2>&1 || true
fi

# ------------------------------------------------------------------------------
# 5. C11 & C++20 Living Examples
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}[4/7] Testing C11 & C++20 Living Examples...${NC}"
if command -v cmake >/dev/null 2>&1; then
    t0=$(python3 -c 'import time; print(time.perf_counter())')
    set +e
    c_ok=false
    cpp_ok=false
    if cmake -B "${BUILD_DIR}/c_app" -S "${REPO_ROOT}/examples/c" -DCMAKE_PREFIX_PATH="${STAGED_DIST}" >/dev/null 2>&1 && \
       cmake --build "${BUILD_DIR}/c_app" >/dev/null 2>&1 && \
       "${BUILD_DIR}/c_app/ttzip_c_demo" >/dev/null 2>&1; then
        c_ok=true
    fi
    if cmake -B "${BUILD_DIR}/cpp_app" -S "${REPO_ROOT}/examples/cpp" -DCMAKE_PREFIX_PATH="${STAGED_DIST}" >/dev/null 2>&1 && \
       cmake --build "${BUILD_DIR}/cpp_app" >/dev/null 2>&1 && \
       "${BUILD_DIR}/cpp_app/ttzip_cpp_demo" >/dev/null 2>&1; then
        cpp_ok=true
    fi
    set -e
    t1=$(python3 -c 'import time; print(time.perf_counter())')
    dur=$(python3 -c "print(f'{$t1 - $t0:.3f}')")
    if [ "${c_ok}" = true ] && [ "${cpp_ok}" = true ]; then
        record_pass "C11 & C++20 Living Examples (CMake + run)" "${dur}"
    else
        record_fail "C11/C++20 Living Examples" "C status: ${c_ok}, C++ status: ${cpp_ok}"
    fi
else
    record_skip "C11 & C++20 Living Examples" "CMake not installed"
fi

# ------------------------------------------------------------------------------
# 6. Go Living Examples (examples/go)
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}[5/7] Testing Go CGO Living Examples...${NC}"
if command -v go >/dev/null 2>&1; then
    t0=$(python3 -c 'import time; print(time.perf_counter())')
    set +e
    go1_out=$(cd "${REPO_ROOT}/examples/go" && env PKG_CONFIG_PATH="${STAGED_DIST}/lib/pkgconfig" go run -a quickstart.go 2>&1)
    go1_code=$?
    go2_out=$(cd "${REPO_ROOT}/examples/go" && env PKG_CONFIG_PATH="${STAGED_DIST}/lib/pkgconfig" go run -a advanced_example.go 2>&1)
    go2_code=$?
    set -e
    t1=$(python3 -c 'import time; print(time.perf_counter())')
    dur=$(python3 -c "print(f'{$t1 - $t0:.3f}')")
    if [ ${go1_code} -eq 0 ] && [ ${go2_code} -eq 0 ]; then
        record_pass "Go Living Examples (quickstart + advanced)" "${dur}"
    else
        record_fail "Go Living Examples" "${go1_out}\n${go2_out}"
    fi
else
    record_skip "Go Living Examples" "Go toolchain not installed"
fi

# ------------------------------------------------------------------------------
# 7. Java 22+ Panama FFM Living Examples (examples/jvm)
# ------------------------------------------------------------------------------
echo -e "\n${BOLD}[6/7] Testing Java 22+ Panama FFM Living Examples...${NC}"
if command -v javac >/dev/null 2>&1 && command -v java >/dev/null 2>&1; then
    t0=$(python3 -c 'import time; print(time.perf_counter())')
    mkdir -p "${REPO_ROOT}/sdk/jvm/bin" "${BUILD_DIR}/jvm_bin"
    javac --enable-preview --release 21 -d "${REPO_ROOT}/sdk/jvm/bin" "${REPO_ROOT}/sdk/jvm/src/main/java/com/ttzip/"*.java >/dev/null 2>&1 || true

    set +e
    j_compile_out=$(javac --enable-preview --release 21 -cp "${REPO_ROOT}/sdk/jvm/bin" -d "${BUILD_DIR}/jvm_bin" \
        "${REPO_ROOT}/examples/jvm/Quickstart.java" "${REPO_ROOT}/examples/jvm/AdvancedExample.java" 2>&1)
    j_compile_code=$?
    set -e

    if [ ${j_compile_code} -eq 0 ]; then
        set +e
        j1_out=$(java --enable-preview -cp "${BUILD_DIR}/jvm_bin:${REPO_ROOT}/sdk/jvm/bin" com.ttzip.examples.Quickstart 2>&1)
        j1_code=$?
        j2_out=$(java --enable-preview -cp "${BUILD_DIR}/jvm_bin:${REPO_ROOT}/sdk/jvm/bin" com.ttzip.examples.AdvancedExample 2>&1)
        j2_code=$?
        set -e
        t1=$(python3 -c 'import time; print(time.perf_counter())')
        dur=$(python3 -c "print(f'{$t1 - $t0:.3f}')")
        if [ ${j1_code} -eq 0 ] && [ ${j2_code} -eq 0 ]; then
            record_pass "Java 22+ Panama FFM Living Examples (quickstart + advanced)" "${dur}"
        else
            record_fail "Java Living Examples execution" "${j1_out}\n${j2_out}"
        fi
    else
        record_fail "Java Living Examples compilation" "${j_compile_out}"
    fi
else
    record_skip "Java Living Examples" "JDK 21+ not installed"
fi

# ------------------------------------------------------------------------------
# Final Summary
# ------------------------------------------------------------------------------
echo -e "\n${BLUE}======================================================================${NC}"
echo -e "${BOLD}Living Examples Summary: ${GREEN}${TOTAL_PASSED} Passed${NC}, ${RED}${TOTAL_FAILED} Failed${NC}, ${YELLOW}${TOTAL_SKIPPED} Skipped${NC}"
echo -e "${BLUE}======================================================================${NC}"

if [ ${TOTAL_FAILED} -gt 0 ]; then
    echo -e "${RED}❌ Living Examples Quality Gate FAILED! Fix decaying examples.${NC}"
    exit 1
else
    echo -e "${GREEN}✅ All Living Examples PASSED! Code decay prevented.${NC}"
    exit 0
fi
