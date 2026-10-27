// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

#include "ttzip_llama_bridge.h"

#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#if defined(__APPLE__)
#include <mach/mach.h>
#endif

#if defined(TTZIP_HAS_UPSTREAM_LLAMA_HEADER)
#  define TTZIP_UPSTREAM_LLAMA_HEADER_AVAILABLE 1
#elif defined(__has_include)
#  if __has_include("llama.h")
#    define TTZIP_UPSTREAM_LLAMA_HEADER_AVAILABLE 1
#  else
#    define TTZIP_UPSTREAM_LLAMA_HEADER_AVAILABLE 0
#  endif
#else
#  define TTZIP_UPSTREAM_LLAMA_HEADER_AVAILABLE 0
#endif

static void write_err(char * buf, size_t cap, const char * msg) {
    if (buf != NULL && cap > 0) {
        snprintf(buf, cap, "%s", msg != NULL ? msg : "unknown error");
    }
}

#if TTZIP_UPSTREAM_LLAMA_HEADER_AVAILABLE

#include "llama.h"
#if defined(__has_include) && __has_include("ggml-backend.h")
#  include "ggml-backend.h"
#endif

#if defined(__LP64__) || defined(_LP64)
_Static_assert(sizeof(struct llama_batch) == 56, "b11461 llama_batch sizeof mismatch");
_Static_assert(_Alignof(struct llama_batch) == 8, "b11461 llama_batch alignof mismatch");
_Static_assert(offsetof(struct llama_batch, n_tokens) == 0, "llama_batch.n_tokens offset");
_Static_assert(offsetof(struct llama_batch, token) == 8, "llama_batch.token offset");
_Static_assert(offsetof(struct llama_batch, logits) == 48, "llama_batch.logits offset");

_Static_assert(sizeof(struct llama_model_params) == 80, "b11461 llama_model_params sizeof mismatch");
_Static_assert(_Alignof(struct llama_model_params) == 8, "b11461 llama_model_params alignof mismatch");
_Static_assert(offsetof(struct llama_model_params, n_gpu_layers) == 16, "n_gpu_layers offset");
_Static_assert(offsetof(struct llama_model_params, load_mode) == 24, "load_mode offset");
_Static_assert(offsetof(struct llama_model_params, lazy_mode) == 28, "lazy_mode offset");
_Static_assert(offsetof(struct llama_model_params, main_gpu) == 32, "main_gpu offset");
_Static_assert(offsetof(struct llama_model_params, tensor_split) == 40, "tensor_split offset");
_Static_assert(offsetof(struct llama_model_params, kv_overrides) == 64, "kv_overrides offset");
_Static_assert(offsetof(struct llama_model_params, vocab_only) == 72, "vocab_only offset");
_Static_assert(offsetof(struct llama_model_params, load_mtp) == 77, "load_mtp offset");

_Static_assert(sizeof(struct llama_context_params) == 160, "b11461 llama_context_params sizeof mismatch");
_Static_assert(_Alignof(struct llama_context_params) == 8, "b11461 llama_context_params alignof mismatch");
_Static_assert(offsetof(struct llama_context_params, n_outputs_max) == 20, "n_outputs_max offset");
_Static_assert(offsetof(struct llama_context_params, n_outputs_max_per_seq) == 24, "n_outputs_max_per_seq offset");
_Static_assert(offsetof(struct llama_context_params, n_threads) == 28, "n_threads offset");
_Static_assert(offsetof(struct llama_context_params, pooling_type) == 44, "pooling_type offset");
_Static_assert(offsetof(struct llama_context_params, attention_type) == 48, "attention_type offset");
_Static_assert(offsetof(struct llama_context_params, embeddings) == 128, "embeddings offset");
_Static_assert(offsetof(struct llama_context_params, kv_unified) == 133, "kv_unified offset");
_Static_assert(offsetof(struct llama_context_params, samplers) == 136, "samplers offset");
_Static_assert(offsetof(struct llama_context_params, n_samplers) == 144, "n_samplers offset");
_Static_assert(offsetof(struct llama_context_params, ctx_other) == 152, "ctx_other offset");
#endif

