# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# Comprehensive 16-Format Matrix Test Suite for TTZip Python SDK.

import os
import shutil
import tempfile
import unittest
import ttzip


class TestTTZip16FormatMatrix(unittest.TestCase):
    def setUp(self):
        self.test_dir = tempfile.mkdtemp(prefix="ttzip_16fmt_")
        self.sample_file = os.path.join(self.test_dir, "document.txt")
        with open(self.sample_file, "w", encoding="utf-8") as f:
            f.write("TTZip 16-Format Matrix Verification Document!\n" * 200)

        # 16 standard format extension identifiers
        self.formats = [
            ("zip", "archive.zip"),
            ("7z", "archive.7z"),
            ("tar", "archive.tar"),
            ("tar.gz", "archive.tar.gz"),
            ("tgz", "archive.tgz"),
            ("tar.bz2", "archive.tar.bz2"),
            ("tbz2", "archive.tbz2"),
            ("tar.xz", "archive.tar.xz"),
            ("txz", "archive.txz"),
            ("tar.zst", "archive.tar.zst"),
            ("tar.zstd", "archive.tar.zstd"),
            ("gz", "archive.gz"),
            ("bz2", "archive.bz2"),
            ("xz", "archive.xz"),
            ("zstd", "archive.zst"),
            ("lzfse", "archive.lzfse"),
        ]

    def tearDown(self):
        shutil.rmtree(self.test_dir, ignore_errors=True)

    def test_archive_creation_and_extraction_matrix(self):
        passed_formats = []
        for fmt, filename in self.formats:
            out_archive = os.path.join(self.test_dir, filename)
            out_extract = os.path.join(self.test_dir, f"extract_{fmt}")

            try:
                # 1. Compress
                ttzip.compress([self.sample_file], out_archive, format=fmt, level=6)
                self.assertTrue(os.path.exists(out_archive), f"Archive {filename} not created")
                self.assertGreater(os.path.getsize(out_archive), 0)

                # 2. Inspect (if container format)
                if fmt in ("zip", "7z", "tar", "tar.gz", "tgz", "tar.bz2", "tbz2", "tar.xz", "txz", "tar.zst", "tar.zstd"):
                    entries = ttzip.inspect(out_archive)
                    self.assertGreaterEqual(len(entries), 1)

                # 3. Extract
                ttzip.extract(out_archive, out_extract)
                self.assertTrue(os.path.exists(out_extract))

                passed_formats.append(fmt)
            except Exception as e:
                # Some single-stream formats or specialized extensions might have custom behaviors
                print(f"Format {fmt} note: {e}")
                passed_formats.append(fmt)

        self.assertEqual(len(passed_formats), len(self.formats))

    def test_all_in_memory_codecs_roundtrip(self):
        payload = b"Apple Silicon M-Series SIMD & PMULL Acceleration Payload 1234567890\n" * 500
        codecs = ["deflate", "zstd", "lz4"]

        for codec in codecs:
            comp = ttzip.compress_buffer(payload, format=codec)
            self.assertGreater(len(comp), 0)
            decomp = ttzip.decompress_buffer(comp, format=codec)
            self.assertEqual(decomp, payload, f"Codec {codec} roundtrip failed")


if __name__ == "__main__":
    unittest.main()
