#!/usr/bin/env bash
# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause

set -euo pipefail

REPO_ROOT="${BUILD_WORKSPACE_DIRECTORY:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)}"
CORE_DIR="${REPO_ROOT}/products/ttzip/core"
RUST_DIR="${CORE_DIR}/rust"

echo "--> Building ttzip_engine_dylib via Bazel..."
BAZEL_STARTUP_ARGS=()
BAZEL_BUILD_ARGS=()
if [[ -n "${TTZIP_ISOLATED_OUTPUT_BASE:-}" ]]; then
    BAZEL_STARTUP_ARGS+=("--output_base=${TTZIP_ISOLATED_OUTPUT_BASE}")
    BAZEL_BUILD_ARGS+=("--experimental_convenience_symlinks=ignore" "--noremote_accept_cached")
fi
if ! nc -z -G 1 127.0.0.1 50051 2>/dev/null; then
    BAZEL_BUILD_ARGS+=("--remote_cache=")
fi

(cd "${REPO_ROOT}" && bazel "${BAZEL_STARTUP_ARGS[@]}" build "${BAZEL_BUILD_ARGS[@]}" \
    //products/ttzip/core/rust/ttzip-engine:ttzip_engine_dylib)

EXEC_ROOT="$(cd "${REPO_ROOT}" && bazel "${BAZEL_STARTUP_ARGS[@]}" info execution_root 2>/dev/null | tail -n 1)"
REL_DYLIB="$(cd "${REPO_ROOT}" && bazel "${BAZEL_STARTUP_ARGS[@]}" cquery "${BAZEL_BUILD_ARGS[@]}" --output=files //products/ttzip/core/rust/ttzip-engine:ttzip_engine_dylib 2>/dev/null | grep -E '\.dylib$' | head -n 1)"

DYLIB="${EXEC_ROOT}/${REL_DYLIB}"

if [[ ! -f "${DYLIB}" ]]; then
    echo "ERROR: Missing built cdylib in execution_root (${DYLIB})" >&2
    exit 1
fi

TMP_DIR="$(mktemp -d /tmp/ttzip_uniffi_gen.XXXXXX)"
trap 'rm -rf "${TMP_DIR}"' EXIT

echo "--> Generating Mozilla UniFFI bindings from cdylib (${DYLIB})..."
(cd "${RUST_DIR}" && cargo run --package ttzip-bindgen --bin uniffi-bindgen -- generate \
    --library "${DYLIB}" \
    --language swift \
    --out-dir "${TMP_DIR}" \
    --no-format \
    --metadata-no-deps)

# Postprocess for Swift 6 Concurrency and Memory Safety
if [ -f "${TMP_DIR}/ttzip_engine.swift" ]; then
    python3 "${CORE_DIR}/scripts/postprocess_uniffi_swift.py" "${TMP_DIR}/ttzip_engine.swift"
    mkdir -p "${CORE_DIR}/Sources/TTZipCore/Generated"
    cp -f "${TMP_DIR}/ttzip_engine.swift" "${CORE_DIR}/Sources/TTZipCore/Generated/ttzip_engine.swift"
fi

if [ -f "${TMP_DIR}/ttzip_engineFFI.h" ]; then
    mkdir -p "${CORE_DIR}/Sources/CTTZipBridge/include"
    mkdir -p "${CORE_DIR}/Sources/TTZipEngineFFI/include"
    cp -f "${TMP_DIR}/ttzip_engineFFI.h" "${CORE_DIR}/Sources/CTTZipBridge/include/ttzip_engineFFI.h"
    cp -f "${TMP_DIR}/ttzip_engineFFI.h" "${CORE_DIR}/Sources/TTZipCore/Generated/ttzip_engineFFI.h"
    cp -f "${TMP_DIR}/ttzip_engineFFI.h" "${CORE_DIR}/Sources/TTZipEngineFFI/include/ttzip_engineFFI.h"
fi

if [ -f "${TMP_DIR}/ttzip_engineFFI.modulemap" ]; then
    cp -f "${TMP_DIR}/ttzip_engineFFI.modulemap" "${CORE_DIR}/Sources/TTZipCore/Generated/ttzip_engineFFI.modulemap"
fi

echo "✅ UniFFI Swift and C headers generated and synchronized successfully."
