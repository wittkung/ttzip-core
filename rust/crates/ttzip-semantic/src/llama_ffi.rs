// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Encapsulated `llama.cpp` runtime session (`b11461` / `4d756bc72bf00a4aacf410ae15a2d315f3db400d`
//! + `llama.cpp-no-logits.patch`) backed by the opaque C header bridge (`csrc/ttzip_llama_bridge.c`),
//!
//! Strict bundled-only production dylib loading, real GGUF model/context lifecycle,
//! tokenization, batch embedding inference, and reference-vector checkpoint verification.

use crate::llama_abi::{
    open_bridge_session, ttzip_llama_session_destroy, ttzip_llama_session_encode,
    verify_pinned_b11461_abi_layout, AbiHeaderStatus, LlamaDylibLoadPolicy,
    TtzipLlamaBridgeSession,
};
use crate::model::{
    cosine_similarity_f32, format_code_query_prompt, format_document_prompt, format_query_prompt,
    quantize_mrl128_int8, verify_gguf_model_file, ReferenceVectorEntry, SemanticComputeBackend,
    SemanticModelError, SemanticModelInfo, SemanticModelState, PINNED_GGUF_FILENAME,
    PINNED_GGUF_SHA256, PINNED_GGUF_SIZE_BYTES, PINNED_MODEL_FINGERPRINT,
};
use crate::scoring::EmbeddingProvider;
use serde::{Deserialize, Serialize};
use std::ffi::{c_char, CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckpointStatus {
    Pass,
    Blocked,
    Fail,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RealModelCheckpointReport {
    pub status: CheckpointStatus,
    pub model_path: String,
    pub backend: SemanticComputeBackend,
    pub verified_backend_device: Option<String>,
    pub load_time_ms: f64,
    pub model_file_bytes: u64,
    pub verified_sha256: Option<String>,
    pub output_dimension: usize,
    pub compared_reference_count: usize,
    pub min_reference_cosine: Option<f32>,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub struct LlamaRuntimeConfig {
    pub model_path: PathBuf,
    pub dylib_policy: LlamaDylibLoadPolicy,
    pub prefer_metal: bool,
    pub max_query_tokens: usize,
    pub max_doc_tokens: usize,
    pub n_ctx: u32,
    pub n_threads: i32,
}

impl Default for LlamaRuntimeConfig {
    fn default() -> Self {
        Self {
            model_path: default_installed_gguf_path(),
            dylib_policy: LlamaDylibLoadPolicy::BundledOnly,
            prefer_metal: true,
            max_query_tokens: 256,
            max_doc_tokens: 512,
            n_ctx: 2048,
            n_threads: 4,
        }
    }
}

impl LlamaRuntimeConfig {
    /// Creates a production runtime config looking for `embeddinggemma-2-Q8_0.gguf` inside `storage_dir/models`
    /// with `LlamaDylibLoadPolicy::BundledOnly` (never using Homebrew or env dylib overrides).
    pub fn default_for_storage_dir(storage_dir: &Path) -> Self {
        let candidate = storage_dir.join("models").join(PINNED_GGUF_FILENAME);
        let model_path = if candidate.exists() {
            candidate
        } else {
            default_installed_gguf_path()
        };
        Self {
            model_path,
            dylib_policy: LlamaDylibLoadPolicy::BundledOnly,
            ..Self::default()
        }
    }
}

/// Resolves the canonical production path for `embeddinggemma-2-Q8_0.gguf`.
pub fn default_installed_gguf_path() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        let app_support = PathBuf::from(&home)
            .join("Library/Application Support/com.metastudyline.ttzip/Models")
            .join(PINNED_GGUF_FILENAME);
        if app_support.exists() {
            return app_support;
        }
        let legacy_support = PathBuf::from(&home)
            .join("Library/Application Support/TTZip/Models")
            .join(PINNED_GGUF_FILENAME);
        if legacy_support.exists() {
            return legacy_support;
        }
        return app_support;
    }
    PathBuf::from(PINNED_GGUF_FILENAME)
}

struct LlamaSession {
    state: SemanticModelState,
    verified_sha256: Option<String>,
    verified_device_name: Option<String>,
    bridge_ptr: *mut TtzipLlamaBridgeSession,
    n_embd_out: usize,
}

impl LlamaSession {
    fn new() -> Self {
        Self {
            state: SemanticModelState::NotInstalled,
            verified_sha256: None,
            verified_device_name: None,
            bridge_ptr: std::ptr::null_mut(),
            n_embd_out: 0,
        }
    }

    fn unload(&mut self) {
        if !self.bridge_ptr.is_null() {
            unsafe {
                ttzip_llama_session_destroy(self.bridge_ptr);
            }
            self.bridge_ptr = std::ptr::null_mut();
        }
        self.n_embd_out = 0;
        self.verified_device_name = None;
        if matches!(self.state, SemanticModelState::Ready(_)) {
            self.state = SemanticModelState::NotInstalled;
        }
    }
}

impl Drop for LlamaSession {
    fn drop(&mut self) {
        self.unload();
    }
}

/// Encapsulates `llama.cpp` opaque C bridge session and verified GGUF model state.
/// Thread-safe and directly implements [`EmbeddingProvider`] for production use.
pub struct LlamaRuntimeEngine {
    config: LlamaRuntimeConfig,
    session: Mutex<LlamaSession>,
}

unsafe impl Send for LlamaRuntimeEngine {}
unsafe impl Sync for LlamaRuntimeEngine {}

impl LlamaRuntimeEngine {
    pub fn new(config: LlamaRuntimeConfig) -> Self {
        Self {
            config,
            session: Mutex::new(LlamaSession::new()),
        }
    }

    pub fn state(&self) -> SemanticModelState {
        self.session
            .lock()
            .expect("llama session lock poisoned")
            .state
            .clone()
    }

    pub fn config(&self) -> &LlamaRuntimeConfig {
        &self.config
    }

    /// Verifies the `b11461` C-ABI layout, GGUF weight file on disk, opens the bundled `llama.cpp`
    /// runtime via `csrc/ttzip_llama_bridge.c`, probes real backend devices, and loads the model + context.
    pub fn verify_and_prepare(&self) -> Result<SemanticModelInfo, SemanticModelError> {
        let mut guard = self.session.lock().expect("llama session lock poisoned");
        Self::ensure_loaded_locked(&self.config, &mut guard)
    }

    fn ensure_loaded_locked(
        config: &LlamaRuntimeConfig,
        session: &mut LlamaSession,
    ) -> Result<SemanticModelInfo, SemanticModelError> {
        if let SemanticModelState::Ready(ref info) = session.state {
            if !session.bridge_ptr.is_null() {
                return Ok(info.clone());
            }
        }
        session.state = SemanticModelState::Verifying;

        let sha256 = verify_gguf_model_file(
            &config.model_path,
            PINNED_GGUF_SIZE_BYTES,
            PINNED_GGUF_SHA256,
        )
        .inspect_err(|err| {
            session.state = if matches!(err, SemanticModelError::ModelFileMissing { .. }) {
                SemanticModelState::NotInstalled
            } else {
                SemanticModelState::Failed(err.clone())
            };
        })?;

        match verify_pinned_b11461_abi_layout() {
            Ok(AbiHeaderStatus::Verified(_)) => {}
            Ok(AbiHeaderStatus::UnboundToUpstreamHeader) => {
                let err = SemanticModelError::RuntimeFailed {
                    reason:
                        "BLOCKED: upstream llama.h not yet mounted by Owner H on ttzip_llama_bridge"
                            .to_string(),
                };
                session.state = SemanticModelState::Failed(err.clone());
                return Err(err);
            }
            Err(abi_err) => {
                let err = SemanticModelError::RuntimeFailed { reason: abi_err };
                session.state = SemanticModelState::Failed(err.clone());
                return Err(err);
            }
        }

        let (bridge_ptr, backend, dev_name, n_embd_out) = open_bridge_session(
            &config.dylib_policy,
            &config.model_path,
            config.prefer_metal,
            config.n_ctx,
            config.n_threads,
        )
        .map_err(|reason| {
            let err = SemanticModelError::RuntimeFailed { reason };
            session.state = SemanticModelState::Failed(err.clone());
            err
        })?;

        let info = SemanticModelInfo::pinned(backend);
        session.verified_sha256 = Some(sha256);
        session.verified_device_name = Some(dev_name);
        session.bridge_ptr = bridge_ptr;
        session.n_embd_out = n_embd_out;
        session.state = SemanticModelState::Ready(info.clone());
        Ok(info)
    }

    /// Encodes a formatted prompt into a full L2-normalized `f32` vector (768-D) and returns `(embedding, tokens)`.
    pub fn encode_raw_f32(
        &self,
        formatted_prompt: &str,
        max_tokens: usize,
    ) -> Result<(Vec<f32>, Vec<i32>), SemanticModelError> {
        let mut guard = self.session.lock().expect("llama session lock poisoned");
        Self::ensure_loaded_locked(&self.config, &mut guard)?;
        Self::encode_raw_f32_locked(&mut guard, formatted_prompt, max_tokens)
    }

    fn encode_raw_f32_locked(
        session: &mut LlamaSession,
        formatted_prompt: &str,
        max_tokens: usize,
    ) -> Result<(Vec<f32>, Vec<i32>), SemanticModelError> {
        if session.bridge_ptr.is_null() {
            return Err(SemanticModelError::RuntimeFailed {
                reason: "llama bridge session not initialized".to_string(),
            });
        }
        let c_prompt =
            CString::new(formatted_prompt).map_err(|e| SemanticModelError::RuntimeFailed {
                reason: e.to_string(),
            })?;
        let mut raw_emb = vec![0.0f32; session.n_embd_out];
        let mut tokens = vec![0i32; formatted_prompt.len() + 16];
        let mut token_count: i32 = 0;
        let mut err_buf = [0 as c_char; 256];

        let rc = unsafe {
            ttzip_llama_session_encode(
                session.bridge_ptr,
                c_prompt.as_ptr(),
                max_tokens as i32,
                raw_emb.as_mut_ptr(),
                raw_emb.len(),
                tokens.as_mut_ptr(),
                tokens.len(),
                &mut token_count,
                err_buf.as_mut_ptr(),
                err_buf.len(),
            )
        };
        if rc != 0 {
            let reason = unsafe { CStr::from_ptr(err_buf.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            return Err(SemanticModelError::RuntimeFailed { reason });
        }
        tokens.truncate(token_count.max(0) as usize);

        let sum_sq: f64 = raw_emb.iter().map(|&v| (v as f64) * (v as f64)).sum();
        let norm = sum_sq.sqrt();
        if !norm.is_finite() || norm < 1e-9 {
            return Err(SemanticModelError::ZeroOrNonFiniteVector);
        }
        let inv = (1.0 / norm) as f32;
        for v in &mut raw_emb {
            *v *= inv;
        }
        Ok((raw_emb, tokens))
    }

    /// Runs the minimal real-model inference smoke test (`compared_reference_count == 0`).
    pub fn run_real_model_checkpoint(&self) -> RealModelCheckpointReport {
        self.run_real_model_checkpoint_with_references(&[])
    }

    /// Runs real GGUF loading, tokenization, forward pass, and cosine parity verification against `references`.
    /// Never returns `CheckpointStatus::Pass` without executing real `llama.cpp` inference.
    pub fn run_real_model_checkpoint_with_references(
        &self,
        references: &[ReferenceVectorEntry],
    ) -> RealModelCheckpointReport {
        let started = Instant::now();
        let path_str = self.config.model_path.to_string_lossy().into_owned();
        let mut guard = self.session.lock().expect("llama session lock poisoned");

        let info = match Self::ensure_loaded_locked(&self.config, &mut guard) {
            Ok(info) => info,
            Err(err) => {
                return Self::build_load_error_report(&guard, path_str, started, err);
            }
        };

        Self::verify_loaded_checkpoint_locked(
            &self.config,
            &mut guard,
            &info,
            path_str,
            started,
            references,
        )
    }

    fn build_load_error_report(
        session: &LlamaSession,
        path_str: String,
        started: Instant,
        err: SemanticModelError,
    ) -> RealModelCheckpointReport {
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        if let SemanticModelError::ModelFileMissing { expected_path } = err {
            return RealModelCheckpointReport {
                status: CheckpointStatus::Blocked,
                model_path: expected_path.clone(),
                backend: SemanticComputeBackend::CpuFallback,
                verified_backend_device: None,
                load_time_ms: elapsed_ms,
                model_file_bytes: 0,
                verified_sha256: None,
                output_dimension: 0,
                compared_reference_count: 0,
                min_reference_cosine: None,
                reason: format!(
                    "BLOCKED: Required GGUF model {} ({} bytes, sha256={}) not present at {}",
                    PINNED_GGUF_FILENAME, PINNED_GGUF_SIZE_BYTES, PINNED_GGUF_SHA256, expected_path
                ),
            };
        }
        let is_blocked_runtime = matches!(
            &err,
            SemanticModelError::RuntimeFailed { reason }
                if reason.contains("not found") || reason.contains("BLOCKED:")
        );
        RealModelCheckpointReport {
            status: if is_blocked_runtime {
                CheckpointStatus::Blocked
            } else {
                CheckpointStatus::Fail
            },
            model_path: path_str,
            backend: SemanticComputeBackend::CpuFallback,
            verified_backend_device: None,
            load_time_ms: elapsed_ms,
            model_file_bytes: 0,
            verified_sha256: session.verified_sha256.clone(),
            output_dimension: 0,
            compared_reference_count: 0,
            min_reference_cosine: None,
            reason: format!("{err}"),
        }
    }

    fn compare_reference_vectors(
        config: &LlamaRuntimeConfig,
        session: &mut LlamaSession,
        references: &[ReferenceVectorEntry],
    ) -> Result<(usize, f32), (usize, String)> {
        let mut worst_cos = 1.0f32;
        let mut checked = 0usize;
        for entry in references {
            let Some(ref prompt) = entry.formatted_prompt else {
                continue;
            };
            match Self::encode_raw_f32_locked(session, prompt, config.n_ctx as usize) {
                Ok((actual_vec, _)) => {
                    let cos = cosine_similarity_f32(&actual_vec, &entry.vector);
                    if cos < worst_cos {
                        worst_cos = cos;
                    }
                    checked += 1;
                }
                Err(err) => return Err((checked, format!("FAIL on '{}': {err}", entry.input))),
            }
        }
        Ok((checked, worst_cos))
    }

    fn checkpoint_probe_error(
        config: &LlamaRuntimeConfig,
        session: &mut LlamaSession,
        checked: usize,
    ) -> Option<SemanticModelError> {
        if checked != 0 {
            return None;
        }
        Self::encode_raw_f32_locked(
            session,
            "task: search result | query: verification probe",
            config.max_query_tokens,
        )
        .err()
    }

    fn verify_loaded_checkpoint_locked(
        config: &LlamaRuntimeConfig,
        session: &mut LlamaSession,
        info: &SemanticModelInfo,
        path_str: String,
        started: Instant,
        references: &[ReferenceVectorEntry],
    ) -> RealModelCheckpointReport {
        let (checked, worst_cos) =
            match Self::compare_reference_vectors(config, session, references) {
                Ok(result) => result,
                Err((checked, reason)) => {
                    return RealModelCheckpointReport {
                        status: CheckpointStatus::Fail,
                        model_path: path_str,
                        backend: info.backend,
                        verified_backend_device: session.verified_device_name.clone(),
                        load_time_ms: started.elapsed().as_secs_f64() * 1000.0,
                        model_file_bytes: PINNED_GGUF_SIZE_BYTES,
                        verified_sha256: session.verified_sha256.clone(),
                        output_dimension: session.n_embd_out,
                        compared_reference_count: checked,
                        min_reference_cosine: None,
                        reason,
                    };
                }
            };
        if let Some(err) = Self::checkpoint_probe_error(config, session, checked) {
            return RealModelCheckpointReport {
                status: CheckpointStatus::Fail,
                model_path: path_str,
                backend: info.backend,
                verified_backend_device: session.verified_device_name.clone(),
                load_time_ms: started.elapsed().as_secs_f64() * 1000.0,
                model_file_bytes: PINNED_GGUF_SIZE_BYTES,
                verified_sha256: session.verified_sha256.clone(),
                output_dimension: 0,
                compared_reference_count: 0,
                min_reference_cosine: None,
                reason: format!("FAIL: inference failed: {err}"),
            };
        } else if worst_cos < 0.999 {
            return RealModelCheckpointReport {
                status: CheckpointStatus::Fail,
                model_path: path_str,
                backend: info.backend,
                verified_backend_device: session.verified_device_name.clone(),
                load_time_ms: started.elapsed().as_secs_f64() * 1000.0,
                model_file_bytes: PINNED_GGUF_SIZE_BYTES,
                verified_sha256: session.verified_sha256.clone(),
                output_dimension: session.n_embd_out,
                compared_reference_count: checked,
                min_reference_cosine: Some(worst_cos),
                reason: format!("FAIL: min reference cosine {worst_cos:.6} < 0.999"),
            };
        }

        RealModelCheckpointReport {
            status: CheckpointStatus::Pass,
            model_path: path_str,
            backend: info.backend,
            verified_backend_device: session.verified_device_name.clone(),
            load_time_ms: started.elapsed().as_secs_f64() * 1000.0,
            model_file_bytes: PINNED_GGUF_SIZE_BYTES,
            verified_sha256: session.verified_sha256.clone(),
            output_dimension: session.n_embd_out,
            compared_reference_count: checked,
            min_reference_cosine: if checked > 0 { Some(worst_cos) } else { None },
            reason: if checked > 0 {
                format!("Verified real GGUF model load and parity across {checked} references")
            } else {
                "Verified real GGUF model smoke inference (0 reference vectors compared)"
                    .to_string()
            },
        }
    }

    /// Unloads any open runtime handle and releases memory immediately.
    pub fn unload(&self) {
        let mut guard = self.session.lock().expect("llama session lock poisoned");
        guard.unload();
    }
}

impl EmbeddingProvider for LlamaRuntimeEngine {
    fn model_state(&self) -> SemanticModelState {
        let mut guard = self.session.lock().expect("llama session lock poisoned");
        if matches!(guard.state, SemanticModelState::NotInstalled)
            && self.config.model_path.exists()
        {
            let _ = Self::ensure_loaded_locked(&self.config, &mut guard);
        }
        guard.state.clone()
    }

    fn model_fingerprint(&self) -> u64 {
        PINNED_MODEL_FINGERPRINT
    }

    fn encode_query(
        &self,
        query: &str,
        is_code_query: bool,
    ) -> Result<[i8; 128], SemanticModelError> {
        if query.trim().is_empty() {
            return Err(SemanticModelError::ZeroOrNonFiniteVector);
        }
        let prompt = if is_code_query {
            format_code_query_prompt(query)
        } else {
            format_query_prompt(query)
        };
        let (raw_f32, _) = self.encode_raw_f32(&prompt, self.config.max_query_tokens)?;
        postprocess_llama_pooled_embedding(&raw_f32)
    }

    fn encode_document(
        &self,
        relative_path: Option<&str>,
        chunk_text: &str,
    ) -> Result<[i8; 128], SemanticModelError> {
        if chunk_text.trim().is_empty() {
            return Err(SemanticModelError::ZeroOrNonFiniteVector);
        }
        let prompt = format_document_prompt(relative_path, chunk_text);
        let (raw_f32, _) = self.encode_raw_f32(&prompt, self.config.max_doc_tokens)?;
        postprocess_llama_pooled_embedding(&raw_f32)
    }
}

/// Converts a 768-D float slice from `llama_get_embeddings_seq` into a validated MRL-128 INT8 vector.
pub fn postprocess_llama_pooled_embedding(
    pooled_768: &[f32],
) -> Result<[i8; 128], SemanticModelError> {
    if pooled_768.len() < 128 {
        return Err(SemanticModelError::ZeroOrNonFiniteVector);
    }
    quantize_mrl128_int8(pooled_768)
}

/// Checks if a file path has the `.gguf` extension.
#[inline]
pub fn is_gguf_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("gguf"))
}
