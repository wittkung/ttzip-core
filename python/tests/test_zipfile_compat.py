# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# zipfile.ZipFile drop-in compatibility and PyBuffer zero-copy verification test suite.

import os
import shutil
import tempfile
import unittest
import ttzip
from ttzip.zipfile import ZipFile, SevenZipFile, open_archive


class TestZipFileCompatibility(unittest.TestCase):
    def setUp(self):
        self.test_dir = tempfile.mkdtemp(prefix="ttzip_zipfile_compat_")
        self.file1 = os.path.join(self.test_dir, "file1.txt")
        self.file2 = os.path.join(self.test_dir, "file2.log")
        self.content1 = b"Hello from TTZip ZipFile drop-in replacement test!\n" * 50
        self.content2 = b"Log entry line 1234567890 for verification.\n" * 100

        with open(self.file1, "wb") as f:
            f.write(self.content1)
        with open(self.file2, "wb") as f:
            f.write(self.content2)

    def tearDown(self):
        shutil.rmtree(self.test_dir, ignore_errors=True)

    def test_zipfile_write_read_roundtrip(self):
        archive_path = os.path.join(self.test_dir, "test_write.zip")

        # 1. Create archive with context manager
        with ZipFile(archive_path, mode="w", compresslevel=6) as zf:
            zf.write(self.file1)
            zf.write(self.file2)

        self.assertTrue(os.path.exists(archive_path))
        self.assertGreater(os.path.getsize(archive_path), 0)

        # 2. Read back using TTZip ZipFile
        with ZipFile(archive_path, mode="r") as zf:
            names = zf.namelist()
            self.assertTrue(any("file1.txt" in n for n in names))
            self.assertTrue(any("file2.log" in n for n in names))

            info1 = zf.getinfo(names[0])
            self.assertIsNotNone(info1)
            self.assertGreater(info1.uncompressed_size, 0)

            # Read member bytes directly
            read_bytes1 = zf.read(names[0])
            self.assertIn(b"Hello from TTZip ZipFile", read_bytes1)

    def test_zipfile_extract_and_extractall(self):
        archive_path = os.path.join(self.test_dir, "test_extract.zip")

        with ZipFile(archive_path, mode="w") as zf:
            zf.write(self.file1)

        # Extract single member
        dest_single = os.path.join(self.test_dir, "single_dest")
        with ZipFile(archive_path, mode="r") as zf:
            names = zf.namelist()
            out_file = zf.extract(names[0], path=dest_single)
            self.assertTrue(os.path.exists(out_file))

        # Extract all members
        dest_all = os.path.join(self.test_dir, "all_dest")
        with ZipFile(archive_path, mode="r") as zf:
            zf.extractall(path=dest_all)

        self.assertTrue(os.path.exists(dest_all))

    def test_open_archive_helper_and_seven_zip(self):
        archive_path = os.path.join(self.test_dir, "test_7z.7z")

        with open_archive(archive_path, mode="w", format="7z") as szf:
            self.assertIsInstance(szf, SevenZipFile)
            szf.write(self.file1)

        self.assertTrue(os.path.exists(archive_path))

    def test_closed_zipfile_guard(self):
        archive_path = os.path.join(self.test_dir, "guard_test.zip")
        zf = ZipFile(archive_path, mode="w")
        zf.write(self.file1)
        zf.close()

        with self.assertRaises(ValueError):
            zf.read("file1.txt")

        with self.assertRaises(ValueError):
            zf.write(self.file2)


class TestPyBufferZeroCopyVerification(unittest.TestCase):
    def setUp(self):
        self.raw_data = b"PyBuffer Zero-Copy Protocol Verification Payload 2026! " * 500

    def test_pybuffer_bytes_bytearray_memoryview_roundtrip(self):
        # 1. Standard bytes
        comp_bytes = ttzip.compress_buffer(self.raw_data, format="deflate", level=6)
        self.assertLess(len(comp_bytes), len(self.raw_data))
        decomp_bytes = ttzip.decompress_buffer(comp_bytes, format="deflate")
        self.assertEqual(decomp_bytes, self.raw_data)

        # 2. Mutable bytearray input (zero-copy PyBuffer)
        bytearray_input = bytearray(self.raw_data)
        comp_ba = ttzip.compress_buffer(bytearray_input, format="zstd", level=3)
        self.assertLess(len(comp_ba), len(self.raw_data))
        decomp_from_comp = ttzip.decompress_buffer(comp_ba, format="zstd")
        self.assertEqual(decomp_from_comp, self.raw_data)

        # 3. memoryview slice (zero-copy sub-buffer protocol)
        mv = memoryview(self.raw_data)
        slice1 = mv[100:600]
        comp_mv = ttzip.compress_buffer(slice1, format="lz4")
        decomp_mv = ttzip.decompress_buffer(comp_mv, format="lz4")
        self.assertEqual(decomp_mv, bytes(slice1))

    def test_pybuffer_decompress_into_preallocated_buffer(self):
        compressed = ttzip.compress_buffer(self.raw_data, format="deflate", level=6)

        # Preallocate mutable target buffer
        out_buf = bytearray(len(self.raw_data) + 1024)
        written = ttzip.decompress_into(compressed, out_buf, format="deflate")

        self.assertEqual(written, len(self.raw_data))
        self.assertEqual(out_buf[:written], self.raw_data)

    def test_pybuffer_zero_copy_crc_calculation(self):
        mv = memoryview(self.raw_data)
        full_crc = ttzip.crc32(mv)
        self.assertNotEqual(full_crc, 0)

        # Test subview without memory copying
        half = len(mv) // 2
        first_half = mv[:half]
        second_half = mv[half:]

        seed = ttzip.crc32(first_half, 0)
        chained = ttzip.crc32(second_half, seed)
        self.assertEqual(chained, full_crc)

        # CRC-64 on memoryview
        crc64_val = ttzip.crc64(mv)
        self.assertGreater(crc64_val, 0)


if __name__ == "__main__":
    unittest.main()
