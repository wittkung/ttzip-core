# Copyright 2026 MetastudyLine. All rights reserved.
# Proprietary and confidential.

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


if __name__ == "__main__":
    unittest.main()
