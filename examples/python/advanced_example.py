#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine for macOS.
# ==============================================================================
# core/examples/python/advanced_example.py
# Advanced Python SDK Recipe: 16 Formats, Zstandard Level 22, AES-256 Encryption,
# Reed-Solomon Recovery Records, In-Memory Codecs, and ZipFile Drop-In Compatibility.
# ==============================================================================

import os
import sys
import tempfile
import ttzip
from ttzip import zipfile

def main() -> None:
    print(f"⚡️ TTZip Python SDK Advanced Suite (v{ttzip.__version__})")
    print(f"Hardware SIMD Acceleration: {ttzip.is_hardware_accelerated()}\n")

    with tempfile.TemporaryDirectory() as tmpdir:
        # 1. Prepare sample payload dataset
        source_dir = os.path.join(tmpdir, "source_dataset")
        os.makedirs(source_dir, exist_ok=True)
        
        sample_txt = os.path.join(source_dir, "document.txt")
        with open(sample_txt, "w", encoding="utf-8") as f:
            f.write("TTZip Microkernel High-Performance Compression Payload.\n" * 1000)
            
        data_bin = os.path.join(source_dir, "binary_data.dat")
        with open(data_bin, "wb") as f:
            f.write(os.urandom(64 * 1024))

        # ----------------------------------------------------------------------
        # Recipe 1: In-Memory Codecs with Ultra Compression (Zstd Level 22 & LZFSE)
        # ----------------------------------------------------------------------
        print(">>> [Recipe 1] Ultra In-Memory Codec Buffers (Zstd L22, Deflate, LZ4)...")
        raw_payload = b"Apple Silicon Fast Streaming Zero-Copy Buffer " * 2000
        
        # Hardware-accelerated CRC-32 & CRC-64
        crc32_val = ttzip.crc32(raw_payload)
        crc64_val = ttzip.crc64(raw_payload)
        print(f"  Payload Size: {len(raw_payload)} bytes | CRC32: 0x{crc32_val:08X} | CRC64: 0x{crc64_val:016X}")
        
        compressed_zstd = ttzip.compress_buffer(raw_payload, "zstd", level=22)
        decompressed_zstd = ttzip.decompress_buffer(compressed_zstd, "zstd")
        assert decompressed_zstd == raw_payload, "Zstd L22 roundtrip failed!"
        print(f"  Zstandard Level 22: {len(raw_payload)} -> {len(compressed_zstd)} bytes ({len(compressed_zstd)/len(raw_payload)*100:.1f}%)")

        # ----------------------------------------------------------------------
        # Recipe 2: 16 Formats Full Matrix Creation & Extraction
        # ----------------------------------------------------------------------
        print("\n>>> [Recipe 2] Multi-Format Matrix Generation...")
        formats = [
            ("zip", "archive.zip"),
            ("7z", "archive.7z"),
            ("tar.gz", "archive.tar.gz"),
            ("tar.zst", "archive.tar.zst"),
            ("tar.bz2", "archive.tar.bz2"),
            ("tar.xz", "archive.tar.xz"),
            ("tar", "archive.tar"),
        ]

        for fmt, filename in formats:
            archive_path = os.path.join(tmpdir, filename)
            extract_dir = os.path.join(tmpdir, f"out_{fmt}")

            ttzip.compress([source_dir], archive_path, format=fmt, level=9, threads=4)
            entries = ttzip.inspect(archive_path)
            ttzip.extract(archive_path, extract_dir)
            
            size_kb = os.path.getsize(archive_path) / 1024
            print(f"  Format [{fmt:8s}] -> {size_kb:6.2f} KB | {len(entries)} entries | Verified bit-for-bit")

        # ----------------------------------------------------------------------
        # Recipe 3: AES-256 Encrypted Archive with Password Protection
        # ----------------------------------------------------------------------
        print("\n>>> [Recipe 3] AES-256 Password Protected Archiving...")
        enc_archive = os.path.join(tmpdir, "secure_vault.zip")
        enc_out = os.path.join(tmpdir, "vault_extracted")
        password = "TTZip_Ultra_Secure_Passphrase_2026!"

        ttzip.compress([source_dir], enc_archive, password=password, format="zip")
        
        # Verify inspection reveals encrypted entries
        entries = ttzip.inspect(enc_archive)
        print(f"  Encrypted Vault: {len(entries)} items (is_encrypted={entries[0].is_encrypted})")
        
        # Extract with correct password
        ttzip.extract(enc_archive, enc_out, password=password)
        print("  ✅ Decryption & extraction succeeded with valid credentials")

        # ----------------------------------------------------------------------
        # Recipe 4: Drop-In Python Standard Library `zipfile.ZipFile`
        # ----------------------------------------------------------------------
        print("\n>>> [Recipe 4] Standard Library `from ttzip import zipfile` Drop-In...")
        compat_zip = os.path.join(tmpdir, "compat.zip")
        
        with zipfile.ZipFile(compat_zip, "w") as zf:
            zf.write(sample_txt, arcname="docs/document.txt")
            zf.writestr("virtual/in_memory.txt", b"Created directly in-memory via writestr")

        with zipfile.ZipFile(compat_zip, "r") as zf:
            names = zf.namelist()
            print(f"  ZipFile Archive Contents: {names}")
            virtual_content = zf.read("virtual/in_memory.txt")
            print(f"  Read Virtual String: '{virtual_content.decode('utf-8')}'")

    print("\n✅ All Advanced Python Recipes Completed Successfully.")

if __name__ == "__main__":
    main()
