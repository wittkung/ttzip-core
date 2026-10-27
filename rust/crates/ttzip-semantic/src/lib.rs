// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! TTZip Semantic Search & Indexing Engine (`ttzip-semantic`).
//!
//! Implements the EmbeddingGemma 2 GGUF (`embeddinggemma-2-Q8_0.gguf`) + `llama.cpp`
//! (`b11461` + `llama.cpp-no-logits.patch`) semantic search architecture:
//! - [`model`]: Pinned GGUF checksum verification, prompt templates, and MRL-128 INT8 math.
//! - [`llama_abi`]: Pinned `b11461` C-ABI layout/offset verifier, opaque C bridge, and `BundledOnly` dylib policy.
//! - [`llama_ffi`]: Encapsulated `llama.cpp` C-ABI runtime loader (Metal primary, CPU fallback).
//! - [`chunker`]: Authorized scope path filtering and UTF-8 text/code chunking (`1,800` chars / `200` overlap).
//! - [`index`]: Versioned `.ttvec` v2 persistent segment index with corruption & upgrade recovery.
//! - [`scoring`]: Deterministic concept embedding provider and hybrid lexical + semantic segment scoring.
//! - [`engine`]: Request-ID isolated hybrid search coordinator and incremental indexer.

pub mod chunker;
pub mod engine;
pub mod index;
pub mod llama_abi;
pub mod llama_ffi;
pub mod model;
pub mod scoring;

pub use chunker::{
    chunk_text_content, classify_content_tier, is_excluded_relative_path, ChunkSegment,
    ContentSupportTier,
};
pub use engine::{
    SemanticFileEvent, SemanticIndexProgress, SemanticIngestPhase, SemanticMatchKind,
    SemanticOpenRequest, SemanticScopeDescriptor, SemanticScopeKind, SemanticSearchCoordinator,
    SemanticSearchHit, SemanticSearchRequest, SemanticSearchResponse,
};
pub use index::{
    IndexHeaderV2, IndexLoadOutcome, IndexedFileRecord, IndexedSegmentRecord, PersistentScopeIndex,
    TT_EMBEDDING_MAGIC, TT_EMBEDDING_SCHEMA_VERSION_2, TT_SECTION_TYPE_EMBEDDINGS_V2,
    TT_SECTION_TYPE_SUBTREE_END_IDS,
};
pub use llama_abi::{
    bundled_production_dylib_candidates, current_phys_footprint_bytes,
    expected_pinned_b11461_layout_snapshot, is_permitted_production_dylib_candidate,
    validate_abi_layout_snapshot, verify_pinned_b11461_abi_layout, AbiHeaderStatus,
    LlamaDylibLoadPolicy, TtzipLlamaAbiLayoutSnapshot, BUNDLED_PRODUCTION_DYLIB_CANDIDATES,
};
pub use llama_ffi::{
    CheckpointStatus, LlamaRuntimeConfig, LlamaRuntimeEngine, RealModelCheckpointReport,
};
pub use model::{
    cosine_similarity_f32, dequantize_int8_to_f32, format_code_query_prompt,
    format_document_prompt, format_query_prompt, quantize_mrl128_int8, strip_code_query_prefix,
    validate_non_zero_int8_vector, verify_gguf_model_file, verify_reference_vector_parity,
    DigUpReferenceFile, ReferenceVectorEntry, ReferenceVectorParityReport, SemanticComputeBackend,
    SemanticModelError, SemanticModelInfo, SemanticModelState, PINNED_GGUF_FILENAME,
    PINNED_GGUF_REPO, PINNED_GGUF_REVISION, PINNED_GGUF_SHA256, PINNED_GGUF_SIZE_BYTES,
    PINNED_LLAMA_CPP_COMMIT, PINNED_LLAMA_CPP_NO_LOGITS_PATCH_SHA256, PINNED_LLAMA_CPP_TAG,
    PINNED_MODEL_FINGERPRINT, PINNED_MODEL_ID, PINNED_MODEL_REVISION, TEST_DOUBLE_MODEL_FINGERPRINT,
};
pub use scoring::{DeterministicEmbeddingProvider, EmbeddingProvider};
