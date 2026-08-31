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

pub mod brotli_dict;
pub mod corpus_provider;
pub mod decodecorpus;
pub mod fast_lzma2_pool;
pub mod fuzz_tail;
pub mod guarded_buffer;
pub mod header_quota_guard;
pub mod huffman_defense;
pub mod lzfse_tunables;
pub mod micro_chunk;
pub mod orchestrator;
pub mod paramgrill;
pub mod reporters;
pub mod secure_symlink;
pub mod self_verifying_bundle;
pub mod silesia_corpus;
pub mod snappy_corpora;
pub mod stats;
pub mod target;
pub mod thermal;
pub mod timed_fn;
pub mod timeloop;
pub mod timing;
pub mod varint_pax_guard;
pub mod xz_adversarial;
pub mod zip64_virtual_reader;

#[cfg(test)]
mod tests;

pub use brotli_dict::{
    BrotliDictDecisionVerdict, BrotliDictDomain, BrotliDictEvaluationReport, BrotliDictPolicy,
    BrotliDictRecommendation, BrotliDictionary, BrotliDictionaryEvaluator,
};
pub use corpus_provider::{
    BenchmarkCorpusProvider, CorpusRegistry, CustomFileCorpusProvider, RealWorldAssetKind,
    RealWorldCorpusProvider, SyntheticCorpusProvider,
};
pub use self_verifying_bundle::{
    BundleAuditReport, BundleCompressionCodec, BundleEntry, BundleEntryAudit,
    BundleHashAlgorithm, SelfVerifyingBundleEngine, StreamingBundleHasher, BUNDLE_MAGIC,
    BUNDLE_VERSION_1, DEFAULT_STREAM_CHUNK_SIZE,
};
pub use silesia_corpus::{
    SilesiaCorpusEngine, SilesiaCorpusKind, SilesiaCorpusProvider, SilesiaFileDescriptor,
    SilesiaValidationError, SilesiaValidationReport, SILESIA_DESCRIPTORS,
    SILESIA_ENTITIES_COUNT, SILESIA_TOTAL_STANDARD_BYTES,
};
pub use snappy_corpora::{SnappyCorpusKind, SnappyIndustrialCorpusProvider};
pub use decodecorpus::{
    zstd_xxh64_digest32, DeterministicRng, ReverseBlockType, ReverseFrameConfig,
    ReverseFrameOutput, ZstdReverseFrameGenerator,
};
pub use fast_lzma2_pool::{
    run_pool_concurrency_stress_test, DoublePingPongBuffer, FastLzma2BufferPool,
    FastLzma2ChunkLease, PoolStressReport, ALIGNMENT_MASK as FL2_ALIGNMENT_MASK,
    ALIGNMENT_SIZE as FL2_ALIGNMENT_SIZE,
};
pub use fuzz_tail::FuzzTailDataProducer;
pub use guarded_buffer::{system_page_size, GuardedBuffer, GuardedBufferError};
pub use crate::memory::{BumpWorkspace, WorkspaceError, CACHE_LINE_ALIGNMENT};
pub use header_quota_guard::{
    validate_header_entry_count, HeaderQuotaGuard, HeaderSecurityError,
    DEFAULT_ESTIMATED_BYTES_PER_ENTRY, DEFAULT_MAX_ENTRIES_PER_HEADER_BYTE,
    DEFAULT_MAX_HEADER_MEMORY_BYTES,
};
pub use huffman_defense::{
    append_empty_dynamic_huffman_block, append_empty_static_huffman_block,
    generate_empty_dynamic_huffman_blocks, generate_empty_dynamic_huffman_stream_by_size,
    generate_empty_static_huffman_blocks, DeflateBitWriter, HuffmanComplexityGuard,
    HuffmanDefenseAuditSummary, HuffmanDefenseReport, HuffmanDefenseStatus, HuffmanDosDefense,
};
pub use lzfse_tunables::{
    LzfseProfile, LzfseRoutingDecision, LzfseTunablesConfig, LzfseTunablesEngine,
    LzfseTunablesReport, APPLE_SILICON_CACHE_LINE_BYTES, APPLE_SILICON_P_CORE_L1D_BYTES,
    DEFAULT_LZFSE_GOOD_MATCH, DEFAULT_LZFSE_HASH_BITS, DEFAULT_LZFSE_HASH_WIDTH,
    DEFAULT_LZVN_THRESHOLD,
};
pub use micro_chunk::{
    MicroChunkBoundedReader, MicroChunkCodec, MicroChunkCodecReport, MicroChunkPassResult,
    MicroChunkStreamValidator, MICRO_CHUNK_STEPS, STAIRCASE_CHUNK_PATTERN,
};
pub use orchestrator::{
    calc_throughput_mbs, AbBaselineSnapshot, AbBenchmarkReport, AbEngineOrchestrator,
    AbOrchestratorConfig, BaselineSnapshotEntry, TargetAbReportItem,
};
pub use paramgrill::{
    HyperParamVector, ParamEvaluationResult, ParamGrillReport, ParamGrillSearchConstraints,
    ParamGrillSearchEngine, VALID_CHUNK_SIZES,
};
pub use reporters::{AsciiTableReporter, JsonTelemetryReporter, MarkdownCommentReporter};
pub use secure_symlink::{
    ExtractionAuditReport, SecureEntryType, SecurePathExtractor, SecurePathExtractorConfig,
    SecurityError,
};
pub use stats::*;
pub use target::*;
pub use thermal::*;
pub use timed_fn::*;
pub use timeloop::{
    Lz4TimeLoopBenchEngine, TimeLoopConfig, TimeLoopPassResult, TimeLoopStats,
    DEFAULT_WARMUP_LOOPS as TIMELOOP_DEFAULT_WARMUP_LOOPS, NB_TESTS, TIMELOOP_MICROS,
};
pub use timing::*;
pub use varint_pax_guard::{
    decode_7z_varint, decode_7z_varint_from_reader, encode_7z_varint, parse_pax_extended_header,
    parse_pax_timestamp, PaxRecord, PaxSecurityError, PaxTimestamp, VarintPaxSecurityGuard,
    VarintSecurityError, MAX_7Z_VARINT_BYTES, MAX_PAX_RECORDS_PER_BLOCK, MAX_PAX_RECORD_SIZE,
    MAX_SAFE_PAX_SECONDS, MIN_SAFE_PAX_SECONDS,
};
pub use xz_adversarial::{
    parse_vli, validate_xz_stream_thorough, XzAdversarialCategory, XzAdversarialHarness,
    XzAdversarialReport, XzAdversarialVector, XzSecurityError, XzVectorResult, XZ_MAGIC_FOOTER,
    XZ_MAGIC_HEADER, XZ_VLI_BYTES_MAX, XZ_VLI_MAX,
};
pub use zip64_virtual_reader::{
    SegmentData, VirtualSegment, Zip64ArchiveBuilder, Zip64CentralHeaderInfo, Zip64EocdInfo,
    Zip64HeaderInspector, Zip64InspectionError, Zip64LocalHeaderInfo, Zip64VerificationReport,
    Zip64VirtualSparseReader, ZIP64_4GB_THRESHOLD, ZIP64_EOCD_MAGIC, ZIP64_EXTRA_FIELD_TAG,
    ZIP64_LOCATOR_MAGIC, ZIP64_OVERFLOW_16, ZIP64_OVERFLOW_32, ZIP_CDH_MAGIC, ZIP_EOCD_MAGIC,
    ZIP_LFH_MAGIC,
};
