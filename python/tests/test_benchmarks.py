# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# Performance benchmarks comparing TTZip Python SDK against built-in zipfile and tarfile.

import os
import shutil
import tempfile
import time
import unittest
import zipfile
import zlib

import ttzip


class TestTTZipBenchmarks(unittest.TestCase):
    def setUp(self):
        self.test_dir = tempfile.mkdtemp(prefix="ttzip_py_bench_")
        self.test_file = os.path.join(self.test_dir, "payload_10mb.dat")

        # Generate a 10MB mixed compressible payload
        block = (
            b"The Wall Street Journal: High throughput systems engineering. "
            b"SIMD PMULL acceleration with zero-cost memory allocations. 1234567890\n"
        )
        with open(self.test_file, "wb") as f:
            for _ in range(10 * 1024 * 1024 // len(block) + 1):
                f.write(block)
            f.truncate(10 * 1024 * 1024)

    def tearDown(self):
        shutil.rmtree(self.test_dir, ignore_errors=True)

    def test_benchmark_crc32_throughput(self):
        with open(self.test_file, "rb") as f:
            data = f.read()

        # 1. Python built-in zlib.crc32
        start = time.perf_counter()
        iters = 20
        for _ in range(iters):
            _ = zlib.crc32(data)
        zlib_time = time.perf_counter() - start

        # 2. TTZip SIMD CRC32
        start = time.perf_counter()
        for _ in range(iters):
            _ = ttzip.crc32(data)
        ttzip_time = time.perf_counter() - start

        total_mb = (len(data) * iters) / (1024 * 1024)
        zlib_gbps = (total_mb / 1024) / zlib_time
        ttzip_gbps = (total_mb / 1024) / ttzip_time
        speedup = zlib_time / max(ttzip_time, 1e-6)

        print("\n" + "=" * 65)
        print("⚡️ [Python SDK Benchmark] CRC-32 (10 MB Stream)")
        print("=" * 65)
        print(f"Python zlib.crc32:  {zlib_gbps:8.2f} GB/s ({zlib_time*1000/iters:6.2f} ms/iter)")
        print(f"TTZip SIMD CRC32:  {ttzip_gbps:8.2f} GB/s ({ttzip_time*1000/iters:6.2f} ms/iter)")
        print(f"Acceleration:      {speedup:8.2f}x faster")
        print("=" * 65)

        self.assertGreater(ttzip_gbps, 10.0)  # >10 GB/s on modern CPU

    def test_benchmark_zip_creation_and_extraction(self):
        ttzip_archive = os.path.join(self.test_dir, "ttzip_bench.zip")
        py_archive = os.path.join(self.test_dir, "py_bench.zip")

        # 1. Python standard zipfile
        start = time.perf_counter()
        with zipfile.ZipFile(py_archive, "w", compression=zipfile.ZIP_DEFLATED) as zf:
            zf.write(self.test_file, arcname="payload.dat")
        py_compress_time = time.perf_counter() - start

        # 2. TTZip compress
        start = time.perf_counter()
        ttzip.compress([self.test_file], ttzip_archive, format="zip", level=6)
        ttzip_compress_time = time.perf_counter() - start

        # 3. Python standard extract
        py_out = os.path.join(self.test_dir, "py_extracted")
        start = time.perf_counter()
        with zipfile.ZipFile(py_archive, "r") as zf:
            zf.extractall(py_out)
        py_extract_time = time.perf_counter() - start

        # 4. TTZip extract
        ttzip_out = os.path.join(self.test_dir, "ttzip_extracted")
        start = time.perf_counter()
        ttzip.extract(ttzip_archive, ttzip_out)
        ttzip_extract_time = time.perf_counter() - start

        print("\n" + "=" * 65)
        print("⚡️ [Python SDK Benchmark] ZIP 10 MB Archive Round-Trip")
        print("=" * 65)
        print(f"Zipfile Compression:  {py_compress_time*1000:8.2f} ms")
        print(f"TTZip Compression:    {ttzip_compress_time*1000:8.2f} ms ({py_compress_time/max(ttzip_compress_time, 1e-6):.2f}x)")
        print(f"Zipfile Extraction:   {py_extract_time*1000:8.2f} ms")
        print(f"TTZip Extraction:     {ttzip_extract_time*1000:8.2f} ms ({py_extract_time/max(ttzip_extract_time, 1e-6):.2f}x)")
        print("=" * 65)

        self.assertTrue(os.path.exists(os.path.join(ttzip_out, os.path.basename(self.test_file))))


if __name__ == "__main__":
    unittest.main()
