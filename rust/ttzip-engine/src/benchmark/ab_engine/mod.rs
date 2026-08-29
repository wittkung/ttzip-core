// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TTZip Declarative A/B Benchmarking & Evaluation Engine.
//!
//! Provides modular architecture layers:
//! - Layer 1: Target Registry & Driver Adapters (`target`)
//! - Layer 2: Corpus Providers & Unified Registry (`corpus_provider`)
//! - Layer 3: Statistical Analysis & Measurement Kernel (`stats`)
//! - Layer 4: Scheduler & Orchestrator (`orchestrator`)
//! - Layer 5: Multimodal Report Exporters (`reporters`)

pub mod corpus_provider;
pub mod orchestrator;
pub mod reporters;
pub mod stats;
pub mod target;

#[cfg(test)]
mod tests;

pub use corpus_provider::{
    BenchmarkCorpusProvider, CorpusRegistry, CustomFileCorpusProvider, RealWorldAssetKind,
    RealWorldCorpusProvider, SyntheticCorpusProvider,
};
pub use orchestrator::{
    calc_throughput_mbs, AbBaselineSnapshot, AbBenchmarkReport, AbEngineOrchestrator,
    AbOrchestratorConfig, BaselineSnapshotEntry, TargetAbReportItem,
};
pub use reporters::{AsciiTableReporter, JsonTelemetryReporter, MarkdownCommentReporter};
pub use stats::*;
pub use target::*;
