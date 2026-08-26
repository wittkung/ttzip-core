#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

set -euo pipefail

if [[ $# -eq 0 || "$1" == "--help" ]]; then
    echo "Usage: $0 <tasks.md_path>"
    echo "Validates tasks.md format and detects parallel task conflicts."
    exit 0
fi

FILE="$1"
if [[ ! -f "$FILE" ]]; then
    echo "Error: File '$FILE' not found." >&2
    exit 1
fi

VIOLATIONS=0
declare -A TASK_IDS
PHASE=0
declare -A PHASE_P_PATHS

line_num=0
while IFS= read -r line || [ -n "$line" ]; do
    line_num=$((line_num + 1))

    # Check Phase headers
    if [[ "$line" =~ ^#+[[:space:]]+Phase[[:space:]]+([0-9]+) ]]; then
        curr_phase="${BASH_REMATCH[1]}"
        if [[ $curr_phase -ne $((PHASE + 1)) ]]; then
            echo "$FILE:$line_num: Phase header out of sequence (expected $((PHASE + 1)), got $curr_phase)"
            VIOLATIONS=$((VIOLATIONS + 1))
        fi
        PHASE=$curr_phase
        unset PHASE_P_PATHS
        declare -A PHASE_P_PATHS
    fi

    # Check task lines
    if [[ "$line" =~ ^-[[:space:]]+\[ ]]; then
        # Rule 1: format
        if ! [[ "$line" =~ ^-[[:space:]]+\[[[:space:]xX]\][[:space:]]+T[0-9]{3,} ]]; then
            echo "$FILE:$line_num: Task line does not match pattern '- [ ] TXXX' or '- [x] TXXX'"
            VIOLATIONS=$((VIOLATIONS + 1))
        fi

        # Rule 2: unique IDs
        if [[ "$line" =~ T([0-9]{3,}) ]]; then
            tid="${BASH_REMATCH[1]}"
            if [[ -n "${TASK_IDS[$tid]:-}" ]]; then
                echo "$FILE:$line_num: Duplicate Task ID T$tid"
                VIOLATIONS=$((VIOLATIONS + 1))
            else
                TASK_IDS[$tid]=1
            fi
        fi

        # Rule 3: [P] tasks in same Phase conflict check
        if [[ "$line" == *"[P]"* ]]; then
            if [[ "$line" =~ in[[:space:]]+\`?([^[:space:]\`]+)\`? ]]; then
                path="${BASH_REMATCH[1]}"
                if [[ -n "${PHASE_P_PATHS[$path]:-}" ]]; then
                    echo "$FILE:$line_num: Parallel task conflict on path '$path' in Phase $PHASE"
                    VIOLATIONS=$((VIOLATIONS + 1))
                else
                    PHASE_P_PATHS[$path]=1
                fi
            fi
        fi
    fi
done < "$FILE"

if [[ $VIOLATIONS -gt 0 ]]; then
    exit 1
fi
echo "All tasks in $FILE verified successfully."
exit 0
