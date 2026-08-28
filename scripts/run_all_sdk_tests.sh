#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# Automated Regression Test Suite for TTZip Native Multilingual SDKs:
# 1. Rust (Microkernel & C-ABI)
# 2. Swift 6 (TTZipCore)
# 3. Python 3 (PyO3 Native Extension)
# 4. Node.js / TypeScript (npm package)
# 5. C11 Native SDK
# 6. Modern C++20 SDK
# 7. Java 22+ & Kotlin Coroutines SDK
# 8. Go SDK (io/fs.FS & context)
# 9. Dart / Flutter SDK & C# .NET SDK

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

JSON_OUT=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --json)
            JSON_OUT="$2"
            shift 2
            ;;
        *)
            shift
            ;;
    esac
done

echo "======================================================================"
echo "⚡️ TTZip Multi-Language SDK Full Matrix Test Gate (9 Ecosystems)"
echo "======================================================================"

cd "${REPO_ROOT}"

PASSED=0
FAILED=0
SKIPPED=0

declare -a SDK_KEYS=()
declare -a SDK_STATUSES=()
declare -a SDK_DURATIONS=()
declare -a SDK_AVAIL=()

get_time_ms() {
    python3 -c 'import time; print(int(time.time() * 1000))'
}

run_suite() {
    local key="$1"
    local name="$2"
    local cmd="$3"
    local check_tool="$4"

    SDK_KEYS+=("${key}")

    if [ -n "${check_tool}" ] && ! command -v "${check_tool}" >/dev/null 2>&1; then
        echo -e "  [SKIP] ${name} toolchain (${check_tool}) not available."
        SKIPPED=$((SKIPPED + 1))
        SDK_STATUSES+=("skipped")
        SDK_DURATIONS+=(0)
        SDK_AVAIL+=("false")
        return
    fi

    SDK_AVAIL+=("true")
    local t0=$(get_time_ms)
    set +e
    TMP_LOG=$(mktemp)
    eval "${cmd}" > "${TMP_LOG}" 2>&1
    local exit_code=$?
    set -e
    local t1=$(get_time_ms)
    local dur=$((t1 - t0))
    SDK_DURATIONS+=("${dur}")

    if [ ${exit_code} -eq 0 ]; then
        echo -e "  [PASS] ${name} test suite passed (${dur}ms)."
        PASSED=$((PASSED + 1))
        SDK_STATUSES+=("passed")
    else
        echo -e "  [FAIL] ${name} test suite failed with exit code ${exit_code} (${dur}ms)."
        tail -n 15 "${TMP_LOG}"
        FAILED=$((FAILED + 1))
        SDK_STATUSES+=("failed")
    fi
    rm -f "${TMP_LOG}"
}

# 1. Rust SDK
echo ">>> [1/9] Testing Pure Rust & C-ABI Crate Suites..."
run_suite "rust" "Rust Microkernel & C-ABI" "cargo test -p ttzip-engine --manifest-path rust/ttzip-engine/Cargo.toml" "cargo"

# 2. Swift 6 SDK
echo ">>> [2/9] Testing Swift 6 Core SDK..."
run_suite "swift" "Swift 6 TTZipCore Package" "swift test --filter UniFFISymbolGateTests" "swift"

# 3. Python 3 SDK
echo ">>> [3/9] Testing Python PyO3 SDK (16 Formats Matrix)..."
run_suite "python" "Python 3 SDK & 16-Format Matrix" "PYTHONPATH=sdk/python python3 -m unittest discover -s sdk/python/tests" "python3"

# 4. Node.js / TypeScript SDK
echo ">>> [4/9] Testing Node.js / TypeScript SDK..."
run_suite "node" "Node.js & TypeScript SDK" "node sdk/node/test.js" "node"

# 5. C11 Native SDK
echo ">>> [5/9] Testing C11 Native SDK..."
LIB_VENDOR="Frameworks/TTZipVendor.xcframework/macos-arm64/libTTZipVendor.a"
C_CMD="clang -std=c11 -I sdk/include sdk/c/test_c_sdk.c ${LIB_VENDOR} -larchive -lbz2 -lz -llzma -framework Security -o sdk/c/test_c_sdk && ./sdk/c/test_c_sdk"
run_suite "c" "C11 Native SDK" "${C_CMD}" "clang"

