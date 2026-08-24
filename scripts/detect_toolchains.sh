#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# Dynamic Toolchain Runtime Detector for TTZip Multi-Language SDK Test Matrix.
# Probes installed compilers, build tools, SDK runtimes, and system topology.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# ANSI Color codes
BOLD="\033[1m"
GREEN="\033[0;32m"
RED="\033[0;31m"
YELLOW="\033[0;33m"
CYAN="\033[0;36m"
GRAY="\033[0;90m"
RESET="\033[0m"

# Default configuration
OUTPUT_FILE=""
MODE="json" # "json", "human", "check", "sdk_check"
CHECK_TOOL=""
CHECK_SDK=""

# Usage help
show_help() {
    cat << 'EOF'
TTZip Toolchain Runtime Detector

Usage:
  detect_toolchains.sh [OPTIONS]

Options:
  --json                Output toolchain detection report as JSON (default)
  --human, -s           Output human-readable formatted summary table
  --output=<path>, -o   Write JSON output directly to file
  --check=<tool>        Check if a specific tool exists (exits 0 if yes, 1 if no)
  --sdk=<sdk_name>      Check if a specific SDK ecosystem is ready (exits 0 or 1)
  --help, -h            Show this help dialog

Supported SDKs:
  rust, swift, python, go, java, c, cpp, dart, dotnet

Examples:
  ./detect_toolchains.sh --json
  ./detect_toolchains.sh --human
  ./detect_toolchains.sh --check=rustc
  ./detect_toolchains.sh --sdk=swift
  ./detect_toolchains.sh -o /tmp/toolchains.json
EOF
}

# Parse CLI arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --json)
            MODE="json"
            shift
            ;;
        --human|-s|--summary)
            MODE="human"
            shift
            ;;
        --output=*|-o=*)
            OUTPUT_FILE="${1#*=}"
            shift
            ;;
        -o|--output)
            OUTPUT_FILE="$2"
            shift 2
            ;;
        --check=*)
            MODE="check"
            CHECK_TOOL="${1#*=}"
            shift
            ;;
        --sdk=*)
            MODE="sdk_check"
            CHECK_SDK="${1#*=}"
            shift
            ;;
        --help|-h)
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

# System Architecture & OS Detection
detect_os() {
    local uname_s
    uname_s="$(uname -s 2>/dev/null || echo "Unknown")"
    case "${uname_s}" in
        Darwin*)  echo "darwin" ;;
        Linux*)   echo "linux" ;;
        CYGWIN*|MINGW*|MSYS*) echo "windows" ;;
        *)        echo "unknown" ;;
    esac
}

detect_arch() {
    local uname_m
    uname_m="$(uname -m 2>/dev/null || echo "unknown")"
    case "${uname_m}" in
        arm64|aarch64) echo "arm64" ;;
        x86_64|amd64)  echo "x86_64" ;;
        *)             echo "${uname_m}" ;;
    esac
}

detect_cpu_cores() {
    if command -v sysctl >/dev/null 2>&1; then
        local cores
        cores="$(sysctl -n hw.ncpu 2>/dev/null || true)"
        if [[ -n "${cores}" ]]; then
            echo "${cores}"
            return
        fi
    fi
    if command -v nproc >/dev/null 2>&1; then
        local cores
        cores="$(nproc 2>/dev/null || true)"
        if [[ -n "${cores}" ]]; then
            echo "${cores}"
            return
        fi
    fi
    if command -v python3 >/dev/null 2>&1; then
        local cores
        cores="$(python3 -c "import os; print(os.cpu_count() or 1)" 2>/dev/null || true)"
        if [[ -n "${cores}" ]]; then
            echo "${cores}"
            return
        fi
    fi
    echo "1"
}

OS_NAME="$(detect_os)"
ARCH_NAME="$(detect_arch)"
CPU_CORES="$(detect_cpu_cores)"
PLATFORM_DESC="$(uname -srm 2>/dev/null || echo "Unknown")"
TIMESTAMP="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

