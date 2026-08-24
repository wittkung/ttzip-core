# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
from typing import Any, List, Optional, Union
from pathlib import Path
from .models import (
    EntryMetadata,
    BenchmarkPointResult,
    BenchmarkMatrixReport,
    ProgressInfo,
)
from .exceptions import (
    TTZipError,
    AuthenticationError,
    CorruptArchiveError,
    SecurityError,
)
from .zipfile import ZipFile, SevenZipFile, open_archive

def compress(
    sources: Union[str, Path, List[Union[str, Path]]],
    destination: Union[str, Path],
    format: str = "auto",
    level: int = 6,
    password: Optional[str] = None,
    threads: int = 0,
) -> None: ...

def extract(
    archive: Union[str, Path],
    destination: Union[str, Path],
    password: Optional[str] = None,
    threads: int = 0,
) -> None: ...

def inspect(
    archive: Union[str, Path],
    password: Optional[str] = None,
) -> List[EntryMetadata]: ...

def decompress_buffer(
    data: Union[bytes, bytearray, memoryview, Any],
    format: str = "deflate",
) -> bytes: ...

def compress_buffer(
    data: Union[bytes, bytearray, memoryview, Any],
    format: str = "deflate",
    level: int = 6,
) -> bytes: ...

def decompress_into(
    data: Union[bytes, bytearray, memoryview, Any],
    dst_buffer: bytearray,
    format: str = "deflate",
) -> int: ...

def crc32(data: Union[bytes, bytearray, memoryview, Any], seed: int = 0) -> int: ...

def crc64(data: Union[bytes, bytearray, memoryview, Any], seed: int = 0) -> int: ...

def version() -> str: ...

def is_hardware_accelerated() -> bool: ...

def benchmark_matrix(
    corpus_type: str = "synthetic_json",
    corpus_size: int = 65536,
    iterations: int = 1,
) -> BenchmarkMatrixReport: ...

open = open_archive

__all__ = [
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
