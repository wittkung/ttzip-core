#!/usr/bin/env python3
# -*- coding: utf-8 -*-
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# Canonical Test Corpus Generator for TTZip Multi-Language SDK Test Matrix.
# Produces 4 standard benchmark & round-trip datasets:
#   1. ASCII text corpus (varying sizes, code, logs, structured text)
#   2. Nested multi-level directory tree (deep nesting, branches, empty dirs)
#   3. Multibyte CJK / Emoji / Diacritic filenames (UTF-8 consistency)
#   4. Sparse 1GB test file (zero-hole large file with deterministic markers)

import argparse
import hashlib
import json
import os
import shutil
import sys
from pathlib import Path
from typing import Dict, List, Optional, Tuple, Any

# Default path relative to repository root
DEFAULT_OUTPUT_DIR = Path(__file__).resolve().parent / "canonical"
SPARSE_FILE_SIZE = 1024 * 1024 * 1024  # 1 GB (1,073,741,824 bytes)


def compute_sha256(file_path: Path) -> str:
    """Compute SHA-256 hash of a file efficiently."""
    hasher = hashlib.sha256()
    with open(file_path, "rb") as f:
        while chunk := f.read(1024 * 1024):  # 1 MB buffer
            hasher.update(chunk)
    return hasher.hexdigest()


