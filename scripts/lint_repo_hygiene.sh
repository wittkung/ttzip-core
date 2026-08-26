#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip Deterministic Repository Hygiene & Artifact Linter Gate

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${WORKSPACE_ROOT}"

# Colors
C_RESET="\033[0m"
C_BOLD="\033[1m"
C_RED="\033[1;31m"
C_GREEN="\033[1;32m"
C_YELLOW="\033[1;33m"
C_CYAN="\033[1;36m"

VIOLATIONS=0

echo -e "${C_CYAN}${C_BOLD}======================================================================${C_RESET}"
echo -e "${C_CYAN}${C_BOLD}          TTZip Deterministic Repository Hygiene Linter Gate          ${C_RESET}"
echo -e "${C_CYAN}${C_BOLD}======================================================================${C_RESET}"

report_violation() {
    local category="$1"
    local path="$2"
    local message="$3"
    echo -e "  ${C_RED}❌ [${category}]${C_RESET} ${path}"
    echo -e "     ${C_YELLOW}Reason: ${message}${C_RESET}"
    VIOLATIONS=$((VIOLATIONS + 1))
}

echo "--> [1/5] Checking for rogue root-level web artifacts in core/..."
for html in core/*.html core/CNAME core/_config.yml core/.nojekyll; do
    if [ -f "${html}" ]; then
        report_violation "ROGUE_WEB_ARTIFACT" "${html}" "Web site files belong in apple/site/ or docs/, not core/ root"
    fi
done

echo "--> [2/5] Checking for unreferenced GUI targets and tests inside core/..."
for orphan in core/Sources/TTZipApp core/Sources/TTZipFinderSync core/Sources/TTZipQuickLook core/Tests/TTZipAppTests; do
    if [ -d "${orphan}" ]; then
        report_violation "ORPHANED_CORE_SOURCE" "${orphan}" "App targets and tests migrated to apple/; remove dead copies from core/"
    fi
done

echo "--> [3/5] Checking for unoptimized compiler flags (.unsafeFlags)..."
if grep -q "no-whole-module-optimization" core/Package.swift 2>/dev/null; then
    report_violation "UNOPTIMIZED_SWIFT_FLAG" "core/Package.swift" "Contains -no-whole-module-optimization; remove to enable Release WMO"
fi

echo "--> [4/5] Checking for forbidden macOS clutter (.DS_Store, ._ files)..."
while IFS= read -r clutter; do
    if [ -n "${clutter}" ]; then
        report_violation "MACOS_METADATA_CLUTTER" "${clutter}" "Unwanted macOS metadata file found in repository"
    fi
done < <(find . -not -path "*/.*" -name ".DS_Store" -o -name "._*" 2>/dev/null || true)

echo "--> [5/5] Checking for unignored build artifacts in repository roots..."
for build_dir in dist/staging core/dist/staging; do
    if [ -d "${build_dir}" ]; then
        report_violation "DIRTY_STAGING_DIR" "${build_dir}" "Uncleaned staging directory found"
    fi
done

echo ""
echo -e "${C_CYAN}======================================================================${C_RESET}"
if [ ${VIOLATIONS} -gt 0 ]; then
    echo -e "${C_RED}${C_BOLD}❌ Repository Hygiene Gate Failed: ${VIOLATIONS} violation(s) detected.${C_RESET}"
    echo -e "${C_RED}Please clean up orphaned files, duplicate web assets, or unoptimized flags.${C_RESET}"
    exit 1
else
    echo -e "${C_GREEN}${C_BOLD}✅ Repository Hygiene Gate Passed: 0 violations detected.${C_RESET}"
    exit 0
fi
