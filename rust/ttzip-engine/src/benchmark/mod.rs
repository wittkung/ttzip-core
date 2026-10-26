// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Benchmarking, MIPS rating, monotonic timing, Pareto frontier, and Matrix Gate suite.

pub mod clock;
pub mod codecs_driver;
pub mod container_driver;
pub mod corpus;
pub mod corpus_provider;
pub mod crypto_driver;
pub mod delta;
pub mod mips;
pub mod multimodal_loader;
pub mod pareto;
pub mod plotter;
pub mod runner;
pub mod scenario_driver;
pub mod silesia_corpus;
pub mod spline;
pub mod stats;
pub mod thermal;
pub mod timing;
pub mod zip64_virtual_reader;

pub mod ab_engine {
    pub use super::corpus_provider;
    pub use super::silesia_corpus;
    pub use super::stats;
    pub use super::thermal;
    pub use super::timing;
    pub use super::zip64_virtual_reader;
}

#[cfg(test)]
mod tests;

pub use corpus_provider::{
    BenchmarkCorpusProvider, CorpusRegistry, RealWorldAssetKind, RealWorldCorpusProvider,
    SyntheticCorpusProvider,
};
pub use silesia_corpus::SilesiaCorpusEngine;
pub use stats::{sync_to_next_tick, HampelFilter, HampelFilterResult};
pub use thermal::ThermalThrottleGovernor;
pub use timing::wait_for_next_tick;
pub use zip64_virtual_reader::{Zip64ArchiveBuilder, Zip64HeaderInspector};
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