# Tool Helper Functions
get_tool_info() {
    local tool_name="$1"
    local version_cmd="$2"
    local version_extract="$3"

    local tool_path=""
    local tool_avail="false"
    local tool_ver=""

    if command -v "${tool_name}" >/dev/null 2>&1; then
        tool_path="$(command -v "${tool_name}" 2>/dev/null || true)"
        if [[ -n "${tool_path}" ]]; then
            tool_avail="true"
            if [[ -n "${version_cmd}" ]]; then
                local raw_ver
                raw_ver="$(eval "${version_cmd}" 2>&1 || true)"
                if [[ -n "${version_extract}" ]]; then
                    tool_ver="$(echo "${raw_ver}" | eval "${version_extract}" 2>/dev/null || true)"
                else
                    tool_ver="$(echo "${raw_ver}" | head -n 1 | sed 's/^[ \t]*//;s/[ \t]*$//')"
                fi
            fi
        fi
    fi

    # Escape quotes and backslashes for JSON safety
    tool_ver="$(echo "${tool_ver}" | tr '\n' ' ' | sed 's/\\/\\\\/g; s/"/\\"/g; s/^[ \t]*//; s/[ \t]*$//')"
    tool_path="$(echo "${tool_path}" | sed 's/\\/\\\\/g; s/"/\\"/g')"

    echo "${tool_avail}|${tool_path}|${tool_ver}"
}

# Detect individual tools
# Format: AVAIL|PATH|VERSION
IFS='|' read -r RUSTC_AVAIL RUSTC_PATH RUSTC_VER <<< "$(get_tool_info "rustc" "rustc --version" "awk '{print \$2}'")"
IFS='|' read -r CARGO_AVAIL CARGO_PATH CARGO_VER <<< "$(get_tool_info "cargo" "cargo --version" "awk '{print \$2}'")"
IFS='|' read -r SWIFT_AVAIL SWIFT_PATH SWIFT_VER <<< "$(get_tool_info "swift" "swift --version" "grep -oE 'Swift version [0-9]+\.[0-9]+(\.[0-9]+)?' | sed 's/Swift version //'")"
IFS='|' read -r SWIFTC_AVAIL SWIFTC_PATH SWIFTC_VER <<< "$(get_tool_info "swiftc" "swiftc --version" "grep -oE 'Swift version [0-9]+\.[0-9]+(\.[0-9]+)?' | sed 's/Swift version //'")"
IFS='|' read -r PYTHON3_AVAIL PYTHON3_PATH PYTHON3_VER <<< "$(get_tool_info "python3" "python3 --version" "awk '{print \$2}'")"
IFS='|' read -r PYTEST_AVAIL PYTEST_PATH PYTEST_VER <<< "$(get_tool_info "pytest" "pytest --version" "awk '{print \$2}'")"
IFS='|' read -r GO_AVAIL GO_PATH GO_VER <<< "$(get_tool_info "go" "go version" "grep -oE 'go[0-9]+\.[0-9]+(\.[0-9]+)?' | sed 's/^go//'")"
IFS='|' read -r JAVA_AVAIL JAVA_PATH JAVA_VER <<< "$(get_tool_info "java" "java -version" "head -n 1 | grep -oE '[0-9]+(\.[0-9]+)*' | head -n 1")"
IFS='|' read -r JAVAC_AVAIL JAVAC_PATH JAVAC_VER <<< "$(get_tool_info "javac" "javac -version" "awk '{print \$2}'")"
IFS='|' read -r MVN_AVAIL MVN_PATH MVN_VER <<< "$(get_tool_info "mvn" "mvn -v" "head -n 1 | awk '{print \$3}'")"
IFS='|' read -r GRADLE_AVAIL GRADLE_PATH GRADLE_VER <<< "$(get_tool_info "gradle" "gradle -v" "grep -E '^Gradle [0-9]' | awk '{print \$2}'")"
IFS='|' read -r DART_AVAIL DART_PATH DART_VER <<< "$(get_tool_info "dart" "dart --version" "grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -n 1")"
IFS='|' read -r DOTNET_AVAIL DOTNET_PATH DOTNET_VER <<< "$(get_tool_info "dotnet" "dotnet --version" "")"
IFS='|' read -r CLANG_AVAIL CLANG_PATH CLANG_VER <<< "$(get_tool_info "clang" "clang --version" "grep -oE 'version [0-9]+\.[0-9]+\.[0-9]+' | sed 's/version //'")"
IFS='|' read -r CLANGPP_AVAIL CLANGPP_PATH CLANGPP_VER <<< "$(get_tool_info "clang++" "clang++ --version" "grep -oE 'version [0-9]+\.[0-9]+\.[0-9]+' | sed 's/version //'")"
IFS='|' read -r CMAKE_AVAIL CMAKE_PATH CMAKE_VER <<< "$(get_tool_info "cmake" "cmake --version" "head -n 1 | awk '{print \$3}'")"
IFS='|' read -r NINJA_AVAIL NINJA_PATH NINJA_VER <<< "$(get_tool_info "ninja" "ninja --version" "")"