typedef void (*fn_backend_init_t)(void);
typedef bool (*fn_supports_gpu_t)(void);
typedef size_t (*fn_dev_count_t)(void);
typedef void * (*fn_dev_get_t)(size_t);
typedef int32_t (*fn_dev_type_t)(void *);
typedef const char * (*fn_dev_name_t)(void *);
typedef struct llama_model_params (*fn_model_default_params_t)(void);
typedef struct llama_model * (*fn_model_load_t)(const char *, struct llama_model_params);
typedef void (*fn_model_free_t)(struct llama_model *);
typedef struct llama_context_params (*fn_ctx_default_params_t)(void);
typedef struct llama_context * (*fn_init_from_model_t)(struct llama_model *, struct llama_context_params);
typedef void (*fn_free_ctx_t)(struct llama_context *);
typedef void (*fn_set_embeddings_t)(struct llama_context *, bool);
typedef void (*fn_set_causal_attn_t)(struct llama_context *, bool);
typedef int32_t (*fn_model_n_embd_out_t)(const struct llama_model *);
typedef const struct llama_vocab * (*fn_model_get_vocab_t)(const struct llama_model *);
typedef int32_t (*fn_vocab_eos_t)(const struct llama_vocab *);
typedef int32_t (*fn_tokenize_t)(const struct llama_vocab *, const char *, int32_t, llama_token *, int32_t, bool, bool);
typedef struct llama_batch (*fn_batch_init_t)(int32_t, int32_t, int32_t);
typedef void (*fn_batch_free_t)(struct llama_batch);
typedef void * (*fn_get_memory_t)(struct llama_context *);
typedef void (*fn_memory_clear_t)(void *, bool);
typedef int32_t (*fn_encode_t)(struct llama_context *, struct llama_batch);
typedef int32_t (*fn_decode_t)(struct llama_context *, struct llama_batch);
typedef const float * (*fn_get_embeddings_seq_t)(struct llama_context *, llama_seq_id);
typedef const float * (*fn_get_embeddings_ith_t)(struct llama_context *, int32_t);

struct TtzipLlamaBridgeSession {
    void * dl_handle;
    struct llama_model * model;
    struct llama_context * ctx;
    const struct llama_vocab * vocab;
    int32_t n_embd_out;
    fn_model_free_t model_free;
    fn_free_ctx_t free_ctx;
    fn_vocab_eos_t vocab_eos;
    fn_tokenize_t tokenize;
    fn_batch_init_t batch_init;
    fn_batch_free_t batch_free;
    fn_get_memory_t get_memory;
    fn_memory_clear_t memory_clear;
    fn_encode_t encode;
    fn_decode_t decode;
    fn_get_embeddings_seq_t get_embeddings_seq;
    fn_get_embeddings_ith_t get_embeddings_ith;
};

