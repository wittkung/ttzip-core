// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Mozilla UniFFI Archive Extraction and In-Place Mutation Scaffolding.
//!
//! Every disk-writing extraction runs through `ExtractionSession`; the one-shot
//! functions here drive a session with the implicit resolutions of a policy.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use super::extraction_observer::{OneShotStageObserver, StageObserver};
use super::extraction_session::{
    CollisionPolicy, CommitResult, ExtractionOutcome, ExtractionRequest, ExtractionSession,
    ResolutionSet,
};
use super::types::{
    CancellationToken, CompressionReport, InPlaceMutationAction, ProgressHandler, TTZipError,
};

/// Resolves effective thread budget adaptively based on hardware topology and QoS.
#[inline]
#[must_use]
pub fn resolve_adaptive_extraction_threads(requested: u32) -> u32 {
    if requested > 0 {
        requested.clamp(1, 64)
    } else {
        let caps = crate::platform::cpu::CpuCapabilities::get();
        if caps.p_cores > 0 {
            caps.p_cores.clamp(1, 64)
        } else {
            crate::types::resolve_thread_budget(0) as u32
        }
    }
}

/// Result of a non-interactive session.
struct PolicyRun {
    staged_bytes: u64,
    outcome: ExtractionOutcome,
}

/// Runs `new -> preflight -> stage -> commit` with the implicit resolution set
/// of `request.policy`. `Stop` aborts before anything reaches the destination
/// as soon as any conflict is known.
fn run_policy_session(
    request: ExtractionRequest,
    progress: Option<&dyn ProgressHandler>,
    cancel: Option<&CancellationToken>,
) -> Result<PolicyRun, TTZipError> {
    let policy = request.policy;
    if policy == CollisionPolicy::Ask {
        return Err(TTZipError::InvalidArgument {
            message: "Non-interactive extraction cannot use the Ask policy".to_string(),
        });
    }
    let stop = |session: &ExtractionSession| {
        session.abort().map(|outcome| PolicyRun { staged_bytes: 0, outcome })
    };
    let session = ExtractionSession::new(request)?;
    let observer = OneShotStageObserver { progress, cancel };

    if session.preflight()?.is_some() && policy == CollisionPolicy::Stop {
        return stop(&session);
    }
    if observer.is_cancelled() {
        session.abort()?;
        return Err(TTZipError::Cancelled);
    }
    let report = session.stage_observed(&observer)?;
    if report.post_stage_conflicts.is_some() && policy == CollisionPolicy::Stop {
        return stop(&session);
    }
    let resolutions = ResolutionSet {
        default_action: policy.default_action(),
        overrides: Vec::new(),
    };
    match session.commit(resolutions)? {
        CommitResult::Finished { outcome } => Ok(PolicyRun {
            staged_bytes: report.staged_bytes,
            outcome,
        }),
        CommitResult::NeedsDecision { .. } => stop(&session),
    }
}

fn compression_report(archive: &Path, run: PolicyRun, start: Instant) -> CompressionReport {
    let bytes = run.staged_bytes;
    let elapsed = start.elapsed();
    let compressed = std::fs::metadata(archive).map_or(0, |m| m.len());
    let space_savings_pct = if bytes > 0 && compressed <= bytes {
        (bytes - compressed) as f64 / bytes as f64 * 100.0
    } else {
        0.0
    };
    CompressionReport {
        uncompressed_bytes: bytes,
        compressed_bytes: compressed,
        elapsed_nanos: u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX),
        throughput_mbs: (bytes as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64().max(1e-6),
        space_savings_pct,
        engine_provenance: "Mozilla UniFFI Native Core Pipeline".to_string(),
        outcome: Some(run.outcome),
    }
}

/// Extracts a full archive with a collision policy, progress and cancellation.
#[uniffi::export]
pub fn extract_archive_stream(
    archive_path: String,
    destination_dir: String,
    policy: CollisionPolicy,
    password: Option<String>,
    progress: Option<Box<dyn ProgressHandler>>,
    token: Option<Arc<CancellationToken>>,
) -> Result<CompressionReport, TTZipError> {
    let start = Instant::now();
    let request = ExtractionRequest {
        archive_path: archive_path.clone(),
        destination_root: destination_dir,
        password,
        entry_filter: None,
        root_at_common_parent: false,
        policy,
    };
    let run = run_policy_session(request, progress.as_deref(), token.as_deref())?;
    Ok(compression_report(Path::new(&archive_path), run, start))
}

