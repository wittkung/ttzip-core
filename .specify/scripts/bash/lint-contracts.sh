#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

set -euo pipefail

if [[ $# -eq 0 || "$1" == "--help" ]]; then
    echo "Usage: $0 <contracts_directory>"
    echo "Validates all JSON files in a contracts directory against engineering standards."
    exit 0
fi

DIR="$1"
if [[ ! -d "$DIR" ]]; then
    echo "Error: Directory '$DIR' not found." >&2
    exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
    echo "Error: jq is required but not installed." >&2
    exit 1
fi

VIOLATIONS=0
FILES_FOUND=0

while IFS= read -r -d '' file; do
    FILES_FOUND=$((FILES_FOUND + 1))
    
    if ! jq empty "$file" >/dev/null 2>&1; then
        echo "$file:1: Invalid JSON"
        VIOLATIONS=$((VIOLATIONS + 1))
        continue
    fi

    # Rule 1: $schema
    schema=$(jq -r '."$schema" // empty' "$file")
    if [[ "$schema" != "http://json-schema.org/draft-07/schema#" ]]; then
        echo "$file:1: Missing or incorrect \$schema declaration (Rule 1)"
        VIOLATIONS=$((VIOLATIONS + 1))
    fi

    # Rule 2: Zero Bare Objects (no type: object without properties)
    bare_objects=$(jq '[.. | objects | select(.type == "object" and (has("properties") | not))] | length' "$file")
    if [[ "$bare_objects" -gt 0 ]]; then
        echo "$file:0: Contains \"type\": \"object\" node(s) without sibling \"properties\" key (Rule 2)"
        VIOLATIONS=$((VIOLATIONS + 1))
    fi

    # Rule 3: Object with properties must have additionalProperties
    missing_add_props=$(jq '[.. | objects | select(has("properties") and (has("additionalProperties") | not))] | length' "$file")
    if [[ "$missing_add_props" -gt 0 ]]; then
        echo "$file:0: Contains \"properties\" node(s) lacking \"additionalProperties\" declaration (Rule 3)"
        VIOLATIONS=$((VIOLATIONS + 1))
    fi

done < <(find "$DIR" -name "*.json" -print0)

if [[ $FILES_FOUND -eq 0 ]]; then
    echo "No .json files found in $DIR."
fi

if [[ $VIOLATIONS -gt 0 ]]; then
    exit 1
fi
echo "All $FILES_FOUND contract schemas verified successfully."
exit 0