# Determine SDK Ecosystem Readiness
is_sdk_ready() {
    local sdk="$1"
    case "${sdk}" in
        rust)
            [[ "${RUSTC_AVAIL}" == "true" && "${CARGO_AVAIL}" == "true" ]]
            ;;
        swift)
            [[ "${SWIFT_AVAIL}" == "true" ]]
            ;;
        python)
            [[ "${PYTHON3_AVAIL}" == "true" ]]
            ;;
        go)
            [[ "${GO_AVAIL}" == "true" ]]
            ;;
        java|jvm)
            [[ "${JAVA_AVAIL}" == "true" || "${JAVAC_AVAIL}" == "true" ]]
            ;;
        c)
            [[ "${CLANG_AVAIL}" == "true" ]]
            ;;
        cpp)
            [[ "${CLANGPP_AVAIL}" == "true" ]]
            ;;
        dart)
            [[ "${DART_AVAIL}" == "true" ]]
            ;;
        dotnet)
            [[ "${DOTNET_AVAIL}" == "true" ]]
            ;;
        *)
            return 1
            ;;
    esac
}

get_sdk_reason() {
    local sdk="$1"
    if is_sdk_ready "${sdk}"; then
        case "${sdk}" in
            rust) echo "Rust compiler (rustc ${RUSTC_VER}) and cargo ready" ;;
            swift) echo "Swift toolchain (${SWIFT_VER}) ready" ;;
            python) echo "Python 3 (${PYTHON3_VER}) ready" ;;
            go) echo "Go toolchain (${GO_VER}) ready" ;;
            java|jvm) echo "Java runtime (${JAVA_VER:-${JAVAC_VER}}) ready" ;;
            c) echo "Clang C11 compiler (${CLANG_VER}) ready" ;;
            cpp) echo "Clang++ C++20 compiler (${CLANGPP_VER}) ready" ;;
            dart) echo "Dart SDK (${DART_VER}) ready" ;;
            dotnet) echo ".NET SDK (${DOTNET_VER}) ready" ;;
        esac
    else
        case "${sdk}" in
            rust) echo "Missing rustc or cargo (install via https://rustup.rs/)" ;;
            swift) echo "Missing swift toolchain (install Xcode or swift.org toolchain)" ;;
            python) echo "Missing python3 (install via brew install python3 or apt)" ;;
            go) echo "Missing go compiler (install via brew install go or golang.org)" ;;
            java|jvm) echo "Missing java/javac (install OpenJDK 21+ via brew install openjdk@21)" ;;
            c) echo "Missing clang (install Xcode Command Line Tools or clang)" ;;
            cpp) echo "Missing clang++ (install Xcode Command Line Tools or clang++)" ;;
            dart) echo "Missing dart (install via brew tap dart-lang/dart && brew install dart)" ;;
            dotnet) echo "Missing dotnet SDK (install .NET 8 via https://dot.net)" ;;
            *) echo "Toolchain not available" ;;
        esac
    fi
}

