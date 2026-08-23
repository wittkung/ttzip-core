# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
from typing import List, Optional, Union
from pathlib import Path
from .models import EntryMetadata, ProgressInfo
from .exceptions import (
    TTZipError,
    AuthenticationError,
    CorruptArchiveError,
    SecurityError,
)

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
    data: bytes,
    format: str = "deflate",
) -> bytes: ...

def compress_buffer(
    data: bytes,
    format: str = "deflate",
    level: int = 6,
) -> bytes: ...

def crc32(data: bytes, seed: int = 0) -> int: ...

def crc64(data: bytes, seed: int = 0) -> int: ...

def version() -> str: ...

def is_hardware_accelerated() -> bool: ...

def benchmark_matrix(
    corpus_type: str = "synthetic_json",
    corpus_size: int = 65536,
    iterations: int = 1,
) -> BenchmarkMatrixReport: ...

__all__ = [
    "compress",
    "extract",
    "inspect",
    "decompress_buffer",
    "compress_buffer",
    "crc32",
    "crc64",
    "version",
    "is_hardware_accelerated",
    "benchmark_matrix",
    "EntryMetadata",
    "BenchmarkPointResult",
    "BenchmarkMatrixReport",
    "ProgressInfo",
    "TTZipError",
    "AuthenticationError",
    "CorruptArchiveError",
    "SecurityError",
]
