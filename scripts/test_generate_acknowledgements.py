#!/usr/bin/env python3
# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause

"""
Unit & negative-case tests for generate_acknowledgements.py.
Verifies that:
1. Header-guard-only third_party packages without traceable METADATA.toml fail fast.
2. Invalid commit hashes or mismatched file SHA-256 digests in METADATA.toml fail fast.
3. Disagreements between fast-lzma2.h version macros and METADATA.toml fail fast.
4. Unrecognized license texts raise RuntimeError instead of falling back to Custom-Permissive.
5. Short/URL-only placeholder license files raise RuntimeError.
"""

import hashlib
import tempfile
import unittest
from pathlib import Path

from generate_acknowledgements import (
    detect_spdx_from_text,
    read_combined_license_files,
    resolve_third_party_metadata,
)


class GenerateAcknowledgementsNegativeTests(unittest.TestCase):
    def test_header_guard_only_without_metadata_toml_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            pkg_dir = Path(tmp) / "lzfse"
            pkg_dir.mkdir()
            (pkg_dir / "lzfse.h").write_text("#ifndef LZFSE_H\n#define LZFSE_H\n#endif\n", encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "Missing traceable METADATA.toml"):
                resolve_third_party_metadata(pkg_dir, "lzfse")

    def test_invalid_commit_sha_in_metadata_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            pkg_dir = Path(tmp) / "lzfse"
            pkg_dir.mkdir()
            hdr = pkg_dir / "lzfse.h"
            hdr.write_text("#ifndef LZFSE_H\n", encoding="utf-8")
            sha = hashlib.sha256(hdr.read_bytes()).hexdigest()
            (pkg_dir / "METADATA.toml").write_text(
                f'name = "lzfse"\nversion = "1.0"\nupstream_url = "https://github.com/lzfse/lzfse"\n'
                f'upstream_tag = "lzfse-1.0"\nupstream_commit = "not-a-40-char-sha"\nspdx = "BSD-3-Clause"\n'
                f'[verified_files_sha256]\n"lzfse.h" = "{sha}"\n',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(RuntimeError, "Invalid 40-char git commit hash"):
                resolve_third_party_metadata(pkg_dir, "lzfse")

    def test_mismatched_file_sha256_in_metadata_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            pkg_dir = Path(tmp) / "lzfse"
            pkg_dir.mkdir()
            (pkg_dir / "lzfse.h").write_text("#ifndef LZFSE_H\n", encoding="utf-8")
            (pkg_dir / "METADATA.toml").write_text(
                'name = "lzfse"\nversion = "1.0"\nupstream_url = "https://github.com/lzfse/lzfse"\n'
                'upstream_tag = "lzfse-1.0"\n'
                'upstream_commit = "88e2d2788b4021d0b2eb9fe2d97352ae9190f128"\n'
                'spdx = "BSD-3-Clause"\n'
                '[verified_files_sha256]\n'
                '"lzfse.h" = "0000000000000000000000000000000000000000000000000000000000000000"\n',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(RuntimeError, "SHA-256 mismatch"):
                resolve_third_party_metadata(pkg_dir, "lzfse")

    def test_header_version_mismatch_with_metadata_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            pkg_dir = Path(tmp) / "fast-lzma2"
            pkg_dir.mkdir()
            hdr = pkg_dir / "fast-lzma2.h"
            hdr.write_text(
                "#define FL2_VERSION_MAJOR 1\n#define FL2_VERSION_MINOR 0\n#define FL2_VERSION_RELEASE 9\n",
                encoding="utf-8",
            )
            sha = hashlib.sha256(hdr.read_bytes()).hexdigest()
            (pkg_dir / "METADATA.toml").write_text(
                'name = "fast-lzma2"\nversion = "1.0.1"\n'
                'upstream_url = "https://github.com/conor42/fast-lzma2"\n'
                'upstream_tag = "v1.0.1"\n'
                'upstream_commit = "9d13017df02e3a0c6cdd1dfd87e141a6a5e2ba30"\n'
                'spdx = "BSD-3-Clause"\n'
                f'[verified_files_sha256]\n"fast-lzma2.h" = "{sha}"\n',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(RuntimeError, "Version mismatch"):
                resolve_third_party_metadata(pkg_dir, "fast-lzma2")

    def test_unrecognized_license_text_fails_without_custom_permissive(self) -> None:
        unknown_text = (
            "Copyright (c) 2026 Example Corp.\n"
            "This software is subject to the Example Custom License v1.0. "
            "All rights not expressly granted are reserved by Example Corp."
        )
        with self.assertRaisesRegex(RuntimeError, "Unrecognized license text for component 'mystery-lib'"):
            detect_spdx_from_text(unknown_text, "mystery-lib")

    def test_short_placeholder_license_file_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            lic = Path(tmp) / "LICENSE"
            lic.write_text("See https://opensource.org/licenses/MIT", encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "unexpectedly short"):
                read_combined_license_files([lic], Path(tmp))


if __name__ == "__main__":
    unittest.main()
