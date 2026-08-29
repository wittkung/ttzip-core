// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Benchmarking, MIPS rating, monotonic timing, Pareto frontier, and Matrix Gate suite.

pub mod ab_engine;
pub mod clock;
pub mod codecs_driver;
pub mod container_driver;
pub mod corpus;
pub mod crypto_driver;
pub mod delta;
pub mod mips;
pub mod multimodal_loader;
pub mod pareto;
pub mod plotter;
pub mod runner;
pub mod scenario_driver;
pub mod spline;

#[cfg(test)]
mod tests;


pub use ab_engine::{
    sync_to_next_tick, wait_for_next_tick, BenchmarkCorpusProvider, BenchmarkTarget, CodecMode,
    CodecTargetAdapter, ComparisonStats, ConfidenceInterval, ContainerMode,
    ContainerTargetAdapter, CorpusRegistry, CryptoMode, CryptoTargetAdapter,
    CustomFileCorpusProvider, DecisionVerdict, HampelFilter, HampelFilterResult,
    Lz4TimeLoopBenchEngine, MeasurementConfig, MeasurementEngine, MeasurementStats, MetricUnit,
    RealWorldAssetKind, RealWorldCorpusProvider, SyntheticCorpusProvider, TargetCategory,
    TargetDescriptor, TargetExecutionOutput, TargetRegistry, TimeLoopConfig,
    TimeLoopPassResult, TimeLoopStats, WelchStudentTTest, WelchTTestResult, NB_TESTS,
    TIMELOOP_MICROS,
};
pub use clock::MonotonicStopwatch;
pub use codecs_driver::{
    BrotliBenchmarkDriver, Bzip2BenchmarkDriver, CodecBenchmarkDriver, DeflateBenchmarkDriver,
    Lz4BenchmarkDriver, LzfseBenchmarkDriver, Lzma2BenchmarkDriver, MatrixCodecConfig,
    MatrixCodecDriver, SnappyBenchmarkDriver, ZstdBenchmarkDriver,
};
pub use container_driver::{
    AarContainerDriver, ContainerBenchmarkDriver, SevenZContainerDriver, TarBrotliContainerDriver,
    TarContainerDriver, TarGzContainerDriver, TarSnappyContainerDriver, TarZstContainerDriver,
    ZipContainerDriver,
};
pub use corpus::{BenchmarkCorpusGenerator, BenchmarkCorpusType};
pub use crypto_driver::{
    Adler32BenchmarkDriver, Blake3BenchmarkDriver, Crc32BenchmarkDriver, Crc64BenchmarkDriver,
    CryptoBenchmarkDriver, CryptoBenchmarkMatrixReport, CryptoBenchmarkPointResult, CryptoCategory,
    MatrixCryptoDriver, SevenZAes256BenchmarkDriver, VaultAesGcmBenchmarkDriver,
    VaultChaChaPolyBenchmarkDriver, WinZipAes256BenchmarkDriver, Xxh3_128BenchmarkDriver,
    Xxh3_64BenchmarkDriver, ZipCryptoBenchmarkDriver,
};
pub use delta::{BinaryDeltaAuditor, BinaryDeltaReport, SegmentDeltaAudit};

pub use mips::{MIPSHardwareBenchmarkEngine, MIPSResult, SplitMix64};
pub use multimodal_loader::{
    compute_shannon_entropy, MultimodalCorpusEntry, MultimodalCorpusKind, MultimodalCorpusLoader,
    SILESIA_STANDARD_FILES,
};
pub use pareto::{
    calculate_pareto_frontier, compute_codec_pareto_frontier_raw, compute_pareto_frontier_raw,
    ParetoCodecPoint, ParetoPointRaw, TTZipParetoCodecPointRaw,
};
pub use plotter::BenchmarkPlotter;
pub use runner::{BenchmarkMatrixReport, BenchmarkMatrixRunner, BenchmarkPointResult};
pub use scenario_driver::{ScenarioBenchmarkDriver, ScenarioBenchmarkPoint, ScenarioMatrixReport};
pub use spline::{FritschCarlsonSpline, SplinePoint};