# Handle --check=<tool> mode
if [[ "${MODE}" == "check" ]]; then
    tool_check_avail="$(get_tool_info "${CHECK_TOOL}" "" "" | cut -d'|' -f1)"
    if [[ "${tool_check_avail}" == "true" ]]; then
        exit 0
    else
        exit 1
    fi
fi

# Handle --sdk=<sdk_name> mode
if [[ "${MODE}" == "sdk_check" ]]; then
    if is_sdk_ready "${CHECK_SDK}"; then
        exit 0
    else
        exit 1
    fi
fi

# Handle --human mode
if [[ "${MODE}" == "human" ]]; then
    echo -e "${BOLD}${CYAN}======================================================================${RESET}"
    echo -e "${BOLD}${CYAN}⚡️ TTZip Multi-Language Toolchain Runtime Detector${RESET}"
    echo -e "${BOLD}${CYAN}======================================================================${RESET}"
    echo -e "  ${BOLD}Platform:${RESET}    ${PLATFORM_DESC}"
    echo -e "  ${BOLD}OS / Arch:${RESET}   ${OS_NAME} / ${ARCH_NAME}"
    echo -e "  ${BOLD}CPU Cores:${RESET}   ${CPU_CORES}"
    echo -e "  ${BOLD}Timestamp:${RESET}   ${TIMESTAMP}"
    echo -e "${BOLD}${CYAN}----------------------------------------------------------------------${RESET}"
    echo -e "${BOLD}Language SDK Ecosystem Readiness:${RESET}"
    
    sdks=("rust" "swift" "python" "go" "java" "c" "cpp" "dart" "dotnet")
    for s in "${sdks[@]}"; do
        sdk_upper="$(echo "${s}" | tr '[:lower:]' '[:upper:]')"
        printf "  %-10s " "${sdk_upper}:"
        if is_sdk_ready "${s}"; then
            echo -e "${GREEN}[AVAILABLE]${RESET}  $(get_sdk_reason "${s}")"
        else
            echo -e "${YELLOW}[NOT FOUND]${RESET}  ${GRAY}$(get_sdk_reason "${s}")${RESET}"
        fi
    done

    echo -e "${BOLD}${CYAN}----------------------------------------------------------------------${RESET}"
    echo -e "${BOLD}Installed Binary Tools:${RESET}"
    tools=("rustc" "cargo" "swift" "swiftc" "python3" "pytest" "go" "java" "javac" "mvn" "gradle" "dart" "dotnet" "clang" "clang++" "cmake" "ninja")
    for t in "${tools[@]}"; do
        case "${t}" in
            go) vcmd="go version" ;;
            java) vcmd="java -version" ;;
            javac) vcmd="javac -version" ;;
            mvn) vcmd="mvn -v" ;;
            gradle) vcmd="gradle -v" ;;
            *) vcmd="${t} --version" ;;
        esac
        info="$(get_tool_info "${t}" "${vcmd}" "")"
        t_avail="$(echo "${info}" | cut -d'|' -f1)"
        t_path="$(echo "${info}" | cut -d'|' -f2)"
        t_ver="$(echo "${info}" | cut -d'|' -f3)"
        printf "  %-10s " "${t}:"
        if [[ "${t_avail}" == "true" ]]; then
            echo -e "${GREEN}✓${RESET} ${t_path} ${GRAY}(${t_ver:-present})${RESET}"
        else
            echo -e "${RED}✗${RESET} ${GRAY}not found in PATH${RESET}"
        fi
    done
    echo -e "${BOLD}${CYAN}======================================================================${RESET}"
    exit 0
fi

# Build SDK JSON Status Object
build_sdk_json() {
    local sdk="$1"
    local avail="false"
    if is_sdk_ready "${sdk}"; then
        avail="true"
    fi
    local reason
    reason="$(get_sdk_reason "${sdk}" | sed 's/\\/\\\\/g; s/"/\\"/g')"
    cat << JSON_SDK_EOF
    "${sdk}": {
      "available": ${avail},
      "reason": "${reason}"
    }
JSON_SDK_EOF
}