void ttzip_llama_get_compiled_abi_layout(struct TtzipLlamaAbiLayoutSnapshot * out) {
    if (out == NULL) {
        return;
    }
    out->bound_to_upstream_llama_header = true;
    out->batch_size = sizeof(struct llama_batch);
    out->batch_align = _Alignof(struct llama_batch);
    out->batch_offset_n_tokens = offsetof(struct llama_batch, n_tokens);
    out->batch_offset_token = offsetof(struct llama_batch, token);
    out->batch_offset_logits = offsetof(struct llama_batch, logits);

    out->model_params_size = sizeof(struct llama_model_params);
    out->model_params_align = _Alignof(struct llama_model_params);
    out->model_params_offset_n_gpu_layers = offsetof(struct llama_model_params, n_gpu_layers);
    out->model_params_offset_load_mode = offsetof(struct llama_model_params, load_mode);
    out->model_params_offset_lazy_mode = offsetof(struct llama_model_params, lazy_mode);
    out->model_params_offset_main_gpu = offsetof(struct llama_model_params, main_gpu);
    out->model_params_offset_tensor_split = offsetof(struct llama_model_params, tensor_split);
    out->model_params_offset_kv_overrides = offsetof(struct llama_model_params, kv_overrides);
    out->model_params_offset_vocab_only = offsetof(struct llama_model_params, vocab_only);
    out->model_params_offset_load_mtp = offsetof(struct llama_model_params, load_mtp);

    out->context_params_size = sizeof(struct llama_context_params);
    out->context_params_align = _Alignof(struct llama_context_params);
    out->context_params_offset_n_outputs_max = offsetof(struct llama_context_params, n_outputs_max);
    out->context_params_offset_n_outputs_max_per_seq =
        offsetof(struct llama_context_params, n_outputs_max_per_seq);
    out->context_params_offset_n_threads = offsetof(struct llama_context_params, n_threads);
    out->context_params_offset_pooling_type = offsetof(struct llama_context_params, pooling_type);
    out->context_params_offset_attention_type = offsetof(struct llama_context_params, attention_type);
    out->context_params_offset_embeddings = offsetof(struct llama_context_params, embeddings);
    out->context_params_offset_kv_unified = offsetof(struct llama_context_params, kv_unified);
    out->context_params_offset_samplers = offsetof(struct llama_context_params, samplers);
    out->context_params_offset_n_samplers = offsetof(struct llama_context_params, n_samplers);
    out->context_params_offset_ctx_other = offsetof(struct llama_context_params, ctx_other);
}

static void * req_sym(void * handle, const char * name, char * err, size_t err_cap) {
    void * p = dlsym(handle, name);
    if (p == NULL) {
        p = dlsym(RTLD_DEFAULT, name);
    }
    if (p == NULL && err != NULL && err_cap > 0) {
        snprintf(err, err_cap, "missing C-ABI symbol %s", name);
    }
    return p;
}

static void probe_backend_device(
    void * handle,
    bool prefer_metal,
    int32_t * out_backend_kind,
    char * out_device_name,
    size_t device_name_cap,
    int32_t * out_n_gpu_layers
) {
    *out_backend_kind = 0;
    *out_n_gpu_layers = 0;
    snprintf(out_device_name, device_name_cap, "CPU");
    if (!prefer_metal) {
        return;
    }
    fn_supports_gpu_t sup = (fn_supports_gpu_t)dlsym(handle, "llama_supports_gpu_offload");
    fn_dev_count_t cnt = (fn_dev_count_t)dlsym(handle, "ggml_backend_dev_count");
    fn_dev_get_t get = (fn_dev_get_t)dlsym(handle, "ggml_backend_dev_get");
    fn_dev_type_t typ = (fn_dev_type_t)dlsym(handle, "ggml_backend_dev_type");
    fn_dev_name_t nam = (fn_dev_name_t)dlsym(handle, "ggml_backend_dev_name");
    if (sup != NULL && sup() && cnt != NULL && get != NULL && typ != NULL && nam != NULL) {
        size_t n = cnt();
        for (size_t i = 0; i < n; ++i) {
            void * dev = get(i);
            if (dev != NULL) {
                int32_t dt = typ(dev);
                if (dt == 1 || dt == 2) {
                    const char * nm = nam(dev);
                    *out_backend_kind = 1;
                    *out_n_gpu_layers = 99;
                    snprintf(out_device_name, device_name_cap, "%s", nm != NULL ? nm : "Metal");
                    return;
                }
            }
        }
    }
}

