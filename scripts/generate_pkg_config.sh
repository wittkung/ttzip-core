#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# Generates ttzip.pc pkg-config file for C/C++ build systems.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

PREFIX="${1:-/usr/local}"
VERSION="${2:-1.0.0}"
OUTPUT_FILE="${3:-${REPO_ROOT}/ttzip.pc}"

sed -e "s|@PREFIX@|${PREFIX}|g" \
    -e "s|@VERSION@|${VERSION}|g" \
    "${REPO_ROOT}/ttzip.pc.in" > "${OUTPUT_FILE}"

echo "✅ Generated pkg-config file at ${OUTPUT_FILE} (prefix=${PREFIX}, version=${VERSION})"
