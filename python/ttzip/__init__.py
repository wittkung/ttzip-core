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
    from . import _ttzip
    _HAS_NATIVE = True
except ImportError:
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
    Releases the Python GIL during heavy compression.
    """
    if not _HAS_NATIVE:
        raise RuntimeError("TTZip native C-extension (_ttzip) is not compiled or available.")

    if isinstance(sources, (str, Path)):
        src_list = [str(sources)]
    else:
        src_list = [str(s) for s in sources]

    _ttzip.compress(
        src_list,
        str(destination),
        format,
        level,
        password,
        threads,
    )


def extract(
    archive: Union[str, Path],
    destination: Union[str, Path],
    password: Optional[str] = None,
    threads: int = 0,
) -> None:
    """
    Extracts an archive safely with built-in Zip Slip protection.
    Releases the Python GIL during extraction.
    """
    if not _HAS_NATIVE:
        raise RuntimeError("TTZip native C-extension (_ttzip) is not compiled or available.")

    _ttzip.extract(
        str(archive),
        str(destination),
        password,
        threads,
    )


def inspect(
    archive: Union[str, Path],
    password: Optional[str] = None,
) -> List[EntryMetadata]:
    """
    Inspects archive entry metadata without extracting to disk.
    """
    if not _HAS_NATIVE:
        raise RuntimeError("TTZip native C-extension (_ttzip) is not compiled or available.")

    return _ttzip.inspect(str(archive), password)


def decompress_buffer(
    data: Union[bytes, bytearray, memoryview, Any],
    format: str = "deflate",
) -> bytes:
    """
    Decompresses an in-memory buffer (deflate, zstd, lz4, snappy, lzfse).
    Supports PyBuffer zero-copy protocol and releases the Python GIL.
    """
    if not _HAS_NATIVE:
        raise RuntimeError("TTZip native C-extension (_ttzip) is not compiled or available.")

    if isinstance(data, memoryview):
        data = data.tobytes()
    return _ttzip.decompress_buffer(data, format)


def compress_buffer(
    data: Union[bytes, bytearray, memoryview, Any],
    format: str = "deflate",
    level: int = 6,
) -> bytes:
    """
    Compresses an in-memory buffer.
    Supports PyBuffer zero-copy protocol and releases the Python GIL.
    """
    if not _HAS_NATIVE:
        raise RuntimeError("TTZip native C-extension (_ttzip) is not compiled or available.")

    if isinstance(data, memoryview):
        data = data.tobytes()
    return _ttzip.compress_buffer(data, format, level)


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
    """
    if not _HAS_NATIVE:
        raise RuntimeError("TTZip native C-extension (_ttzip) is not compiled or available.")

    if isinstance(data, memoryview):
        data = data.tobytes()
    return _ttzip.crc32(data, seed)


def crc64(data: Union[bytes, bytearray, memoryview, Any], seed: int = 0) -> int:
    """
    Computes SIMD-accelerated CRC-64.
    """
    if not _HAS_NATIVE:
        raise RuntimeError("TTZip native C-extension (_ttzip) is not compiled or available.")

    if isinstance(data, memoryview):
        data = data.tobytes()
    return _ttzip.crc64(data, seed)


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
