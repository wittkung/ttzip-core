# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause

import unittest


class TestTTZipPythonBinding(unittest.TestCase):
    def test_import_and_version(self):
        import _ttzip

        ver = _ttzip.version()
        self.assertIsInstance(ver, str)
        self.assertTrue(len(ver) > 0)

    def test_crc32(self):
        import _ttzip

        data = b"MetastudyLine Industrial Monorepo"
        crc = _ttzip.crc32(data, 0)
        self.assertIsInstance(crc, int)
        self.assertNotEqual(crc, 0)

    def test_compress_decompress_buffer(self):
        import _ttzip

        data = b"Pure Safe Rust High-Throughput Archiving and Compression" * 10
        compressed = _ttzip.compress_buffer(data, "deflate", 6)
        self.assertTrue(len(compressed) < len(data))
        decompressed = _ttzip.decompress_buffer(compressed, "deflate")
        self.assertEqual(bytes(decompressed), data)

    def test_bytearray_and_decompress_into_safety(self):
        import _ttzip

        raw = b"Mutable bytearray and memoryview safety verification " * 16
        compressed = _ttzip.compress_buffer(bytearray(raw), "deflate", 6)
        self.assertEqual(
            _ttzip.decompress_buffer(memoryview(compressed), "deflate"),
            raw,
        )

        dst = bytearray(len(raw))
        written = _ttzip.decompress_into(bytearray(compressed), dst, "deflate")
        self.assertEqual(written, len(raw))
        self.assertEqual(bytes(dst), raw)


if __name__ == "__main__":
    unittest.main()
