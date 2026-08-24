#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine for Python.
# Self-contained runnable quickstart example.

import os
import sys
import tempfile
from pathlib import Path

import ttzip


def main() -> None:
    print("=" * 70)
    print(f"⚡️ TTZip Python SDK (v{ttzip.__version__}) Quickstart")
    print("=" * 70)

    hw_accel = ttzip.is_hardware_accelerated()
    print(f"• Hardware Acceleration (ARM NEON / AVX-512): {hw_accel}")
    print(f"• Native Engine Version: {ttzip.version()}")
    print()

    # -------------------------------------------------------------------------
    # 1. SIMD-Accelerated CRC-32 & In-Memory Codecs
    # -------------------------------------------------------------------------
    payload = b"High-Performance SIMD-Accelerated Python Data Stream" * 100
    crc = ttzip.crc32(payload)
    print(f"[1] SIMD CRC-32: 0x{crc:08X} for {len(payload)} bytes")

    compressed = ttzip.compress_buffer(payload, format="zstd", level=3)
    decompressed = ttzip.decompress_buffer(compressed, format="zstd")
    assert decompressed == payload, "Decompression mismatch!"
    ratio = len(payload) / len(compressed)
    print(f"    Zstandard buffer: {len(payload)}B -> {len(compressed)}B (Ratio: {ratio:.2f}x)")
    print()

    # -------------------------------------------------------------------------
    # 2. Direct High-Throughput Archiving (compress, inspect, extract)
    # -------------------------------------------------------------------------
    with tempfile.TemporaryDirectory(prefix="ttzip_quickstart_") as tmpdir:
        tmp_path = Path(tmpdir)

        # Create dummy source files
        file1 = tmp_path / "dataset.csv"
        file1.write_text("id,name,score\n1,Alice,98.5\n2,Bob,91.2\n3,Charlie,95.0\n" * 50)

        file2 = tmp_path / "config.json"
        file2.write_text('{"project": "TTZip", "version": "1.0.0", "active": true}')

        archive_zip = tmp_path / "bundle.zip"
        archive_extract = tmp_path / "extracted_direct"

        print(f"[2] Compressing archive via ttzip.compress: {archive_zip.name}")
        ttzip.compress(
            sources=[file1, file2],
            destination=archive_zip,
            format="zip",
            level=6,
        )
        print(f"    Created: {archive_zip} ({archive_zip.stat().st_size} bytes)")

        # Inspect without extracting to disk
        print(f"[3] Inspecting archive via ttzip.inspect:")
        entries = ttzip.inspect(archive_zip)
        for e in entries:
            print(f"    - {e.path:<16} uncompressed: {e.uncompressed_size:>6}B | compressed: {e.compressed_size:>6}B | CRC32: 0x{e.crc32:08X}")

        # Extract safely
        print(f"[4] Extracting archive via ttzip.extract -> {archive_extract.name}/")
        ttzip.extract(archive_zip, archive_extract)
        assert (archive_extract / "dataset.csv").exists()
        assert (archive_extract / "config.json").exists()
        print("    Extraction verified successfully.")
        print()

        # ---------------------------------------------------------------------
        # 3. Standard Library zipfile.ZipFile Drop-In Compatibility
        # ---------------------------------------------------------------------
        compat_zip = tmp_path / "compat_demo.zip"
        compat_extract = tmp_path / "extracted_compat"

        print(f"[5] Using zipfile.ZipFile drop-in context manager:")
        # Write mode
        with ttzip.ZipFile(compat_zip, mode="w", compresslevel=6) as zf:
            zf.write(file1)
            zf.write(file2)
        print(f"    Written archive: {compat_zip.name} ({compat_zip.stat().st_size} bytes)")

        # Read mode
        with ttzip.ZipFile(compat_zip, mode="r") as zf:
            names = zf.namelist()
            print(f"    Archive members ({len(names)}): {names}")
            csv_bytes = zf.read("dataset.csv")
            print(f"    Read 'dataset.csv' ({len(csv_bytes)} bytes)")
            info = zf.getinfo("config.json")
            print(f"    Getinfo 'config.json': size={info.uncompressed_size}B, CRC=0x{info.crc32:08X}")
            zf.extractall(compat_extract)

        assert (compat_extract / "dataset.csv").exists()
        assert (compat_extract / "config.json").exists()
        print("    ZipFile drop-in extraction verified successfully.")

    print()
    print("=" * 70)
    print("✅ All TTZip Python SDK operations completed successfully!")
    print("=" * 70)


if __name__ == "__main__":
    main()
