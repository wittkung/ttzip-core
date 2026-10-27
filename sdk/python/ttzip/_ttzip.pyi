# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.
# PyO3 native Python C-extension type stubs (_ttzip).

from typing import Any, List, Optional, Union

class PyEntryMetadata:
    path: str
    uncompressed_size: int
    compressed_size: int
    crc32: int
    mtime_epoch_secs: int
    is_directory: bool
    is_encrypted: bool

    def __init__(
        self,
        path: str = ...,
        uncompressed_size: int = ...,
        compressed_size: int = ...,
        crc32: int = ...,
        mtime_epoch_secs: int = ...,
        is_directory: bool = ...,
        is_encrypted: bool = ...,
    ) -> None: ...
    def __repr__(self) -> str: ...

class PyBenchmarkPointResult:
    algorithm: str
    level: int
    display_name: str
    original_size_bytes: int
    compressed_size_bytes: int
    space_savings_pct: float
    compress_throughput_mbs: float
    decompress_throughput_mbs: float
    is_pareto_optimal: bool

    def __repr__(self) -> str: ...

class PyBenchmarkMatrixReport:
    total_points_evaluated: int
    pareto_optimal_count: int
    peak_compress_throughput_mbs: float
    peak_decompress_throughput_mbs: float
    max_space_savings_pct: float
    points: List[PyBenchmarkPointResult]
    passed_gate: bool

    def __repr__(self) -> str: ...

class TTZipError(Exception): ...
class AuthenticationError(TTZipError): ...
class CorruptArchiveError(TTZipError): ...
class SecurityError(TTZipError): ...

def compress(
    sources: List[str],
    destination: str,
    format: str = "auto",
    level: int = 6,
    password: Optional[str] = None,
    threads: int = 0,
) -> None: ...

def extract(
    archive: str,
    destination: str,
    password: Optional[str] = None,
    threads: int = 0,
) -> None: ...

def inspect(
    archive: str,
    password: Optional[str] = None,
) -> List[PyEntryMetadata]: ...

def compress_buffer(
    data: Union[bytes, bytearray, memoryview, Any],
    format: str = "deflate",
    level: int = 6,
) -> bytes: ...

def decompress_buffer(
    data: Union[bytes, bytearray, memoryview, Any],
    format: str = "deflate",
) -> bytes: ...

def decompress_into(
    data: Union[bytes, bytearray, memoryview, Any],
    dst_buffer: bytearray,
    format: str = "deflate",
) -> int: ...

def crc32(
    data: Union[bytes, bytearray, memoryview, Any],
    seed: int = 0,
) -> int: ...

def crc64(
    data: Union[bytes, bytearray, memoryview, Any],
    seed: int = 0,
) -> int: ...

def version() -> str: ...

def is_hardware_accelerated() -> bool: ...

def benchmark_matrix(
    corpus_type: str = "synthetic_json",
    corpus_size: int = 65536,
    iterations: int = 1,
) -> PyBenchmarkMatrixReport: ...
