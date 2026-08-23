# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# In-memory buffer codecs and SIMD checksum test suite.

import unittest
import zlib
import ttzip


class TestTTZipBuffers(unittest.TestCase):
    def setUp(self):
        # 100 KB payload with mixed repetition
        self.payload = (b"TTZip High Performance Buffer Compression Payload! 1234567890\n" * 1500)

    def test_deflate_buffer_roundtrip(self):
        compressed = ttzip.compress_buffer(self.payload, format="deflate", level=6)
        self.assertLess(len(compressed), len(self.payload))

        decompressed = ttzip.decompress_buffer(compressed, format="deflate")
        self.assertEqual(decompressed, self.payload)

    def test_zstd_buffer_roundtrip(self):
        compressed = ttzip.compress_buffer(self.payload, format="zstd", level=3)
        self.assertLess(len(compressed), len(self.payload))

        decompressed = ttzip.decompress_buffer(compressed, format="zstd")
        self.assertEqual(decompressed, self.payload)

    def test_lz4_buffer_roundtrip(self):
        compressed = ttzip.compress_buffer(self.payload, format="lz4")
        self.assertLess(len(compressed), len(self.payload))

        decompressed = ttzip.decompress_buffer(compressed, format="lz4")
        self.assertEqual(decompressed, self.payload)

    def test_simd_crc32_correctness(self):
        # Verify hardware SIMD CRC32 matches Python standard zlib.crc32
        expected = zlib.crc32(self.payload) & 0xFFFFFFFF
        actual = ttzip.crc32(self.payload)
        self.assertEqual(actual, expected)

    def test_simd_crc64(self):
        val = ttzip.crc64(self.payload)
        self.assertGreater(val, 0)


if __name__ == "__main__":
    unittest.main()
