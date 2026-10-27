// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Pinned `llama.cpp` `b11461` (`4d756bc72bf00a4aacf410ae15a2d315f3db400d`) C-ABI layout verifier,
//! opaque C bridge session bindings (`csrc/ttzip_llama_bridge.h`), and strict `BundledOnly`
//! production dynamic library policy.
//!
//! Zero hand-written upstream `llama_*` struct declarations exist in Rust or `ttzip_llama_bridge.h`.
//! All upstream struct layout measurements and compile-time `_Static_assert`s are produced directly
//! from the real upstream `llama.h` inside `csrc/ttzip_llama_bridge.c`.

use crate::model::SemanticComputeBackend;
use std::ffi::{c_char, CStr, CString};
use std::path::PathBuf;

/// Opaque session handle managed by `csrc/ttzip_llama_bridge.c`.
#[repr(C)]
pub struct TtzipLlamaBridgeSession {
    _private: [u8; 0],
}

/// Snapshot of the `llama.h` struct sizes, alignments, and critical field offsets
/// measured inside `csrc/ttzip_llama_bridge.c`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TtzipLlamaAbiLayoutSnapshot {
    pub bound_to_upstream_llama_header: bool,

    pub batch_size: usize,
    pub batch_align: usize,
    pub batch_offset_n_tokens: usize,
    pub batch_offset_token: usize,
    pub batch_offset_logits: usize,

    pub model_params_size: usize,
    pub model_params_align: usize,
    pub model_params_offset_n_gpu_layers: usize,
    pub model_params_offset_load_mode: usize,
    pub model_params_offset_lazy_mode: usize,
    pub model_params_offset_main_gpu: usize,
    pub model_params_offset_tensor_split: usize,
    pub model_params_offset_kv_overrides: usize,
    pub model_params_offset_vocab_only: usize,
    pub model_params_offset_load_mtp: usize,

    pub context_params_size: usize,
    pub context_params_align: usize,
    pub context_params_offset_n_outputs_max: usize,
    pub context_params_offset_n_outputs_max_per_seq: usize,
    pub context_params_offset_n_threads: usize,
    pub context_params_offset_pooling_type: usize,
    pub context_params_offset_attention_type: usize,
    pub context_params_offset_embeddings: usize,
    pub context_params_offset_kv_unified: usize,
    pub context_params_offset_samplers: usize,
    pub context_params_offset_n_samplers: usize,
    pub context_params_offset_ctx_other: usize,
}

/// Reports whether `csrc/ttzip_llama_bridge.c` was compiled with the real upstream `llama.h`
/// and verified against the pinned `b11461` (`4d756bc72bf00a4aacf410ae15a2d315f3db400d`) layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AbiHeaderStatus {
    /// Compiled against real upstream `llama.h` and all sizes, alignments, and offsets match `b11461`.
    Verified(Box<TtzipLlamaAbiLayoutSnapshot>),
    /// Upstream `llama.h` has not yet been mounted on `ttzip_llama_bridge` by Owner H.
    UnboundToUpstreamHeader,
}

extern "C" {
    pub fn ttzip_llama_get_compiled_abi_layout(out_snapshot: *mut TtzipLlamaAbiLayoutSnapshot);

    pub fn ttzip_llama_session_create(
        dylib_path: *const c_char,
        model_path: *const c_char,
        prefer_metal: bool,
        n_ctx: u32,
        n_threads: i32,
        out_backend_kind: *mut i32,
        out_device_name: *mut c_char,
        device_name_cap: usize,
        out_n_embd: *mut i32,
        out_err: *mut c_char,
        err_cap: usize,
    ) -> *mut TtzipLlamaBridgeSession;

    pub fn ttzip_llama_session_encode(
        session: *mut TtzipLlamaBridgeSession,
        prompt: *const c_char,
        max_tokens: i32,
        out_embd: *mut f32,
        embd_cap: usize,
        out_tokens: *mut i32,
        tokens_cap: usize,
        out_token_count: *mut i32,
        out_err: *mut c_char,
        err_cap: usize,
    ) -> i32;

    pub fn ttzip_llama_session_destroy(session: *mut TtzipLlamaBridgeSession);

    pub fn ttzip_llama_current_phys_footprint_bytes() -> u64;
}

/// Returns the current macOS process `phys_footprint` in bytes via `task_info(TASK_VM_INFO)`.
#[inline]
pub fn current_phys_footprint_bytes() -> u64 {
    unsafe { ttzip_llama_current_phys_footprint_bytes() }
}

/// Production dylib loading policy.
/// In production (`BundledOnly`), ONLY bundled fixed-version paths inside the application bundle
/// are permitted. Homebrew, bare library names, and environment variable overrides are forbidden.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LlamaDylibLoadPolicy {
    BundledOnly,
    ExplicitTestPath(PathBuf),
}

/// Fixed bundled dynamic library candidate paths permitted in production.
pub const BUNDLED_PRODUCTION_DYLIB_CANDIDATES: &[&str] = &[
    "@executable_path/../Frameworks/libllamacpp.dylib",
    "@loader_path/../Frameworks/libllamacpp.dylib",
    "@rpath/libllamacpp.dylib",
];

#[inline]
pub fn bundled_production_dylib_candidates() -> &'static [&'static str] {
    BUNDLED_PRODUCTION_DYLIB_CANDIDATES
}

/// Returns `true` if and only if `candidate` is one of the allowed bundled production paths.
/// Rejects Homebrew (`/opt/homebrew/...`), `/usr/local/...`, bare names (`libllama.dylib`), and env paths.
pub fn is_permitted_production_dylib_candidate(candidate: &str) -> bool {
    BUNDLED_PRODUCTION_DYLIB_CANDIDATES.contains(&candidate)
}