class CanonicalCorpusGenerator:
    """Generates canonical test datasets for TTZip multilingual SDK testing."""

    def __init__(self, root_dir: Path, verbose: bool = False):
        self.root_dir = root_dir.resolve()
        self.verbose = verbose
        self.manifest_entries: List[Dict[str, Any]] = []

    def log(self, message: str) -> None:
        if self.verbose:
            print(f"[CanonicalCorpus] {message}")

    def clean(self) -> None:
        """Removes existing canonical corpus directory if present."""
        if self.root_dir.exists():
            self.log(f"Cleaning existing directory: {self.root_dir}")
            shutil.rmtree(self.root_dir)
        self.root_dir.mkdir(parents=True, exist_ok=True)

    def register_entry(self, dataset: str, file_path: Path, entry_type: str = "file") -> Dict[str, Any]:
        """Calculates checksum and registers item in manifest."""
        rel_path = file_path.relative_to(self.root_dir).as_posix()
        size_bytes = 0
        sha256 = ""

        if entry_type == "file" or entry_type == "sparse_file":
            size_bytes = file_path.stat().st_size
            sha256 = compute_sha256(file_path)
        elif entry_type == "directory":
            size_bytes = 0
            sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"  # SHA-256 of empty string

        entry = {
            "dataset": dataset,
            "relativePath": rel_path,
            "sizeBytes": size_bytes,
            "sha256": sha256,
            "type": entry_type,
        }
        self.manifest_entries.append(entry)
        return entry

    def generate_ascii_text(self) -> List[Dict[str, Any]]:
        """1. Standard ASCII text files of various sizes and structures."""
        self.log("Generating Dataset 1: ASCII Text Corpus...")
        target_dir = self.root_dir / "ascii_text"
        target_dir.mkdir(parents=True, exist_ok=True)
        self.register_entry("ascii_text", target_dir, "directory")

        # 1.1 Small text file (1 KB)
        small_text = target_dir / "sample_small.txt"
        content_1k = (
            "The quick brown fox jumps over the lazy dog.\n"
            "TTZip: Extreme performance multithreaded compression engine.\n"
            "Testing C-ABI 2.0 and native zero-subprocess SDK interop.\n"
        ) * 10
        small_text.write_text(content_1k, encoding="ascii")
        self.register_entry("ascii_text", small_text)

        # 1.2 Structured RFC text (64 KB)
        rfc_text = target_dir / "structured_rfc1951.txt"
        rfc_lines = [
            f"Line {i:05d}: RFC 1951 DEFLATE Compressed Data Format Specification compliant test stream.\n"
            for i in range(1, 800)
        ]
        rfc_text.write_text("".join(rfc_lines), encoding="ascii")
        self.register_entry("ascii_text", rfc_text)

        # 1.3 C-ABI Header pseudo source code (16 KB)
        c_src = target_dir / "ttzip_cabi_sample.c"
        c_content = [
            "/* SPDX-License-Identifier: BSD-3-Clause */\n",
            "#include <stdint.h>\n#include <stddef.h>\n#include <stdbool.h>\n\n",
            "typedef struct TTZipEngineContext TTZipEngineContext;\n",
            "int32_t ttzip_rust_create_archive(const char* dst, const char* src, uint32_t flags);\n",
            "int32_t ttzip_rust_extract_archive(const char* src, const char* dst, uint32_t flags);\n",
        ]
        for i in range(200):
            c_content.append(f"int32_t ttzip_stub_function_{i:03d}(void* ctx, size_t len);\n")
        c_src.write_text("".join(c_content), encoding="ascii")
        self.register_entry("ascii_text", c_src)

        # 1.4 Structured key-value configuration file
        config_file = target_dir / "engine_settings.ini"
        ini_content = (
            "[engine]\nthreads = 8\ncompression_level = 6\nchunk_size_mb = 64\n"
            "[security]\nmax_ratio = 1000\nblock_traversal = true\n"
            "[vfs]\npaging_threshold_mb = 256\narena_growth_mb = 32\n"
        )
        config_file.write_text(ini_content, encoding="ascii")
        self.register_entry("ascii_text", config_file)

        return [e for e in self.manifest_entries if e["dataset"] == "ascii_text"]

    def generate_nested_tree(self) -> List[Dict[str, Any]]:
        """2. Deeply nested multi-level directory tree with branches and empty folders."""
        self.log("Generating Dataset 2: Nested Multi-Level Directory Tree...")
        base_dir = self.root_dir / "nested_tree"
        base_dir.mkdir(parents=True, exist_ok=True)
        self.register_entry("nested_tree", base_dir, "directory")

        # Create deep 6-level chain: root/level1/level2/level3/level4/level5/level6
        current = base_dir
        for level in range(1, 7):
            current = current / f"level_{level:02d}"
            current.mkdir(parents=True, exist_ok=True)
            self.register_entry("nested_tree", current, "directory")

            # Add a file at this level
            leaf_file = current / f"depth_marker_{level}.txt"
            leaf_file.write_text(f"Hierarchy depth level {level}\nPath: {leaf_file}\n", encoding="utf-8")
            self.register_entry("nested_tree", leaf_file)

        # Deepest leaf payload
        deep_leaf = current / "deepest_payload.bin"
        deep_leaf.write_bytes(b"\xAA\xBB\xCC\xDD" * 256)
        self.register_entry("nested_tree", deep_leaf)

        # Branching sub-trees
        branch_a = base_dir / "branch_alpha" / "sub_branch"
        branch_a.mkdir(parents=True, exist_ok=True)
        self.register_entry("nested_tree", base_dir / "branch_alpha", "directory")
        self.register_entry("nested_tree", branch_a, "directory")

        branch_file_1 = branch_a / "alpha_config.json"
        branch_file_1.write_text(json.dumps({"branch": "alpha", "active": True}), encoding="utf-8")
        self.register_entry("nested_tree", branch_file_1)

        branch_b = base_dir / "branch_beta"
        branch_b.mkdir(parents=True, exist_ok=True)
        self.register_entry("nested_tree", branch_b, "directory")

        branch_file_2 = branch_b / "beta_data.dat"
        branch_file_2.write_bytes(b"TTZip Branch Beta Binary Test Vector" * 16)
        self.register_entry("nested_tree", branch_file_2)

        # Explicit empty directory
        empty_dir = base_dir / "empty_subfolder"
        empty_dir.mkdir(parents=True, exist_ok=True)
        self.register_entry("nested_tree", empty_dir, "directory")

        return [e for e in self.manifest_entries if e["dataset"] == "nested_tree"]

    def generate_multibyte_cjk_emoji(self) -> List[Dict[str, Any]]:
        """3. Multibyte UTF-8 CJK, accented, and Emoji filenames."""
        self.log("Generating Dataset 3: Multibyte CJK / Emoji / Diacritic Filenames...")
        base_dir = self.root_dir / "unicode_cjk_emoji"
        base_dir.mkdir(parents=True, exist_ok=True)
        self.register_entry("unicode_cjk_emoji", base_dir, "directory")

        # 3.1 Chinese (Simplified & Traditional)
        zh_sim = base_dir / "简体中文_测试文档.txt"
        zh_sim.write_text("TTZip 简体中文多字节文件名测试内容。\n极速无损压缩解压验证。\n", encoding="utf-8")
        self.register_entry("unicode_cjk_emoji", zh_sim)

        zh_tra = base_dir / "繁體中文_歸檔測試.txt"
        zh_tra.write_text("TTZip 繁體中文多位元組檔名測試內容。\n高效能跨語言互操作性驗證。\n", encoding="utf-8")
        self.register_entry("unicode_cjk_emoji", zh_tra)

        # 3.2 Japanese (Kanji, Hiragana, Katakana)
        ja_dir = base_dir / "日本語_フォルダ"
        ja_dir.mkdir(parents=True, exist_ok=True)
        self.register_entry("unicode_cjk_emoji", ja_dir, "directory")

        ja_file = ja_dir / "圧縮テスト_ひらがな_カタカナ.txt"
        ja_file.write_text("TTZip 日本語テストファイルです。\nマルチスレッド高速圧縮エンジン。\n", encoding="utf-8")
        self.register_entry("unicode_cjk_emoji", ja_file)

        # 3.3 Korean (Hangul)
        ko_file = base_dir / "한국어_압축_테스트.txt"
        ko_file.write_text("TTZip 한국어 유니코드 파일명 테스트 문서입니다.\n", encoding="utf-8")
        self.register_entry("unicode_cjk_emoji", ko_file)

        # 3.4 Emoji & Unicode Symbols
        emoji_dir = base_dir / "📁_folder_🎉_celebration"
        emoji_dir.mkdir(parents=True, exist_ok=True)
        self.register_entry("unicode_cjk_emoji", emoji_dir, "directory")

        emoji_file = emoji_dir / "🚀_rocket_⚡️_lightning_🔥_fire.txt"
        emoji_file.write_text("Emoji filename verification: 🚀 ⚡️ 🔥 📦 🛡️\n", encoding="utf-8")
        self.register_entry("unicode_cjk_emoji", emoji_file)

        # 3.5 European Diacritics & Accents
        diacritic_file = base_dir / "café_résumé_über_ñ_münchen.txt"
        diacritic_file.write_text("Diacritics: François, René, Müller, España, Smørrebrød.\n", encoding="utf-8")
        self.register_entry("unicode_cjk_emoji", diacritic_file)

        return [e for e in self.manifest_entries if e["dataset"] == "unicode_cjk_emoji"]

    def generate_sparse_file(self) -> List[Dict[str, Any]]:
        """4. Sparse 1GB test file with deterministic header/footer markers and sparse hole."""
        self.log("Generating Dataset 4: Sparse 1GB Test File...")
        target_dir = self.root_dir / "sparse_large"
        target_dir.mkdir(parents=True, exist_ok=True)
        self.register_entry("sparse_large", target_dir, "directory")

        sparse_file = target_dir / "sparse_1gb.bin"
        header = b"TTZIP_SPARSE_CANONICAL_CORPUS_HEADER_V1_MARKER\n"
        footer = b"\nTTZIP_SPARSE_CANONICAL_CORPUS_FOOTER_V1_MARKER"

        with open(sparse_file, "wb") as f:
            f.write(header)
            f.seek(SPARSE_FILE_SIZE - len(footer))
            f.write(footer)

        self.register_entry("sparse_large", sparse_file, "sparse_file")
        return [e for e in self.manifest_entries if e["dataset"] == "sparse_large"]

    def generate_all(self) -> Dict[str, Any]:
        """Generates all 4 canonical datasets and writes manifest.json."""
        self.clean()
        self.generate_ascii_text()
        self.generate_nested_tree()
        self.generate_multibyte_cjk_emoji()
        self.generate_sparse_file()

        manifest = {
            "version": "1.0.0",
            "generator": "generate_canonical_corpus.py",
            "totalEntries": len(self.manifest_entries),
            "datasets": [
                "ascii_text",
                "nested_tree",
                "unicode_cjk_emoji",
                "sparse_large",
            ],
            "entries": self.manifest_entries,
        }

        manifest_path = self.root_dir / "manifest.json"
        manifest_path.write_text(json.dumps(manifest, indent=2, ensure_ascii=False), encoding="utf-8")
        self.log(f"Generated {len(self.manifest_entries)} entries in manifest: {manifest_path}")
        return manifest

    def verify(self) -> Tuple[bool, List[str]]:
        """Verifies existing directory against manifest.json."""
        manifest_path = self.root_dir / "manifest.json"
        if not manifest_path.exists():
            return False, [f"Manifest not found: {manifest_path}"]

        try:
            data = json.loads(manifest_path.read_text(encoding="utf-8"))
            entries = data.get("entries", [])
        except Exception as e:
            return False, [f"Failed to parse manifest: {e}"]

        errors = []
        for entry in entries:
            rel_path = entry["relativePath"]
            expected_sha = entry["sha256"]
            expected_size = entry["sizeBytes"]
            entry_type = entry.get("type", "file")

            target = self.root_dir / rel_path
            if not target.exists():
                errors.append(f"Missing target: {rel_path}")
                continue

            if entry_type == "directory":
                if not target.is_dir():
                    errors.append(f"Expected directory: {rel_path}")
            else:
                if not target.is_file():
                    errors.append(f"Expected regular file: {rel_path}")
                    continue
                actual_size = target.stat().st_size
                if actual_size != expected_size:
                    errors.append(f"Size mismatch {rel_path}: expected {expected_size}, got {actual_size}")
                actual_sha = compute_sha256(target)
                if actual_sha != expected_sha:
                    errors.append(f"SHA-256 mismatch {rel_path}: expected {expected_sha}, got {actual_sha}")

        return len(errors) == 0, errors