# Build Tool JSON Status Object
build_tool_json() {
    local name="$1"
    local avail="$2"
    local path="$3"
    local ver="$4"
    cat << JSON_TOOL_EOF
    "${name}": {
      "available": ${avail},
      "path": "${path}",
      "version": "${ver}"
    }
JSON_TOOL_EOF
}

# Generate Complete JSON
JSON_DATA=$(cat << JSON_EOF
{
  "timestamp": "${TIMESTAMP}",
  "system": {
    "os": "${OS_NAME}",
    "arch": "${ARCH_NAME}",
    "cpuCores": ${CPU_CORES},
    "platform": "${PLATFORM_DESC}"
  },
  "environment": {
    "os": "${OS_NAME}",
    "cpuCores": ${CPU_CORES},
    "rustcVersion": "${RUSTC_VER}",
    "swiftVersion": "${SWIFT_VER}",
    "pythonVersion": "${PYTHON3_VER}",
    "goVersion": "${GO_VER}",
    "javaVersion": "${JAVA_VER:-${JAVAC_VER}}"
  },
  "sdks": {
$(build_sdk_json "rust"),
$(build_sdk_json "swift"),
$(build_sdk_json "python"),
$(build_sdk_json "go"),
$(build_sdk_json "java"),
$(build_sdk_json "c"),
$(build_sdk_json "cpp"),
$(build_sdk_json "dart"),
$(build_sdk_json "dotnet")
  },
  "tools": {
$(build_tool_json "rustc" "${RUSTC_AVAIL}" "${RUSTC_PATH}" "${RUSTC_VER}"),
$(build_tool_json "cargo" "${CARGO_AVAIL}" "${CARGO_PATH}" "${CARGO_VER}"),
$(build_tool_json "swift" "${SWIFT_AVAIL}" "${SWIFT_PATH}" "${SWIFT_VER}"),
$(build_tool_json "swiftc" "${SWIFTC_AVAIL}" "${SWIFTC_PATH}" "${SWIFTC_VER}"),
$(build_tool_json "python3" "${PYTHON3_AVAIL}" "${PYTHON3_PATH}" "${PYTHON3_VER}"),
$(build_tool_json "pytest" "${PYTEST_AVAIL}" "${PYTEST_PATH}" "${PYTEST_VER}"),
$(build_tool_json "go" "${GO_AVAIL}" "${GO_PATH}" "${GO_VER}"),
$(build_tool_json "java" "${JAVA_AVAIL}" "${JAVA_PATH}" "${JAVA_VER}"),
$(build_tool_json "javac" "${JAVAC_AVAIL}" "${JAVAC_PATH}" "${JAVAC_VER}"),
$(build_tool_json "mvn" "${MVN_AVAIL}" "${MVN_PATH}" "${MVN_VER}"),
$(build_tool_json "gradle" "${GRADLE_AVAIL}" "${GRADLE_PATH}" "${GRADLE_VER}"),
$(build_tool_json "dart" "${DART_AVAIL}" "${DART_PATH}" "${DART_VER}"),
$(build_tool_json "dotnet" "${DOTNET_AVAIL}" "${DOTNET_PATH}" "${DOTNET_VER}"),
$(build_tool_json "clang" "${CLANG_AVAIL}" "${CLANG_PATH}" "${CLANG_VER}"),
$(build_tool_json "clangpp" "${CLANGPP_AVAIL}" "${CLANGPP_PATH}" "${CLANGPP_VER}"),
$(build_tool_json "cmake" "${CMAKE_AVAIL}" "${CMAKE_PATH}" "${CMAKE_VER}"),
$(build_tool_json "ninja" "${NINJA_AVAIL}" "${NINJA_PATH}" "${NINJA_VER}")
  }
}
JSON_EOF
)

# Output or save JSON
if [[ -n "${OUTPUT_FILE}" ]]; then
    mkdir -p "$(dirname "${OUTPUT_FILE}")"
    echo "${JSON_DATA}" > "${OUTPUT_FILE}"
fi

echo "${JSON_DATA}"
exit 0