/// Extracts a single entry stream preview directly by relative entry path.
#[uniffi::export]
pub fn extract_single_entry_by_path(
    archive_path: String,
    entry_path: String,
    password: Option<String>,
) -> Result<Vec<u8>, TTZipError> {
    let secure_pwd = password.map(crate::crypto::ZeroizingString::new);
    let p = std::path::Path::new(&archive_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let source = crate::archive::source::open_archive_source(p)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let mapped = source.as_slice().ok_or_else(|| TTZipError::IoError {
        message: "Failed to map archive bytes".to_string(),
    })?;

    let clean_target = entry_path.trim_start_matches('/');
    const DEFAULT_EXTRACTION_BUDGET_BYTES: u64 = 64 * 1024 * 1024; // Strict 64MB limit

    if mapped.starts_with(b"7z\xBC\xAF\x27\x1C") {
        let arch = crate::sevenz::decoder::SevenZArchive::open_slice(mapped)
            .map_err(|_| TTZipError::CorruptHeader { details: "Invalid 7z header".to_string(), offset: 0 })?;
        
        let found_idx = arch.files().iter().position(|f| {
            f.rel_path == clean_target || f.rel_path.ends_with(&format!("/{}", clean_target))
        }).ok_or_else(|| TTZipError::FileNotFound { path: entry_path.clone() })?;

        if let Some(loc) = arch.seek_index().entries.get(found_idx) {
            if loc.uncompressed_size > DEFAULT_EXTRACTION_BUDGET_BYTES {
                return Err(TTZipError::OutOfMemory {
                    details: format!(
                        "Entry uncompressed size ({} bytes) exceeds safety budget ({} bytes)",
                        loc.uncompressed_size, DEFAULT_EXTRACTION_BUDGET_BYTES
                    ),
                });
            }
        }

        let budget_bytes = DEFAULT_EXTRACTION_BUDGET_BYTES;
        crate::sevenz::decoder::stream::extract_entry_bytes_stream_bounded(
            mapped,
            arch.info(),
            arch.seek_index(),
            found_idx,
            secure_pwd.as_deref(),
            budget_bytes,
        ).map_err(|status| match status {
            crate::types::TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
            crate::types::TTZipStatus::ErrSolidBudgetExceeded => TTZipError::DecompressionBombDetected {
                reason: "7z solid decompression budget exceeded".to_string(),
            },
            crate::types::TTZipStatus::ErrOutOfMemory => TTZipError::OutOfMemory {
                details: "Memory allocation failed during 7z decompression".to_string(),
            },
            crate::types::TTZipStatus::ErrCorruptHeader => TTZipError::CorruptHeader {
                details: "Corrupted 7z payload or CRC mismatch".to_string(),
                offset: 0,
            },
            crate::types::TTZipStatus::ErrSecurityViolation => TTZipError::DecompressionBombDetected {
                reason: "Security policy boundary violation".to_string(),
            },
            _ => TTZipError::EngineError { code: status as i32 },
        })
    } else if let Ok(zip_archive) = crate::zip::reader::ZipArchive::open_slice(mapped) {
        let found_idx = zip_archive.entries().iter().position(|e| {
            e.rel_path == clean_target || e.rel_path.ends_with(&format!("/{}", clean_target))
        }).ok_or_else(|| TTZipError::FileNotFound { path: entry_path.clone() })?;

        let entry = &zip_archive.entries()[found_idx];
        if entry.uncompressed_size > DEFAULT_EXTRACTION_BUDGET_BYTES {
            return Err(TTZipError::OutOfMemory {
                details: format!(
                    "Entry uncompressed size ({} bytes) exceeds safety budget ({} bytes)",
                    entry.uncompressed_size, DEFAULT_EXTRACTION_BUDGET_BYTES
                ),
            });
        }

        zip_archive.extract_entry_bytes(found_idx, secure_pwd.as_deref())
            .map_err(|status| match status {
                crate::types::TTZipStatus::ErrCorruptHeader => {
                    TTZipError::CorruptHeader { details: "Corrupted entry CRC or payload".to_string(), offset: 0 }
                }
                crate::types::TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
                crate::types::TTZipStatus::ErrOutOfMemory => TTZipError::OutOfMemory {
                    details: "Memory allocation failed during ZIP decompression".to_string(),
                },
                crate::types::TTZipStatus::ErrSecurityViolation => TTZipError::DecompressionBombDetected {
                    reason: "Decompression ratio or overlap violation detected".to_string(),
                },
                _ => TTZipError::EngineError { code: status as i32 },
            })
    } else {
        crate::archive::unified::extract_single::extract_single_entry_memory(
            p,
            Some(&entry_path),
            -1,
            secure_pwd.as_deref(),
        ).map_err(|status| match status {
            crate::types::TTZipStatus::ErrFileNotFound => TTZipError::FileNotFound {
                path: entry_path.clone(),
            },
            crate::types::TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
            crate::types::TTZipStatus::ErrCorruptHeader => TTZipError::CorruptHeader {
                details: "Corrupted entry CRC or payload".to_string(),
                offset: 0,
            },
            crate::types::TTZipStatus::ErrOutOfMemory => TTZipError::OutOfMemory {
                details: "Memory allocation failed during decompression".to_string(),
            },
            crate::types::TTZipStatus::ErrSecurityViolation => TTZipError::DecompressionBombDetected {
                reason: "Decompression ratio or overlap violation detected".to_string(),
            },
            _ => TTZipError::EngineError { code: status as i32 },
        })
    }
}

/// Repairs damaged archive file and writes to output destination.
#[uniffi::export]
pub fn repair_archive_file(damaged_path: String, output_path: String) -> Result<u64, TTZipError> {
    let damaged = std::path::Path::new(&damaged_path);
    let output = std::path::Path::new(&output_path);
    if !damaged.exists() {
        return Err(TTZipError::FileNotFound { path: damaged_path });
    }
    crate::archive::unified::repair::repair_archive(damaged, output)
        .map(|count| count as u64)
        .map_err(|s| TTZipError::EngineError { code: s as i32 })
}

/// Atomically mutates archive in-place (append, replace, delete) without full recompression.
#[uniffi::export]
pub fn in_place_mutate_archive(
    archive_path: String,
    actions: Vec<InPlaceMutationAction>,
) -> Result<(), TTZipError> {
    let p = std::path::Path::new(&archive_path);
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let mut session = crate::archive::in_place_edit::InPlaceArchiveSession::begin(p, None)
        .map_err(|s| TTZipError::EngineError { code: s as i32 })?;

    for act in actions {
        if act.is_delete {
            session.delete(&act.entry_path)
                .map_err(|s| TTZipError::EngineError { code: s as i32 })?;
        } else if let Some(ref src) = act.source_path {
            let src_path = std::path::Path::new(src);
            session.replace(&act.entry_path, src_path)
                .map_err(|s| TTZipError::EngineError { code: s as i32 })?;
        }
    }

    session.commit()
        .map_err(|s| TTZipError::EngineError { code: s as i32 })?;

    Ok(())
}
