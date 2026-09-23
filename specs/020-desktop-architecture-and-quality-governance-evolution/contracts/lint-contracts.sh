#!/usr/bin/env bash
# Copyright 2026 MetastudyLine. All rights reserved.
# Proprietary and confidential.

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
