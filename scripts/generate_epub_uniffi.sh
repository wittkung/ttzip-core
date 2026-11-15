#!/usr/bin/env bash
# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause

set -euo pipefail

if [[ $# != 2 ]]; then
    echo "Usage: $0 <uniffi-bindgen> <libttzip_epub.dylib>" >&2
    exit 2
fi
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CORE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

(cd "${CORE_DIR}/rust" && "$1" generate --library "$2" --language swift --out-dir "${TMP_DIR}" --no-format --metadata-no-deps)
PYTHONPATH="${SCRIPT_DIR}" python3 - "${TMP_DIR}" <<'PYTHON'
from pathlib import Path
import sys
from postprocess_uniffi_swift import postprocess_epub

for path in Path(sys.argv[1]).iterdir():
    content = path.read_text()
    if path.suffix == ".swift":
        content = postprocess_epub(content)
    lines = (line.rstrip() for line in content.splitlines())
    path.write_text("\n".join(lines).rstrip() + "\n")
PYTHON
cp "${TMP_DIR}/ttzip_epub.swift" "${CORE_DIR}/Sources/TTZipEpub/Generated/"
cp "${TMP_DIR}/ttzip_epubFFI.h" "${CORE_DIR}/Sources/CTTZipEpubBridge/include/"
cp "${TMP_DIR}/ttzip_epubFFI.modulemap" "${CORE_DIR}/Sources/CTTZipEpubBridge/include/module.modulemap"