static bool resolve_session_symbols(
    TtzipLlamaBridgeSession * s,
    fn_backend_init_t * out_backend_init,
    fn_model_default_params_t * out_mdef,
    fn_model_load_t * out_mload,
    fn_ctx_default_params_t * out_cdef,
    fn_init_from_model_t * out_cinit,
    fn_model_n_embd_out_t * out_n_embd,
    fn_model_get_vocab_t * out_get_vocab,
    char * out_err,
    size_t err_cap
) {
    void * h = s->dl_handle;
    *out_backend_init = (fn_backend_init_t)req_sym(h, "llama_backend_init", out_err, err_cap);
    *out_mdef = (fn_model_default_params_t)req_sym(h, "llama_model_default_params", out_err, err_cap);
    *out_mload = (fn_model_load_t)req_sym(h, "llama_model_load_from_file", out_err, err_cap);
    s->model_free = (fn_model_free_t)req_sym(h, "llama_model_free", out_err, err_cap);
    *out_cdef = (fn_ctx_default_params_t)req_sym(h, "llama_context_default_params", out_err, err_cap);
    *out_cinit = (fn_init_from_model_t)req_sym(h, "llama_init_from_model", out_err, err_cap);
    s->free_ctx = (fn_free_ctx_t)req_sym(h, "llama_free", out_err, err_cap);
    *out_n_embd = (fn_model_n_embd_out_t)req_sym(h, "llama_model_n_embd_out", out_err, err_cap);
    *out_get_vocab = (fn_model_get_vocab_t)req_sym(h, "llama_model_get_vocab", out_err, err_cap);
    s->vocab_eos = (fn_vocab_eos_t)req_sym(h, "llama_vocab_eos", out_err, err_cap);
    s->tokenize = (fn_tokenize_t)req_sym(h, "llama_tokenize", out_err, err_cap);
    s->batch_init = (fn_batch_init_t)req_sym(h, "llama_batch_init", out_err, err_cap);
    s->batch_free = (fn_batch_free_t)req_sym(h, "llama_batch_free", out_err, err_cap);
    s->get_memory = (fn_get_memory_t)dlsym(h, "llama_get_memory");
    s->memory_clear = (fn_memory_clear_t)dlsym(h, "llama_memory_clear");
    s->encode = (fn_encode_t)dlsym(h, "llama_encode");
    s->decode = (fn_decode_t)req_sym(h, "llama_decode", out_err, err_cap);
    s->get_embeddings_seq = (fn_get_embeddings_seq_t)req_sym(h, "llama_get_embeddings_seq", out_err, err_cap);
    s->get_embeddings_ith = (fn_get_embeddings_ith_t)dlsym(h, "llama_get_embeddings_ith");
    return (*out_backend_init && *out_mdef && *out_mload && s->model_free && *out_cdef &&
            *out_cinit && s->free_ctx && *out_n_embd && *out_get_vocab && s->vocab_eos &&
            s->tokenize && s->batch_init && s->batch_free && s->decode && s->get_embeddings_seq);
}

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
) {
    void * h = dlopen(dylib_path, RTLD_NOW | RTLD_LOCAL);
    if (h == NULL) {
        write_err(out_err, err_cap, "bundled libllamacpp.dylib not found or failed to dlopen");
        return NULL;
    }
    TtzipLlamaBridgeSession * s = (TtzipLlamaBridgeSession *)calloc(1, sizeof(TtzipLlamaBridgeSession));
    if (s == NULL) {
        dlclose(h);
        write_err(out_err, err_cap, "calloc failed for TtzipLlamaBridgeSession");
        return NULL;
    }
    s->dl_handle = h;
    fn_backend_init_t backend_init = NULL;
    fn_model_default_params_t mdef = NULL;
    fn_model_load_t mload = NULL;
    fn_ctx_default_params_t cdef = NULL;
    fn_init_from_model_t cinit = NULL;
    fn_model_n_embd_out_t n_embd_fn = NULL;
    fn_model_get_vocab_t get_vocab_fn = NULL;
    if (!resolve_session_symbols(
            s, &backend_init, &mdef, &mload, &cdef, &cinit, &n_embd_fn, &get_vocab_fn, out_err, err_cap)) {
        ttzip_llama_session_destroy(s);
        return NULL;
    }
    backend_init();
    int32_t n_gpu_layers = 0;
    probe_backend_device(h, prefer_metal, out_backend_kind, out_device_name, device_name_cap, &n_gpu_layers);

    struct llama_model_params mparams = mdef();
    mparams.n_gpu_layers = n_gpu_layers;
    s->model = mload(model_path, mparams);
    if (s->model == NULL) {
        write_err(out_err, err_cap, "llama_model_load_from_file returned NULL");
        ttzip_llama_session_destroy(s);
        return NULL;
    }
    struct llama_context_params cparams = cdef();
    cparams.n_ctx = n_ctx;
    cparams.n_batch = n_ctx;
    cparams.n_ubatch = n_ctx;
    cparams.n_threads = n_threads;
    cparams.n_threads_batch = n_threads;
    cparams.pooling_type = LLAMA_POOLING_TYPE_MEAN;
    cparams.attention_type = LLAMA_ATTENTION_TYPE_NON_CAUSAL;
    cparams.embeddings = true;
    s->ctx = cinit(s->model, cparams);
    if (s->ctx == NULL) {
        write_err(out_err, err_cap, "llama_init_from_model returned NULL");
        ttzip_llama_session_destroy(s);
        return NULL;
    }
    fn_set_embeddings_t set_emb = (fn_set_embeddings_t)dlsym(h, "llama_set_embeddings");
    if (set_emb != NULL) {
        set_emb(s->ctx, true);
    }
    fn_set_causal_attn_t set_causal = (fn_set_causal_attn_t)dlsym(h, "llama_set_causal_attn");
    if (set_causal != NULL) {
        set_causal(s->ctx, false);
    }
    s->vocab = get_vocab_fn(s->model);
    s->n_embd_out = n_embd_fn(s->model);
    if (s->vocab == NULL || s->n_embd_out < 128) {
        write_err(out_err, err_cap, "invalid vocab pointer or embedding dimension < 128");
        ttzip_llama_session_destroy(s);
        return NULL;
    }
    *out_n_embd = s->n_embd_out;
    return s;
}

