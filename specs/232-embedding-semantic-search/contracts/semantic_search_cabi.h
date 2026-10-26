// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

#ifndef TTZIP_SEMANTIC_SEARCH_CABI_H
#define TTZIP_SEMANTIC_SEARCH_CABI_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Legacy section 9 is reserved for TT_SECTION_TYPE_SUBTREE_END_IDS.
 * Embedding sections v2 use section ID 10 (TT_SECTION_TYPE_EMBEDDINGS_V2).
 */
#define TTZIP_SECTION_TYPE_SUBTREE_END_IDS 9
#define TTZIP_SECTION_TYPE_EMBEDDINGS_V2   10
#define TTZIP_EMBEDDING_MAGIC              0x54545645U /* "TTVE" LE */
#define TTZIP_EMBEDDING_SCHEMA_VERSION_2   2U
#define TTZIP_EMBEDDING_DIM_128            128U
#define TTZIP_MODEL_FINGERPRINT_V1         0x914F7F80B1146101ULL

#pragma pack(push, 8)

typedef struct ttzip_embedding_header_v2_s {
    uint32_t magic;                 // 0x54545645 ("TTVE")
    uint32_t version;               // 2
    uint32_t file_count;            // Total indexed files / VFS nodes
    uint32_t segment_count;         // Total indexed segments (path + content chunks)
    uint16_t dimension;             // 128
    uint8_t  quantization;          // 1 = INT8
    uint8_t  flags;                 // Bit 0: Tier1Complete, Bit 1: Tier2Complete
    uint32_t _reserved;             // 0
    uint64_t model_fingerprint;     // 0x914F7F80B1146101ULL
    uint8_t  scope_blake3[32];      // BLAKE3 hash of scope/archive identity
    uint8_t  payload_blake3[32];    // BLAKE3 checksum of payload bytes [96..End]
} ttzip_embedding_header_v2_t;

#pragma pack(pop)

/**
 * Minimal C-ABI surface of llama.cpp (tag b11461 + llama.cpp-no-logits.patch)
 * encapsulated by Rust `crates/ttzip-semantic/src/llama_ffi.rs`.
 */
typedef struct llama_model   llama_model;
typedef struct llama_context llama_context;
typedef struct llama_vocab   llama_vocab;
typedef int32_t              llama_token;
typedef int32_t              llama_pos;
typedef int32_t              llama_seq_id;

#ifdef __cplusplus
}
#endif

#endif // TTZIP_SEMANTIC_SEARCH_CABI_H
