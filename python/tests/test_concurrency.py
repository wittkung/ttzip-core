# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# Concurrency and GIL release test suite for TTZip Python SDK.

import concurrent.futures
import os
import shutil
import tempfile
import time
import unittest

import ttzip


class TestTTZipConcurrency(unittest.TestCase):
    def setUp(self):
        self.test_dir = tempfile.mkdtemp(prefix="ttzip_py_conc_")

    def tearDown(self):
        shutil.rmtree(self.test_dir, ignore_errors=True)

    def test_concurrent_buffer_compression(self):
        # 1 MB buffer per task
        payload = b"Parallel concurrency and GIL release validation data! " * 20000

        def worker(idx: int) -> int:
            compressed = ttzip.compress_buffer(payload, format="zstd", level=3)
            decompressed = ttzip.decompress_buffer(compressed, format="zstd")
            assert decompressed == payload
            return len(compressed)

        # Run across 8 worker threads
        start = time.perf_counter()
        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
            futures = [executor.submit(worker, i) for i in range(16)]
            results = [f.result() for f in futures]
        elapsed = time.perf_counter() - start

        self.assertEqual(len(results), 16)
        self.assertTrue(all(r > 0 for r in results))
        # Total processed: 16 MB uncompressed, should complete in <1 second
        self.assertLess(elapsed, 2.0)

    def test_concurrent_archive_creation(self):
        def create_task(idx: int) -> str:
            src = os.path.join(self.test_dir, f"file_{idx}.txt")
            with open(src, "w") as f:
                f.write(f"Archive task payload {idx}\n" * 1000)
            dst = os.path.join(self.test_dir, f"archive_{idx}.zip")
            ttzip.compress([src], dst, format="zip")
            assert os.path.exists(dst)
            return dst

        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
            futures = [executor.submit(create_task, i) for i in range(8)]
            archives = [f.result() for f in futures]

        self.assertEqual(len(archives), 8)


if __name__ == "__main__":
    unittest.main()