int32_t ttzip_llama_session_encode(
    TtzipLlamaBridgeSession * s,
    const char * prompt,
    int32_t max_tokens,
    float * out_embd,
    size_t embd_cap,
    int32_t * out_tokens,
    size_t tokens_cap,
    int32_t * out_token_count,
    char * out_err,
    size_t err_cap
) {
    if (s == NULL || prompt == NULL || out_embd == NULL || embd_cap < (size_t)s->n_embd_out) {
        write_err(out_err, err_cap, "invalid session or output buffer arguments");
        return -1;
    }
    int32_t text_len = (int32_t)strlen(prompt);
    int32_t count = s->tokenize(s->vocab, prompt, text_len, out_tokens, (int32_t)tokens_cap, true, false);
    if (count <= 0) {
        write_err(out_err, err_cap, "llama_tokenize returned <= 0 or exceeded token buffer");
        return -2;
    }
    if (count > max_tokens && max_tokens >= 2) {
        out_tokens[max_tokens - 1] = s->vocab_eos(s->vocab);
        count = max_tokens;
    }
    *out_token_count = count;
    if (s->get_memory != NULL && s->memory_clear != NULL) {
        void * mem = s->get_memory(s->ctx);
        if (mem != NULL) {
            s->memory_clear(mem, true);
        }
    }
    struct llama_batch batch = s->batch_init(count, 0, 1);
    batch.n_tokens = count;
    for (int32_t i = 0; i < count; ++i) {
        batch.token[i] = out_tokens[i];
        batch.pos[i] = i;
        batch.n_seq_id[i] = 1;
        if (batch.seq_id != NULL && batch.seq_id[i] != NULL) {
            batch.seq_id[i][0] = 0;
        }
        batch.logits[i] = 1;
    }
    int32_t rc = s->encode != NULL ? s->encode(s->ctx, batch) : s->decode(s->ctx, batch);
    s->batch_free(batch);
    if (rc != 0) {
        write_err(out_err, err_cap, "llama_encode/llama_decode returned non-zero status");
        return -3;
    }
    const float * emb = s->get_embeddings_seq(s->ctx, 0);
    if (emb == NULL && s->get_embeddings_ith != NULL) {
        emb = s->get_embeddings_ith(s->ctx, -1);
    }
    if (emb == NULL) {
        write_err(out_err, err_cap, "llama_get_embeddings_seq returned NULL");
        return -4;
    }
    memcpy(out_embd, emb, (size_t)s->n_embd_out * sizeof(float));
    return 0;
}

