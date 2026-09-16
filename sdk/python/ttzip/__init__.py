# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

"""
TTZip: Ultra-fast Safe Rust Archiving & Compression Engine for Python.
"""

from typing import Any, List, Optional, Union
from pathlib import Path

from .exceptions import (
    TTZipError,
    AuthenticationError,
    CorruptArchiveError,
    SecurityError,
)
from .models import (
    EntryMetadata,
    BenchmarkPointResult,
    BenchmarkMatrixReport,
    ProgressInfo,
)
from .zipfile import ZipFile, SevenZipFile, open_archive

try:
    from . import ttzip_engine as uniffi_engine
    _HAS_UNIFFI = True
except ImportError:
    uniffi_engine = None
    _HAS_UNIFFI = False

try:
    from . import _ttzip
    _HAS_NATIVE = True
except ImportError:
    _ttzip = None
    _HAS_NATIVE = False

__version__ = "1.0.0"


def compress(
    sources: Union[str, Path, List[Union[str, Path]]],
    destination: Union[str, Path],
    format: str = "auto",
    level: int = 6,
    password: Optional[str] = None,
    threads: int = 0,
) -> None:
    """
    Compresses source files or directories into a target archive (ZIP, 7z, TAR, GZ, ZSTD).
    Prefers Mozilla UniFFI bindings when available, with native C-extension fallback.
    """
    if isinstance(sources, (str, Path)):
        src_list = [str(sources)]
    else:
        src_list = [str(s) for s in sources]

    if _HAS_UNIFFI and uniffi_engine is not None:
        uniffi_engine.create_archive_stream(
            source_paths=src_list,
            destination_archive_path=str(destination),
            password=password,
            progress=None,
            token=None,
        )
        return

    if _HAS_NATIVE and _ttzip is not None:
        _ttzip.compress(
            src_list,
            str(destination),
            format,
            level,
            password,
            threads,
        )
        return

    raise RuntimeError("Neither TTZip UniFFI engine nor native C-extension (_ttzip) is available.")


def extract(
    archive: Union[str, Path],
    destination: Union[str, Path],
    password: Optional[str] = None,
    threads: int = 0,
) -> None:
    """
    Extracts an archive safely with built-in Zip Slip protection.
    Prefers Mozilla UniFFI bindings when available, with native C-extension fallback.
    """
    if _HAS_UNIFFI and uniffi_engine is not None:
        uniffi_engine.extract_archive_stream(
            archive_path=str(archive),
            destination_dir=str(destination),
            password=password,
            progress=None,
            token=None,
        )
        return

    if _HAS_NATIVE and _ttzip is not None:
        _ttzip.extract(
            str(archive),
            str(destination),
            password,
            threads,
        )
        return

    raise RuntimeError("Neither TTZip UniFFI engine nor native C-extension (_ttzip) is available.")


def inspect(
    archive: Union[str, Path],
    password: Optional[str] = None,
) -> List[EntryMetadata]:
    """
    Inspects archive entry metadata without extracting to disk.
    Prefers Mozilla UniFFI bindings when available, with native C-extension fallback.
    """
    if _HAS_UNIFFI and uniffi_engine is not None:
        entries = uniffi_engine.inspect_archive_entries(str(archive), password)
        return [
            EntryMetadata(
                path=e.path,
                uncompressed_size=e.uncompressed_size,
                compressed_size=e.compressed_size,
                crc32=e.crc32,
                is_directory=e.is_directory,
                is_encrypted=e.is_encrypted,
                last_modified=e.last_modified,
            )
            for e in entries
        ]

    if _HAS_NATIVE and _ttzip is not None:
        return _ttzip.inspect(str(archive), password)

    raise RuntimeError("Neither TTZip UniFFI engine nor native C-extension (_ttzip) is available.")


_CODEC_MAP = {
    "deflate": "DEFLATE_RAW",
    "raw": "DEFLATE_RAW",
    "deflate_raw": "DEFLATE_RAW",
    "zlib": "ZLIB",
    "gzip": "GZIP",
    "gz": "GZIP",
    "zstd": "ZSTD",
    "zst": "ZSTD",
    "lz4": "LZ4_FAST",
    "lz4_fast": "LZ4_FAST",
    "lz4_hc": "LZ4_HC",
    "lzfse": "LZFSE",
    "lzvn": "LZVN",
    "brotli": "BROTLI",
    "snappy": "SNAPPY",
    "bzip2": "BZIP2",
    "bz2": "BZIP2",
    "xz": "XZ",
    "fl2": "FL2",
}


def decompress_buffer(
    data: Union[bytes, bytearray, memoryview, Any],
    format: str = "deflate",
) -> bytes:
    """
    Decompresses an in-memory buffer (deflate, zstd, lz4, snappy, lzfse, etc.).
    Supports PyBuffer zero-copy protocol and releases the Python GIL.
    Prefers Mozilla UniFFI bindings when available, with native C-extension fallback.
    """
    if isinstance(data, memoryview):
        data = data.tobytes()
    elif isinstance(data, bytearray):
        data = bytes(data)

    if _HAS_UNIFFI and uniffi_engine is not None:
        codec_name = _CODEC_MAP.get(format.lower())
        if codec_name and hasattr(uniffi_engine.UniFfiCompressionCodec, codec_name):
            codec = getattr(uniffi_engine.UniFfiCompressionCodec, codec_name)
            return bytes(uniffi_engine.uniffi_decompress_buffer(codec, data, None, None))

    if _HAS_NATIVE and _ttzip is not None:
        return _ttzip.decompress_buffer(data, format)

    raise RuntimeError("Neither TTZip UniFFI engine nor native C-extension (_ttzip) is available.")


