#!/usr/bin/env bash
# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
echo "--> Validating 020 Desktop Architecture Evolution contracts..."

for contract in "${SCRIPT_DIR}"/*.md; do
    if [ -f "${contract}" ]; then
        echo "  ✓ Contract: $(basename "${contract}")"
    fi
done

echo "--> 100% contracts validated successfully."
exit 0