void ttzip_llama_session_destroy(TtzipLlamaBridgeSession * s) {
    if (s == NULL) {
        return;
    }
    if (s->ctx != NULL && s->free_ctx != NULL) {
        s->free_ctx(s->ctx);
        s->ctx = NULL;
    }
    if (s->model != NULL && s->model_free != NULL) {
        s->model_free(s->model);
        s->model = NULL;
    }
    if (s->dl_handle != NULL) {
        dlclose(s->dl_handle);
        s->dl_handle = NULL;
    }
    free(s);
}

#else // !TTZIP_UPSTREAM_LLAMA_HEADER_AVAILABLE

struct TtzipLlamaBridgeSession {
    int unused;
};

void ttzip_llama_get_compiled_abi_layout(struct TtzipLlamaAbiLayoutSnapshot * out) {
    if (out == NULL) {
        return;
    }
    memset(out, 0, sizeof(*out));
    out->bound_to_upstream_llama_header = false;
}

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
) {
    (void)dylib_path;
    (void)model_path;
    (void)prefer_metal;
    (void)n_ctx;
    (void)n_threads;
    if (out_backend_kind != NULL) {
        *out_backend_kind = 0;
    }
    if (out_device_name != NULL && device_name_cap > 0) {
        out_device_name[0] = '\0';
    }
    if (out_n_embd != NULL) {
        *out_n_embd = 0;
    }
    write_err(
        out_err,
        err_cap,
        "BLOCKED: upstream llama.h not yet mounted by Owner H on ttzip_llama_bridge"
    );
    return NULL;
}

int32_t ttzip_llama_session_encode(
    TtzipLlamaBridgeSession * s,
    const char * prompt,
    int32_t max_tokens,
    float * out_embd,
    size_t embd_cap,
    int32_t * out_tokens,
    size_t tokens_cap,
    int32_t * out_token_count,
    char * out_err,
    size_t err_cap
) {
    (void)s;
    (void)prompt;
    (void)max_tokens;
    (void)out_embd;
    (void)embd_cap;
    (void)out_tokens;
    (void)tokens_cap;
    (void)out_token_count;
    write_err(
        out_err,
        err_cap,
        "BLOCKED: upstream llama.h not yet mounted by Owner H on ttzip_llama_bridge"
    );
    return -1;
}

void ttzip_llama_session_destroy(TtzipLlamaBridgeSession * s) {
    (void)s;
}

#endif // TTZIP_UPSTREAM_LLAMA_HEADER_AVAILABLE

uint64_t ttzip_llama_current_phys_footprint_bytes(void) {
#if defined(__APPLE__)
    task_vm_info_data_t vm_info;
    mach_msg_type_number_t count = TASK_VM_INFO_COUNT;
    kern_return_t kr = task_info(mach_task_self(), TASK_VM_INFO, (task_info_t)&vm_info, &count);
    if (kr == KERN_SUCCESS) {
        return (uint64_t)vm_info.phys_footprint;
    }
#endif
    return 0;
}

