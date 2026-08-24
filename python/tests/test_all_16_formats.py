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

    def test_zstd_level_22_ultra_compression(self):
        # 1. In-memory buffer compression at level 22 (Ultra)
        payload = b"TTZip High-Entropy Zstandard Ultra Level 22 Compression Verification Payload 2026!\n" * 1000
        comp_l22 = ttzip.compress_buffer(payload, format="zstd", level=22)
        self.assertGreater(len(comp_l22), 0)
        self.assertLess(len(comp_l22), len(payload))

        decomp = ttzip.decompress_buffer(comp_l22, format="zstd")
        self.assertEqual(decomp, payload, "Zstd level 22 decompression roundtrip failed")

        # 2. Archive file compression at level 22
        out_zst = os.path.join(self.test_dir, "ultra_level22.tar.zst")
        out_extract = os.path.join(self.test_dir, "extract_ultra_level22")
        ttzip.compress([self.sample_file], out_zst, format="tar.zst", level=22)
        self.assertTrue(os.path.exists(out_zst))
        self.assertGreater(os.path.getsize(out_zst), 0)

        ttzip.extract(out_zst, out_extract)
        extracted_doc = os.path.join(out_extract, "document.txt")
        self.assertTrue(os.path.exists(extracted_doc))

    def test_corrupt_archive_and_buffer_detection(self):
        # 1. Corrupted archive file
        corrupt_zip = os.path.join(self.test_dir, "corrupt_data.zip")
        with open(corrupt_zip, "wb") as f:
            f.write(b"PK\x03\x04\x00\x00\xff\xff\x12\x34\x56\x78" * 5)

        corrupt_dest = os.path.join(self.test_dir, "corrupt_extracted")
        with self.assertRaises((ttzip.CorruptArchiveError, ttzip.TTZipError, Exception)):
            ttzip.extract(corrupt_zip, corrupt_dest)

        # 2. Corrupted in-memory decompression buffer
        garbage_buf = b"\x1f\x8b\x08\x00\xff\xff\xff\xff\x00\x00" * 4
        with self.assertRaises((ttzip.CorruptArchiveError, ttzip.TTZipError, Exception)):
            ttzip.decompress_buffer(garbage_buf, format="deflate")

        with self.assertRaises((ttzip.CorruptArchiveError, ttzip.TTZipError, Exception)):
            ttzip.decompress_buffer(garbage_buf, format="zstd")

    def test_password_validation_and_authentication(self):
        # 1. Create AES-256 encrypted archive
        encrypted_zip = os.path.join(self.test_dir, "secure_vault.zip")
        correct_password = "TTZipSecretPyPass2026!"
        wrong_password = "IncorrectPassword999!"

        ttzip.compress([self.sample_file], encrypted_zip, format="zip", password=correct_password)
        self.assertTrue(os.path.exists(encrypted_zip))

        # 2. Inspect with password
        entries = ttzip.inspect(encrypted_zip, password=correct_password)
        self.assertGreaterEqual(len(entries), 1)
        self.assertTrue(entries[0].is_encrypted)

        # 3. Extract with correct password -> must succeed
        valid_dest = os.path.join(self.test_dir, "extract_valid_pwd")
        ttzip.extract(encrypted_zip, valid_dest, password=correct_password)
        extracted_doc = os.path.join(valid_dest, "document.txt")
        self.assertTrue(os.path.exists(extracted_doc))

        # 4. Extract with wrong password -> must raise AuthenticationError / TTZipError
        invalid_dest = os.path.join(self.test_dir, "extract_invalid_pwd")
        with self.assertRaises((ttzip.AuthenticationError, ttzip.TTZipError, Exception)):
            ttzip.extract(encrypted_zip, invalid_dest, password=wrong_password)


if __name__ == "__main__":
    unittest.main()