def compress_buffer(
    data: Union[bytes, bytearray, memoryview, Any],
    format: str = "deflate",
    level: int = 6,
) -> bytes:
    """
    Compresses an in-memory buffer.
    Supports PyBuffer zero-copy protocol and releases the Python GIL.
    Prefers Mozilla UniFFI bindings when available, with native C-extension fallback.
    """
    if isinstance(data, memoryview):
        data = data.tobytes()
    elif isinstance(data, bytearray):
        data = bytes(data)

    if _HAS_UNIFFI and uniffi_engine is not None:
        codec_name = _CODEC_MAP.get(format.lower())
        if codec_name and hasattr(uniffi_engine.UniFfiCompressionCodec, codec_name):
            codec = getattr(uniffi_engine.UniFfiCompressionCodec, codec_name)
            opts = uniffi_engine.UniFfiCompressionOptions(
                level=level,
                acceleration=None,
                window_mb=None,
                ppmd_order=None,
                ppmd_mem_mb=None,
            )
            return bytes(uniffi_engine.uniffi_compress_buffer(codec, data, opts))

    if _HAS_NATIVE and _ttzip is not None:
        return _ttzip.compress_buffer(data, format, level)

    raise RuntimeError("Neither TTZip UniFFI engine nor native C-extension (_ttzip) is available.")


def decompress_into(
    data: Union[bytes, bytearray, memoryview, Any],
    dst_buffer: bytearray,
    format: str = "deflate",
) -> int:
    """
    Zero-copy in-place decompression directly into a pre-allocated mutable buffer.
    Releases the Python GIL during decompression. Returns written byte length.
    """
    if not _HAS_NATIVE:
        raise RuntimeError("TTZip native C-extension (_ttzip) is not compiled or available.")

    if isinstance(data, memoryview):
        data = data.tobytes()
    return _ttzip.decompress_into(data, dst_buffer, format)


def crc32(data: Union[bytes, bytearray, memoryview, Any], seed: int = 0) -> int:
    """
    Computes SIMD-accelerated CRC-32 (>40 GB/s on Apple Silicon / AVX-512).
    Prefers Mozilla UniFFI bindings when available, with native C-extension fallback.
    """
    if isinstance(data, memoryview):
        data = data.tobytes()
    elif isinstance(data, bytearray):
        data = bytes(data)

    if _HAS_UNIFFI and uniffi_engine is not None:
        if seed == 0:
            return uniffi_engine.uniffi_crc32(data)
        else:
            return uniffi_engine.uniffi_crc32_rolling(seed, data)

    if _HAS_NATIVE and _ttzip is not None:
        return _ttzip.crc32(data, seed)

    raise RuntimeError("Neither TTZip UniFFI engine nor native C-extension (_ttzip) is available.")


def crc64(data: Union[bytes, bytearray, memoryview, Any], seed: int = 0) -> int:
    """
    Computes SIMD-accelerated CRC-64.
    Prefers Mozilla UniFFI bindings when available, with native C-extension fallback.
    """
    if isinstance(data, memoryview):
        data = data.tobytes()
    elif isinstance(data, bytearray):
        data = bytes(data)

    if _HAS_UNIFFI and uniffi_engine is not None:
        return uniffi_engine.uniffi_crc64(data, seed if seed != 0 else None)

    if _HAS_NATIVE and _ttzip is not None:
        return _ttzip.crc64(data, seed)

    raise RuntimeError("Neither TTZip UniFFI engine nor native C-extension (_ttzip) is available.")


def version() -> str:
    """Returns the underlying TTZip engine version string."""
    if not _HAS_NATIVE:
        return "1.0.0"
    return _ttzip.version()


def is_hardware_accelerated() -> bool:
    """Returns True if ARM NEON/PMULL or x86 AVX2/AES-NI acceleration is active."""
    if not _HAS_NATIVE:
        return False
    return _ttzip.is_hardware_accelerated()


def benchmark_matrix(
    corpus_type: str = "synthetic_json",
    corpus_size: int = 65536,
    iterations: int = 1,
) -> BenchmarkMatrixReport:
    """
    Executes a high-throughput 50-point matrix benchmark across all algorithms
    (Deflate, Zstandard, LZ4, Brotli, Snappy, Bzip2) and computes Pareto optimality.
    """
    if not _HAS_NATIVE:
        raise RuntimeError("TTZip native C-extension (_ttzip) is not compiled or available.")

    return _ttzip.benchmark_matrix(corpus_type, corpus_size, iterations)


open = open_archive


__all__ = [
    "__version__",
    "compress",
    "extract",
    "inspect",
    "decompress_buffer",
    "compress_buffer",
    "decompress_into",
    "crc32",
    "crc64",
    "version",
    "is_hardware_accelerated",
    "benchmark_matrix",
    "ZipFile",
    "SevenZipFile",
    "open_archive",
    "open",
    "EntryMetadata",
    "BenchmarkPointResult",
    "BenchmarkMatrixReport",
    "ProgressInfo",
    "TTZipError",
    "AuthenticationError",
    "CorruptArchiveError",
    "SecurityError",
]