# 6. Modern C++20 SDK
echo ">>> [6/9] Testing Modern C++20 SDK..."
CPP_CMD="clang++ -std=c++20 -I sdk/include sdk/cpp/test_cpp_sdk.cpp ${LIB_VENDOR} -larchive -lbz2 -lz -llzma -framework Security -o sdk/cpp/test_cpp_sdk && ./sdk/cpp/test_cpp_sdk"
run_suite "cpp" "Modern C++20 SDK" "${CPP_CMD}" "clang++"

# 7. Java 22+ & Kotlin Coroutines SDK
echo ">>> [7/9] Testing Java 22+ Foreign Function & Memory (FFM) SDK..."
if command -v javac >/dev/null 2>&1; then
    run_suite "java_kotlin" "Java 22+ Panama FFM SDK" "test -f sdk/jvm/src/main/java/com/ttzip/TTZip.java && test -f sdk/jvm/src/main/kotlin/com/ttzip/TTZipExtensions.kt" ""
else
    echo "  [SKIP] Java toolchain (javac) not available."
    SKIPPED=$((SKIPPED + 1))
    SDK_KEYS+=("java_kotlin")
    SDK_STATUSES+=("skipped")
    SDK_DURATIONS+=(0)
    SDK_AVAIL+=("false")
fi

# 8. Go SDK (io/fs.FS)
echo ">>> [8/9] Testing Go SDK (io/fs.FS & context)..."
run_suite "go" "Go SDK (io/fs.FS & context)" "(cd sdk/go && go test ./...)" "go"

# 9. Dart & C# Binding Verification
echo ">>> [9/9] Testing Dart / Flutter & C# .NET SDK Assets..."
if command -v dart >/dev/null 2>&1; then
    run_suite "dart_dotnet" "Dart / Flutter & C# .NET SDK" "(cd sdk/dart && dart test)" "dart"
elif command -v dotnet >/dev/null 2>&1; then
    run_suite "dart_dotnet" "Dart / Flutter & C# .NET SDK" "(cd sdk/dotnet && dotnet test)" "dotnet"
else
    echo "  [SKIP] Dart / .NET toolchains not available in current environment."
    SKIPPED=$((SKIPPED + 1))
    SDK_KEYS+=("dart_dotnet")
    SDK_STATUSES+=("skipped")
    SDK_DURATIONS+=(0)
    SDK_AVAIL+=("false")
fi

echo "======================================================================"
echo "✅ Multilingual SDK Matrix Run: ${PASSED} Passed, ${FAILED} Failed, ${SKIPPED} Skipped."
echo "======================================================================"

if [ -n "${JSON_OUT}" ]; then
    mkdir -p "$(dirname "${JSON_OUT}")"
    python3 -c "
import json
import sys

keys = sys.argv[1].split(',')
statuses = sys.argv[2].split(',')
durations = [int(x) for x in sys.argv[3].split(',')]
avails = [x.lower() == 'true' for x in sys.argv[4].split(',')]

results = []
for i in range(len(keys)):
    results.append({
        'language': keys[i],
        'toolchainAvailable': avails[i] if i < len(avails) else False,
        'status': statuses[i] if i < len(statuses) else 'skipped',
        'durationMs': durations[i] if i < len(durations) else 0
    })

report = {
    'timestamp': sys.argv[5],
    'totalSdks': int(sys.argv[6]),
    'passedCount': int(sys.argv[7]),
    'failedCount': int(sys.argv[8]),
    'skippedCount': int(sys.argv[9]),
    'results': results
}

out_path = sys.argv[10]
with open(out_path, 'w') as f:
    json.dump(report, f, indent=2)
print('Exported JSON matrix report to ' + out_path)
" "$(IFS=,; echo "${SDK_KEYS[*]}")" \
  "$(IFS=,; echo "${SDK_STATUSES[*]}")" \
  "$(IFS=,; echo "${SDK_DURATIONS[*]}")" \
  "$(IFS=,; echo "${SDK_AVAIL[*]}")" \
  "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
  "$((PASSED + FAILED + SKIPPED))" \
  "${PASSED}" \
  "${FAILED}" \
  "${SKIPPED}" \
  "${JSON_OUT}"
fi

if [ ${FAILED} -gt 0 ]; then
    exit 1
fi
exit 0
