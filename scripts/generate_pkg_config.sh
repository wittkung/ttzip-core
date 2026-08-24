#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# Generates ttzip.pc pkg-config file for C/C++ build systems with Libs.private.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

PREFIX="${1:-/usr/local}"
VERSION="${2:-1.0.0}"
OUTPUT_FILE="${3:-${REPO_ROOT}/ttzip.pc}"
LIBDIR="${4:-}"
INCLUDEDIR="${5:-}"

if [ -z "${LIBDIR}" ]; then
    LIBDIR='${exec_prefix}/lib'
fi

if [ -z "${INCLUDEDIR}" ]; then
    INCLUDEDIR='${prefix}/include'
fi

# Determine Libs.private based on OS
OS_NAME="$(uname -s)"
if [ "${OS_NAME}" = "Darwin" ]; then
    LIBS_PRIVATE="-larchive -lbz2 -lz -llzma -lpthread -framework Security -framework CoreFoundation"
elif [ "${OS_NAME}" = "Linux" ]; then
    LIBS_PRIVATE="-larchive -lbz2 -lz -llzma -lpthread -ldl -lm"
else
    LIBS_PRIVATE="-larchive -lbz2 -lz -llzma -lpthread"
fi

sed -e "s|@PREFIX@|${PREFIX}|g" \
    -e "s|@VERSION@|${VERSION}|g" \
    -e "s|@LIBDIR@|${LIBDIR}|g" \
    -e "s|@INCLUDEDIR@|${INCLUDEDIR}|g" \
    -e "s|@LIBS_PRIVATE@|${LIBS_PRIVATE}|g" \
    "${REPO_ROOT}/ttzip.pc.in" > "${OUTPUT_FILE}"

echo "✅ Generated pkg-config file at ${OUTPUT_FILE} (prefix=${PREFIX}, version=${VERSION})"
