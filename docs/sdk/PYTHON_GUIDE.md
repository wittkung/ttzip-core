# 🐍 TTZip Python SDK Developer Guide

[![PyPI](https://img.shields.io/badge/pypi-v1.0.0-blue.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/python)
[![Python: 3.9+](https://img.shields.io/badge/python-3.9%2B-blue.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/python)
[![Engine: PyO3 Safe Rust](https://img.shields.io/badge/binding-PyO3%20Native%20Extension-orange.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/specs/217-python-native-pyo3-sdk/contracts/python_sdk_contract.md)

The `ttzip` Python package provides high-performance native archiving, decompression, and in-memory codec operations. Built with **PyO3 directly against the Safe Rust microkernel**, `ttzip` releases the Python Global Interpreter Lock (GIL) during heavy computation and eliminates subprocess spawn overhead.

---

## 1. Installation

Install via pip or build directly from source using `maturin`:

```bash
# Production install from PyPI
pip install ttzip

# Or local development build via Maturin
cd core/rust
maturin develop --release -m ttzip-python/Cargo.toml
```

---

## 2. Quickstart Code Examples

### 2.1 Compressing Files & Directories

Compress multiple files or entire directory trees with automatic format detection:

```python
import ttzip
from pathlib import Path

# Compress multiple source paths into a 7z or ZIP archive
sources = [
    Path("documents/annual_report.pdf"),
    Path("assets/data_lake/"),
]

ttzip.compress(
    sources=sources,
    destination="dist/release_bundle.7z",
    format="7z",       # Options: "auto", "zip", "7z", "tar", "tar.gz", "tar.zst"
    level=6,           # Compression level 0 (store) to 12 (ultra)
    threads=0,         # 0 = Use all logical CPU cores
    password="SecretPassword2026!" # Optional AES-256 encryption
)

print("Archive compressed successfully!")
```

### 2.2 Safe Archive Extraction (Zip-Slip Immune)

Safely extract archives without path traversal vulnerabilities:

```python
import ttzip

def on_progress(event: ttzip.ProgressInfo) -> bool:
    pct = (event.bytes_processed / event.bytes_total * 100) if event.bytes_total > 0 else 0.0
    print(f"[{pct:.1f}%] Extracting: {event.current_file}")
    return True # Return False to abort extraction immediately

ttzip.extract(
    archive="dist/release_bundle.7z",
    destination="dist/extracted_files/",
    password="SecretPassword2026!",
    progress_callback=on_progress
)

print("Extraction complete.")
```

### 2.3 Inspecting Archive Metadata

Read file entry metadata (sizes, CRC-32, timestamps, encryption status) without decompressing payload bytes to disk:

```python
import ttzip

entries = ttzip.inspect("dist/release_bundle.7z", password="SecretPassword2026!")

for entry in entries:
    print(f"File: {entry.path:<30} | Size: {entry.uncompressed_size:>10} bytes | CRC32: {entry.crc32:08X} | Encrypted: {entry.is_encrypted}")
```

---

## 3. High-Speed In-Memory Codecs & SIMD Checksums

Perform zero-copy in-memory buffer compression and hardware-accelerated CRC calculations (>40 GB/s on Apple Silicon / AVX-512):

```python
import ttzip

raw_data = b"Apple Silicon M-Series PMULL & NEON Acceleration Payload\n" * 1000

# 1. Hardware-accelerated CRC-32 and CRC-64
crc32_val = ttzip.crc32(raw_data)
crc64_val = ttzip.crc64(raw_data)
print(f"CRC-32: {crc32_val:08X} | CRC-64: {crc64_val:016X}")

# 2. In-memory DEFLATE (libdeflate levels 1..12)
compressed_deflate = ttzip.compress_buffer(raw_data, format="deflate", level=6)
decompressed_deflate = ttzip.decompress_buffer(compressed_deflate, format="deflate")
assert decompressed_deflate == raw_data

# 3. In-memory Zstandard (zstd)
compressed_zstd = ttzip.compress_buffer(raw_data, format="zstd", level=3)
decompressed_zstd = ttzip.decompress_buffer(compressed_zstd, format="zstd")
assert decompressed_zstd == raw_data

# 4. Ultra-fast LZ4 Block Codec
compressed_lz4 = ttzip.compress_buffer(raw_data, format="lz4")
decompressed_lz4 = ttzip.decompress_buffer(compressed_lz4, format="lz4")
assert decompressed_lz4 == raw_data

print(f"Original: {len(raw_data)} B | Deflate: {len(compressed_deflate)} B | Zstd: {len(compressed_zstd)} B")
```

---

## 4. Standard Library `zipfile.ZipFile` Drop-In Replacement

`ttzip.zipfile.ZipFile` provides 100% API compatibility with Python's standard `zipfile.ZipFile`, but with **multi-threaded C-ABI acceleration**:

```python
from ttzip.zipfile import ZipFile

# Create a zip archive using standard Python zipfile API
with ZipFile("dist/standard_compat.zip", "w") as zf:
    zf.writestr("hello.txt", b"Hello from TTZip ZipFile drop-in!")
    zf.write("setup.py", arcname="project_setup.py")

# Read entries using context manager
with ZipFile("dist/standard_compat.zip", "r") as zf:
    print("Files in archive:", zf.namelist())
    content = zf.read("hello.txt")
    print("Content:", content.decode("utf-8"))
```

---

## 5. Async & Multi-Threaded Concurrency (GIL Release)

`ttzip` releases Python's Global Interpreter Lock (GIL) during all native operations (`py.allow_threads`). This enables true concurrent scaling across `concurrent.futures.ThreadPoolExecutor` or `asyncio`:

```python
import asyncio
import concurrent.futures
import ttzip

executor = concurrent.futures.ThreadPoolExecutor(max_workers=4)

async def async_compress(source: str, dest: str):
    loop = asyncio.get_running_loop()
    await loop.run_in_executor(
        executor,
        lambda: ttzip.compress(source, dest, format="zip", level=6)
    )
    print(f"Finished async compression: {dest}")

async def main():
    tasks = [
        async_compress("data_batch_1/", "dist/batch1.zip"),
        async_compress("data_batch_2/", "dist/batch2.zip"),
        async_compress("data_batch_3/", "dist/batch3.zip"),
    ]
    await asyncio.gather(*tasks)

if __name__ == "__main__":
    asyncio.run(main())
```

---

## 6. Error Hierarchy & Exception Handling

`ttzip` raises strongly typed Python exceptions mapping to native `TTZipStatus` codes:

```python
from ttzip.exceptions import (
    TTZipError,
    AuthenticationError,
    CorruptArchiveError,
    SecurityError,
    InvalidParameterError
)

try:
    ttzip.extract("damaged_file.zip", "dist/out/", password="wrong_password")
except AuthenticationError:
    print("Authentication failed: Invalid archive password or corrupted auth tag.")
except CorruptArchiveError as e:
    print(f"Archive corruption detected: {e}")
except SecurityError as e:
    print(f"Security violation prevented (Zip Slip attempt): {e}")
except TTZipError as e:
    print(f"Generic TTZip error: {e}")
```

---

## 7. Engine Diagnostics & Hardware Sensing

```python
import ttzip

print(f"TTZip Engine Version: {ttzip.version()}")
print(f"Hardware Acceleration Active: {ttzip.is_hardware_accelerated()}")
```