def generate_canonical_corpus(output_dir: Optional[Path] = None, clean: bool = True, verbose: bool = False) -> Dict[str, Any]:
    """Programmatic API to generate canonical test corpus."""
    out = output_dir or DEFAULT_OUTPUT_DIR
    generator = CanonicalCorpusGenerator(root_dir=out, verbose=verbose)
    if clean:
        generator.clean()
    return generator.generate_all()


def main() -> int:
    parser = argparse.ArgumentParser(
        description="TTZip Canonical Test Corpus Generator",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument(
        "-o", "--output-dir",
        type=Path,
        default=DEFAULT_OUTPUT_DIR,
        help="Destination directory for canonical corpus",
    )
    parser.add_argument(
        "--dataset",
        choices=["all", "ascii", "nested", "cjk", "sparse"],
        default="all",
        help="Specific dataset to generate",
    )
    parser.add_argument(
        "--clean",
        action="store_true",
        default=True,
        help="Clean target directory before generating",
    )
    parser.add_argument(
        "--verify",
        action="store_true",
        help="Verify existing corpus against manifest.json",
    )
    parser.add_argument(
        "-v", "--verbose",
        action="store_true",
        help="Enable verbose output",
    )

    args = parser.parse_args()
    generator = CanonicalCorpusGenerator(root_dir=args.output_dir, verbose=args.verbose)

    if args.verify:
        print(f"Verifying canonical corpus at {args.output_dir}...")
        valid, errors = generator.verify()
        if valid:
            print("✅ Verification PASSED: All canonical fixtures match SHA-256 manifest.")
            return 0
        else:
            print(f"❌ Verification FAILED with {len(errors)} error(s):")
            for err in errors:
                print(f"  - {err}")
            return 1

    print(f"⚡️ Generating TTZip canonical test corpus in: {args.output_dir}")
    if args.dataset == "all":
        manifest = generator.generate_all()
    else:
        if args.clean:
            generator.clean()
        if args.dataset == "ascii":
            generator.generate_ascii_text()
        elif args.dataset == "nested":
            generator.generate_nested_tree()
        elif args.dataset == "cjk":
            generator.generate_multibyte_cjk_emoji()
        elif args.dataset == "sparse":
            generator.generate_sparse_file()
        manifest = {"entries": generator.manifest_entries}

    print(f"✅ Canonical test corpus generation complete ({len(generator.manifest_entries)} items).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
