# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine for Python.

from dataclasses import dataclass
from typing import List

try:
    from ._ttzip import (
        PyEntryMetadata as EntryMetadata,
        PyBenchmarkPointResult as BenchmarkPointResult,
        PyBenchmarkMatrixReport as BenchmarkMatrixReport,
    )
except ImportError:
    @dataclass(frozen=True)
    class EntryMetadata:
        path: str
        uncompressed_size: int
        compressed_size: int
        crc32: int
        mtime_epoch_secs: int
        is_directory: bool
        is_encrypted: bool

    @dataclass(frozen=True)
    class BenchmarkPointResult:
        algorithm: str
        level: int
        display_name: str
        original_size_bytes: int
        compressed_size_bytes: int
        space_savings_pct: float
        compress_throughput_mbs: float
        decompress_throughput_mbs: float
        is_pareto_optimal: bool

    @dataclass(frozen=True)
    class BenchmarkMatrixReport:
        total_points_evaluated: int
        pareto_optimal_count: int
        peak_compress_throughput_mbs: float
        peak_decompress_throughput_mbs: float
        max_space_savings_pct: float
        points: List[BenchmarkPointResult]
        passed_gate: bool

@dataclass(frozen=True)
class ProgressInfo:
    processed_bytes: int
    total_bytes: int
    current_entry: str
    fraction_completed: float

__all__ = [
    "EntryMetadata",
    "BenchmarkPointResult",
    "BenchmarkMatrixReport",
    "ProgressInfo",
]
