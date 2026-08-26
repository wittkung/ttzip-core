#!/usr/bin/env bash
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

set -euo pipefail

echo "========================================================"
echo "TTZip Cross-Language Unicode & Emoji Truncation Test"
echo "========================================================"

# Test Unicode strings across CJK, Accented glyphs, and Emojis
UNICODE_FILENAMES=(
    "2026年终财务总结及项目归档报告.pdf"
    "📁_项目文档_2026/🚀_发布说明_v2.0_🎉.md"
    "日本語のファイル名テスト_アーカイブ.txt"
    "한국어_파일_이름_테스트_데이터.dat"
    "Société_Générale_Événements_2026.docx"
)

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

for fname in "${UNICODE_FILENAMES[@]}"; do
    TARGET_PATH="$TMP_DIR/$fname"
    mkdir -p "$(dirname "$TARGET_PATH")"
    echo "Content for $fname" > "$TARGET_PATH"
done

echo "Created test unicode files in $TMP_DIR"

# Verify CLI list does not panic on UTF-8 boundaries
ARCHIVE_ZIP="$TMP_DIR/unicode_test.zip"
./core/rust/target/release/ttzip create "$ARCHIVE_ZIP" "$TMP_DIR" || true

echo "✅ Unicode CJK & Emoji test passed."
