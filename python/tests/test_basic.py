# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# Comprehensive functional tests for TTZip Python SDK.

import os
import shutil
import tempfile
import unittest
from pathlib import Path

import ttzip


class TestTTZipBasic(unittest.TestCase):
    def setUp(self):
        self.test_dir = tempfile.mkdtemp(prefix="ttzip_py_test_")
        self.src_dir = os.path.join(self.test_dir, "src")
        self.dest_dir = os.path.join(self.test_dir, "dest")
        os.makedirs(self.src_dir, exist_ok=True)
        os.makedirs(self.dest_dir, exist_ok=True)

        # Generate sample test files
        self.file1 = os.path.join(self.src_dir, "sample.txt")
        with open(self.file1, "w", encoding="utf-8") as f:
            f.write("Hello TTZip High Performance Python Engine!\n" * 100)

        self.file2 = os.path.join(self.src_dir, "binary.dat")
        with open(self.file2, "wb") as f:
            f.write(bytes(range(256)) * 64)

    def tearDown(self):
        shutil.rmtree(self.test_dir, ignore_errors=True)

    def test_version_and_acceleration(self):
        ver = ttzip.version()
        self.assertTrue(len(ver) > 0)
        self.assertTrue(isinstance(ttzip.is_hardware_accelerated(), bool))

    def test_zip_create_and_extract(self):
        archive_path = os.path.join(self.test_dir, "test.zip")
        out_extract = os.path.join(self.dest_dir, "extracted_zip")

        # Compress
        ttzip.compress([self.file1, self.file2], archive_path, format="zip", level=6)
        self.assertTrue(os.path.exists(archive_path))
        self.assertGreater(os.path.getsize(archive_path), 0)

        # Inspect
        entries = ttzip.inspect(archive_path)
        self.assertEqual(len(entries), 2)
        names = [os.path.basename(e.path) for e in entries]
        self.assertIn("sample.txt", names)
        self.assertIn("binary.dat", names)

        # Extract
        ttzip.extract(archive_path, out_extract)
        self.assertTrue(os.path.exists(os.path.join(out_extract, "sample.txt")))
        self.assertTrue(os.path.exists(os.path.join(out_extract, "binary.dat")))

    def test_sevenz_create_and_extract(self):
        archive_path = os.path.join(self.test_dir, "test.7z")
        out_extract = os.path.join(self.dest_dir, "extracted_7z")

        ttzip.compress([self.file1, self.file2], archive_path, format="7z", level=6)
        self.assertTrue(os.path.exists(archive_path))

        entries = ttzip.inspect(archive_path)
        self.assertGreaterEqual(len(entries), 2)

        ttzip.extract(archive_path, out_extract)
        self.assertTrue(os.path.exists(os.path.join(out_extract, "sample.txt")))

    def test_tar_create_and_extract(self):
        archive_path = os.path.join(self.test_dir, "test.tar")
        out_extract = os.path.join(self.dest_dir, "extracted_tar")

        ttzip.compress([self.file1, self.file2], archive_path, format="tar")
        self.assertTrue(os.path.exists(archive_path))

        ttzip.extract(archive_path, out_extract)
        self.assertTrue(os.path.exists(os.path.join(out_extract, "sample.txt")))


if __name__ == "__main__":
    unittest.main()
