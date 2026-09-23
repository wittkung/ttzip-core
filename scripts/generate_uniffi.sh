#!/usr/bin/env bash
# Copyright 2026 MetastudyLine. All rights reserved.
# Proprietary and confidential.

set -euo pipefail

REPO_ROOT="${BUILD_WORKSPACE_DIRECTORY:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)}"
CORE_DIR="${REPO_ROOT}/products/ttzip/core"
RUST_DIR="${CORE_DIR}/rust"

BINDGEN_BIN="${CORE_DIR}/bazel-bin/products/ttzip/core/rust/ttzip-bindgen/uniffi_bindgen_bin"
if [ ! -f "${BINDGEN_BIN}" ]; then
    BINDGEN_BIN="${REPO_ROOT}/bazel-bin/products/ttzip/core/rust/ttzip-bindgen/uniffi_bindgen_bin"
fi

DYLIB="${CORE_DIR}/bazel-bin/products/ttzip/core/rust/ttzip-engine/libttzip_engine.dylib"
if [ ! -f "${DYLIB}" ]; then
    DYLIB="${REPO_ROOT}/bazel-bin/products/ttzip/core/rust/ttzip-engine/libttzip_engine.dylib"
fi

if [ ! -f "${BINDGEN_BIN}" ] || [ ! -f "${DYLIB}" ]; then
    echo "--> Building prerequisites via Bazel..."
    (cd "${REPO_ROOT}" && bazel build //products/ttzip/core/rust/ttzip-bindgen:uniffi_bindgen_bin //products/ttzip/core/rust/ttzip-engine:ttzip_engine_dylib)
fi

TMP_DIR="$(mktemp -d /tmp/ttzip_uniffi_gen.XXXXXX)"
trap 'rm -rf "${TMP_DIR}"' EXIT

echo "--> Generating Mozilla UniFFI bindings from cdylib..."
(
    cd "${RUST_DIR}"
    "${BINDGEN_BIN}" generate \
        --library "${DYLIB}" \
        --language swift \
        --out-dir "${TMP_DIR}" \
        --no-format \
        --metadata-no-deps
)

# Postprocess for Swift 6 Concurrency and Memory Safety
if [ -f "${TMP_DIR}/ttzip_engine.swift" ]; then
    python3 "${CORE_DIR}/scripts/postprocess_uniffi_swift.py" "${TMP_DIR}/ttzip_engine.swift"
    mkdir -p "${CORE_DIR}/Sources/TTZipCore/Generated"
    cp -f "${TMP_DIR}/ttzip_engine.swift" "${CORE_DIR}/Sources/TTZipCore/Generated/ttzip_engine.swift"
fi

if [ -f "${TMP_DIR}/ttzip_engineFFI.h" ]; then
    mkdir -p "${CORE_DIR}/Sources/CTTZipBridge/include"
    cp -f "${TMP_DIR}/ttzip_engineFFI.h" "${CORE_DIR}/Sources/CTTZipBridge/include/ttzip_engineFFI.h"
    cp -f "${TMP_DIR}/ttzip_engineFFI.h" "${CORE_DIR}/Sources/TTZipCore/Generated/ttzip_engineFFI.h"
fi

if [ -f "${TMP_DIR}/ttzip_engineFFI.modulemap" ]; then
    cp -f "${TMP_DIR}/ttzip_engineFFI.modulemap" "${CORE_DIR}/Sources/TTZipCore/Generated/ttzip_engineFFI.modulemap"
fi

echo "✅ UniFFI Swift and C headers generated and synchronized successfully."