/// Returns the expected 64-bit ABI layout snapshot for pinned `llama.cpp` `b11461`
/// (`4d756bc72bf00a4aacf410ae15a2d315f3db400d/include/llama.h`).
pub const fn expected_pinned_b11461_layout_snapshot() -> TtzipLlamaAbiLayoutSnapshot {
    TtzipLlamaAbiLayoutSnapshot {
        bound_to_upstream_llama_header: true,

        batch_size: 56,
        batch_align: 8,
        batch_offset_n_tokens: 0,
        batch_offset_token: 8,
        batch_offset_logits: 48,

        model_params_size: 80,
        model_params_align: 8,
        model_params_offset_n_gpu_layers: 16,
        model_params_offset_load_mode: 24,
        model_params_offset_lazy_mode: 28,
        model_params_offset_main_gpu: 32,
        model_params_offset_tensor_split: 40,
        model_params_offset_kv_overrides: 64,
        model_params_offset_vocab_only: 72,
        model_params_offset_load_mtp: 77,

        context_params_size: 160,
        context_params_align: 8,
        context_params_offset_n_outputs_max: 20,
        context_params_offset_n_outputs_max_per_seq: 24,
        context_params_offset_n_threads: 28,
        context_params_offset_pooling_type: 44,
        context_params_offset_attention_type: 48,
        context_params_offset_embeddings: 128,
        context_params_offset_kv_unified: 133,
        context_params_offset_samplers: 136,
        context_params_offset_n_samplers: 144,
        context_params_offset_ctx_other: 152,
    }
}

/// Validates a layout snapshot obtained from `csrc/ttzip_llama_bridge.c` against the pinned
/// `b11461` (`4d756bc72bf00a4aacf410ae15a2d315f3db400d`) layout contract.
pub fn validate_abi_layout_snapshot(
    snap: &TtzipLlamaAbiLayoutSnapshot,
) -> Result<AbiHeaderStatus, String> {
    if !snap.bound_to_upstream_llama_header {
        return Ok(AbiHeaderStatus::UnboundToUpstreamHeader);
    }
    let expected = expected_pinned_b11461_layout_snapshot();
    if *snap != expected {
        return Err(format!(
            "Upstream llama.h ABI layout mismatch against pinned b11461: actual={snap:?}, expected={expected:?}"
        ));
    }
    Ok(AbiHeaderStatus::Verified(Box::new(*snap)))
}

/// Queries the compiled `csrc/ttzip_llama_bridge.c` layout snapshot and validates it against
/// the pinned `b11461` (`4d756bc72bf00a4aacf410ae15a2d315f3db400d`) contract.
pub fn verify_pinned_b11461_abi_layout() -> Result<AbiHeaderStatus, String> {
    let mut c_snap = TtzipLlamaAbiLayoutSnapshot::default();
    unsafe {
        ttzip_llama_get_compiled_abi_layout(&mut c_snap);
    }
    validate_abi_layout_snapshot(&c_snap)
}

/// Opens a `TtzipLlamaBridgeSession` using the configured dylib load policy (`BundledOnly` in production).
pub fn open_bridge_session(
    policy: &LlamaDylibLoadPolicy,
    model_path: &std::path::Path,
    prefer_metal: bool,
    n_ctx: u32,
    n_threads: i32,
) -> Result<
    (
        *mut TtzipLlamaBridgeSession,
        SemanticComputeBackend,
        String,
        usize,
    ),
    String,
> {
    match verify_pinned_b11461_abi_layout()? {
        AbiHeaderStatus::Verified(_) => {}
        AbiHeaderStatus::UnboundToUpstreamHeader => {
            return Err(
                "BLOCKED: upstream llama.h not yet mounted by Owner H on ttzip_llama_bridge"
                    .to_string(),
            );
        }
    }

    let c_model_path = CString::new(model_path.to_string_lossy().as_bytes())
        .map_err(|e| format!("invalid model path CString: {e}"))?;

    let candidate_paths: Vec<String> = match policy {
        LlamaDylibLoadPolicy::BundledOnly => BUNDLED_PRODUCTION_DYLIB_CANDIDATES
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        LlamaDylibLoadPolicy::ExplicitTestPath(p) => vec![p.to_string_lossy().into_owned()],
    };

    let mut last_err = String::from("bundled libllamacpp.dylib not found");
    for cand in &candidate_paths {
        let Ok(c_dylib) = CString::new(cand.as_str()) else {
            continue;
        };
        let mut backend_kind: i32 = 0;
        let mut dev_buf = [0 as c_char; 128];
        let mut n_embd: i32 = 0;
        let mut err_buf = [0 as c_char; 256];
        let session_ptr = unsafe {
            ttzip_llama_session_create(
                c_dylib.as_ptr(),
                c_model_path.as_ptr(),
                prefer_metal,
                n_ctx,
                n_threads,
                &mut backend_kind,
                dev_buf.as_mut_ptr(),
                dev_buf.len(),
                &mut n_embd,
                err_buf.as_mut_ptr(),
                err_buf.len(),
            )
        };
        if !session_ptr.is_null() {
            let dev_name = unsafe { CStr::from_ptr(dev_buf.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            let backend = if backend_kind == 1 {
                SemanticComputeBackend::Metal
            } else {
                SemanticComputeBackend::CpuFallback
            };
            return Ok((session_ptr, backend, dev_name, n_embd.max(0) as usize));
        }
        let msg = unsafe { CStr::from_ptr(err_buf.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        if !msg.is_empty() {
            last_err = msg;
        }
    }
    Err(last_err)
}
