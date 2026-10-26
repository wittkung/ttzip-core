// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

#ifndef TTZIP_LLAMA_BRIDGE_H
#define TTZIP_LLAMA_BRIDGE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// ============================================================================
// Opaque Bridge Session Handle & Upstream Header Layout Snapshot
//
// Architectural Rule:
// This header intentionally contains ZERO hand-written copies of upstream
// `llama.cpp` structs (`llama_model_params`, `llama_context_params`, `llama_batch`).
// `ttzip_llama_bridge.c` directly `#include "llama.h"` provided by Owner H and
// produces all `_Static_assert`s and `TtzipLlamaAbiLayoutSnapshot` values from
// the real upstream header declarations.
// ============================================================================

typedef struct TtzipLlamaBridgeSession TtzipLlamaBridgeSession;

struct TtzipLlamaAbiLayoutSnapshot {
    bool bound_to_upstream_llama_header;

    size_t batch_size;
    size_t batch_align;
    size_t batch_offset_n_tokens;
    size_t batch_offset_token;
    size_t batch_offset_logits;

    size_t model_params_size;
    size_t model_params_align;
    size_t model_params_offset_n_gpu_layers;
    size_t model_params_offset_load_mode;
    size_t model_params_offset_lazy_mode;
    size_t model_params_offset_main_gpu;
    size_t model_params_offset_tensor_split;
    size_t model_params_offset_kv_overrides;
    size_t model_params_offset_vocab_only;
    size_t model_params_offset_load_mtp;

    size_t context_params_size;
    size_t context_params_align;
    size_t context_params_offset_n_outputs_max;
    size_t context_params_offset_n_outputs_max_per_seq;
    size_t context_params_offset_n_threads;
    size_t context_params_offset_pooling_type;
    size_t context_params_offset_attention_type;
    size_t context_params_offset_embeddings;
    size_t context_params_offset_kv_unified;
    size_t context_params_offset_samplers;
    size_t context_params_offset_n_samplers;
    size_t context_params_offset_ctx_other;
};

void ttzip_llama_get_compiled_abi_layout(struct TtzipLlamaAbiLayoutSnapshot * out_snapshot);

TtzipLlamaBridgeSession * ttzip_llama_session_create(
    const char * dylib_path,
    const char * model_path,
    bool prefer_metal,
    uint32_t n_ctx,
    int32_t n_threads,
    int32_t * out_backend_kind,
    char * out_device_name,
    size_t device_name_cap,
    int32_t * out_n_embd,
    char * out_err,
    size_t err_cap
);

int32_t ttzip_llama_session_encode(
    TtzipLlamaBridgeSession * session,
    const char * prompt,
    int32_t max_tokens,
    float * out_embd,
    size_t embd_cap,
    int32_t * out_tokens,
    size_t tokens_cap,
    int32_t * out_token_count,
    char * out_err,
    size_t err_cap
);

void ttzip_llama_session_destroy(TtzipLlamaBridgeSession * session);

uint64_t ttzip_llama_current_phys_footprint_bytes(void);

#ifdef __cplusplus
}
#endif

#endif // TTZIP_LLAMA_BRIDGE_H

