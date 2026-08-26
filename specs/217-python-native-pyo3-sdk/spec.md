# Feature Specification: High-Throughput Native Python SDK via PyO3

**Feature**: `217-python-native-pyo3-sdk`  
**Classification**: `[Full SDD]` (Defines PyO3 native extension boundary, GIL release concurrency model, PEP 561 type stubs, Maturin packaging, and zero-cloud local pytest gate)  
**Status**: `SPECIFIED`  

---

## 1. Executive Summary & Problem Statement

### 1.1 Context
Python is the predominant language for data engineering, AI/ML training data loaders, ETL pipelines, and developer automation. However, Python's built-in compression libraries (`zipfile`, `tarfile`, `gzip`, `bz2`) suffer from critical bottlenecks:
1. **Single-Threaded Execution**: Built-in modules operate sequentially on a single CPU core.
2. **Global Interpreter Lock (GIL) Contention**: Native decompression holding the GIL blocks other worker threads.
3. **Lack of Modern Format Support**: Zero native support for solid 7z, Zstandard multi-threaded streaming, Snappy framing, or LZFSE without wrapping external fragmented C libraries.

`ttzip-core`'s Safe Rust engine already delivers multi-gigabyte/sec throughput with SIMD vectorization and multi-threaded parallel pipelines. Exposing this via **PyO3** directly to Python developers provides a drop-in, high-throughput solution (`pip install ttzip`).

### 1.2 Objectives
1. **Zero-Overhead Safe Rust Binding**: Build a native C-extension crate (`ttzip-python`) using PyO3 0.22+ targeting `ttzip-glue` directly.
2. **GIL-Released Parallelism**: Release the Python GIL (`Python::allow_threads`) during all compression, decompression, and CRC verification routines to enable true linear multi-core scaling in Python `threading.Thread` and `concurrent.futures.ThreadPoolExecutor`.
3. **Ergonomic Pythonic API**: Support Python type hints, context managers, exception mapping (`TTZipError`, `AuthenticationError`, `CorruptArchiveError`, `SecurityError`), and PEP 561 `.pyi` type stubs.
4. **Fast Buffer & Streaming Codecs**: Expose direct byte-to-byte memory decompression (`ttzip.decompress_buffer`) and streaming APIs compatible with Python `bytes` and `bytearray`.
5. **Universal Wheel Packaging**: Support cross-compilation and wheel generation via `maturin` with ABI3 compatibility (Python 3.10+).

---

## 2. User Scenarios & Personas

### Persona 1: AI/ML Data Engineer (PyTorch / HuggingFace)
- **Scenario**: Unpacking a 50GB dataset containing 500,000 images compressed in a solid 7z / ZIP archive.
- **Workflow**: Calls `ttzip.extract("dataset.7z", "/mnt/fast_nvme/train", threads=16)`. Decompression finishes in seconds instead of minutes with automatic multi-core utilization.

### Persona 2: Cloud Backend & Microservices Developer
- **Scenario**: Fast real-time decompression of JSON payloads and web archives using Zstandard or Deflate in FastAPI/Django.
- **Workflow**: Invokes `ttzip.decompress_buffer(raw_payload, format="zstd")` without GIL lock stalling the async event loop.

### Persona 3: Security & Forensic Analyst
- **Scenario**: Inspecting untrusted archives without risking Zip Slip path traversal vulnerabilities.
- **Workflow**: Calls `ttzip.inspect("untrusted.zip")` to retrieve validated `EntryMetadata` structures. If a directory traversal attack is detected, `ttzip.SecurityError` is raised immediately.

---

## 3. Functional Requirements

### 3.1 Native Module & Format Support
- **REQ-PY-001**: The Python package MUST be importable via `import ttzip`.
- **REQ-PY-002**: `ttzip.compress(sources, destination, format="auto", level=6, password=None, threads=0)` MUST support ZIP, 7z, TAR, GZ, BZ2, XZ, and ZSTD.
- **REQ-PY-003**: `ttzip.extract(archive, destination, password=None, threads=0)` MUST extract all supported container formats with automatic path sanitization.
- **REQ-PY-004**: `ttzip.inspect(archive, password=None)` MUST return a list of `EntryMetadata` objects containing `path`, `uncompressed_size`, `compressed_size`, `crc32`, `is_directory`, and `is_encrypted`.

### 3.2 In-Memory Buffers & Codecs
- **REQ-PY-005**: `ttzip.decompress_buffer(data: bytes, format: str = "deflate") -> bytes` MUST decompress raw memory buffers without disk I/O.
- **REQ-PY-006**: `ttzip.compress_buffer(data: bytes, format: str = "deflate", level: int = 6) -> bytes` MUST compress raw memory buffers.
- **REQ-PY-007**: `ttzip.crc32(data: bytes) -> int` and `ttzip.crc64(data: bytes) -> int` MUST provide hardware-accelerated SIMD checksums.

### 3.3 Concurrency & Exception Model
- **REQ-PY-008**: All blocking operations MUST release the Python GIL via `py.allow_threads(...)`.
- **REQ-PY-009**: Rust panics and error status codes MUST be translated into standard Python exceptions:
  - `TTZipError` (Base exception)
  - `FileNotFoundError` (Missing archive or source files)
  - `AuthenticationError` (Missing or invalid password)
  - `CorruptArchiveError` (Damaged headers or CRC mismatches)
  - `SecurityError` (Zip Slip path traversal attempts)

### 3.4 Packaging & Typing Standards
- **REQ-PY-010**: The package MUST provide complete PEP 561 `__init__.pyi` type stubs for mypy, pyright, and IDE autocompletion.
- **REQ-PY-011**: Build configuration MUST use `pyproject.toml` with `maturin` build backend.

---

## 4. Non-Functional Requirements & Success Criteria

| Metric | Target | Verification Method |
| :--- | :--- | :--- |
| **Extraction Speedup vs Python `zipfile`** | $\ge 5\times$ on single-core, $\ge 15\times$ on multi-core | Benchmark script |
| **GIL Release Verification** | 0 GIL stalls during 100MB+ decompression | Multi-threaded concurrency test |
| **Type Check Compliance** | 100% clean under `mypy --strict` | `mypy python/` |
| **Test Pass Rate** | 100% across all format matrix tests | `pytest python/tests` |
