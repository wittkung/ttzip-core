// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Mozilla UniFFI Archive Extraction and In-Place Mutation Scaffolding.
//!
//! Every disk-writing extraction runs through `ExtractionSession`; the one-shot
//! functions here drive a session with the implicit resolutions of a policy.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::sevenz::header::SevenZFolder;
use crate::types::TTZipStatus;

mod solid_preview;

pub use self::solid_preview::{SolidLzma2PreviewError, extract_solid_lzma2_preview};
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

pub(crate) fn map_single_entry_status(
    status: crate::types::TTZipStatus,
    missing_path: &str,
) -> TTZipError {
    match status {
        crate::types::TTZipStatus::ErrFileNotFound => TTZipError::FileNotFound {
            path: missing_path.to_string(),
        },
        crate::types::TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
        crate::types::TTZipStatus::ErrSolidBudgetExceeded => {
            TTZipError::DecompressionBombDetected {
                reason: "7z solid decompression budget exceeded".to_string(),
            }
        }
        crate::types::TTZipStatus::ErrOutOfMemory => TTZipError::OutOfMemory {
            details: "Memory allocation failed during decompression".to_string(),
        },
        crate::types::TTZipStatus::ErrCorruptHeader => TTZipError::CorruptHeader {
            details: "Corrupted entry CRC or payload".to_string(),
            offset: 0,
        },
        crate::types::TTZipStatus::ErrSecurityViolation => TTZipError::DecompressionBombDetected {
            reason: "Security policy boundary violation".to_string(),
        },
        _ => TTZipError::EngineError {
            code: status as i32,
        },
    }
}

pub(crate) fn open_sevenz_slice_for_preview<'a>(
    mapped: &'a [u8],
    password: Option<&str>,
) -> Result<crate::sevenz::decoder::SevenZArchive<'a>, TTZipError> {
    crate::sevenz::decoder::SevenZArchive::open_slice_with_password(mapped, password).map_err(
        |status| match status {
            crate::types::TTZipStatus::ErrInvalidPassword => TTZipError::InvalidPassword,
            _ => TTZipError::CorruptHeader {
                details: "Invalid 7z header".to_string(),
                offset: 0,
            },
        },
    )
}

fn resolve_7z_entry_index(
    files: &[crate::sevenz::header::SevenZFileMeta],
    entry_path: Option<&str>,
    entry_index: i64,
) -> Result<usize, TTZipError> {
    if let Some(target) = entry_path {
        let clean = target.trim_start_matches('/');
        let suffix = format!("/{clean}");
        files
            .iter()
            .position(|f| f.rel_path == clean || f.rel_path.ends_with(&suffix))
            .ok_or_else(|| TTZipError::FileNotFound {
                path: target.to_string(),
            })
    } else {
        let idx = usize::try_from(entry_index).unwrap_or(usize::MAX);
        if idx < files.len() {
            Ok(idx)
        } else {
            Err(TTZipError::FileNotFound {
                path: format!("entry #{entry_index}"),
            })
        }
    }
}

pub(crate) fn enforce_7z_entry_budget(
    loc: &crate::sevenz::header::SevenZEntryLocation,
    budget_bytes: u64,
) -> Result<(), TTZipError> {
    const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
    let max_size = if budget_bytes > 0 {
        budget_bytes.min(MAX_ENTRY_BYTES)
    } else {
        MAX_ENTRY_BYTES
    };
    if loc.uncompressed_size > max_size {
        return Err(TTZipError::OutOfMemory {
            details: format!(
                "Entry uncompressed size ({} bytes) exceeds safety budget ({} bytes)",
                loc.uncompressed_size, max_size
            ),
        });
    }
    if budget_bytes > 0 && loc.offset_in_folder > budget_bytes {
        return Err(TTZipError::DecompressionBombDetected {
            reason: "7z solid decompression budget exceeded".to_string(),
        });
    }
    Ok(())
}

pub(crate) fn extract_single_entry_split_guarded(
    volume_chain: &[std::path::PathBuf],
    archive_path: &Path,
    entry_path: Option<&str>,
    entry_index: i64,
    password: Option<&str>,
    max_preceding_budget_mb: u64,
) -> Result<Vec<u8>, TTZipError> {
    use std::io::{Read, Seek, SeekFrom};
    let missing_label = entry_path
        .map(str::to_owned)
        .unwrap_or_else(|| format!("entry #{entry_index}"));
    let mut mv_reader =
        crate::archive::split::VirtualMultiVolumeReader::from_volumes(volume_chain.to_vec())
            .map_err(|e| TTZipError::IoError {
                message: e.to_string(),
            })?;
    let mut magic = [0u8; 6];
    let is_sevenz = mv_reader.read_exact(&mut magic).is_ok() && &magic == b"7z\xBC\xAF\x27\x1C";
    let _ = mv_reader.seek(SeekFrom::Start(0));

    if !is_sevenz {
        return crate::archive::unified::extract_single::extract_single_entry_memory(
            archive_path,
            entry_path,
            entry_index,
            password,
        )
        .map_err(|s| map_single_entry_status(s, &missing_label));
    }

    let info = crate::sevenz::encrypted_header::EncodedHeaderDecoder::default()
        .decode_from_reader(&mut mv_reader, password)
        .map_err(|e| match e {
            crate::sevenz::dag::SevenZError::BadPassword => TTZipError::InvalidPassword,
            _ => TTZipError::CorruptHeader {
                details: "Invalid 7z header".to_string(),
                offset: 0,
            },
        })?;
    let seek_idx = crate::sevenz::header::SevenZSeekIndex::build(&info);
    let idx = resolve_7z_entry_index(&info.files, entry_path, entry_index)?;
    let budget_bytes = max_preceding_budget_mb.saturating_mul(1024 * 1024);
    if let Some(loc) = seek_idx.entries.get(idx) {
        let is_unencrypted_copy = loc
            .folder_index
            .and_then(|fi| info.folders.get(fi))
            .map(|f| crate::archive::stream_adapter::is_unencrypted_copy_folder(&info, f))
            .unwrap_or(false);
        let effective_budget = if is_unencrypted_copy { 0 } else { budget_bytes };
        enforce_7z_entry_budget(loc, effective_budget)?;
    }
    crate::archive::stream_adapter::extract_7z_single_entry_from_reader_fast(
        &mut mv_reader,
        &info,
        &seek_idx,
        idx,
        password,
    )
    .map_err(|s| map_single_entry_status(s, &missing_label))
}

fn extract_zip_entry_by_path_bounded(
    zip_archive: &crate::zip::reader::ZipArchive<'_>,
    clean_target: &str,
    entry_path: &str,
    password: Option<&str>,
    budget_bytes: u64,
) -> Result<Vec<u8>, TTZipError> {
    let suffix = format!("/{clean_target}");
    let found_idx = zip_archive
        .entries()
        .iter()
        .position(|e| e.rel_path == clean_target || e.rel_path.ends_with(&suffix))
        .ok_or_else(|| TTZipError::FileNotFound {
            path: entry_path.to_string(),
        })?;
    let entry = &zip_archive.entries()[found_idx];
    if entry.uncompressed_size > budget_bytes {
        return Err(TTZipError::OutOfMemory {
            details: format!(
                "Entry uncompressed size ({} bytes) exceeds safety budget ({budget_bytes} bytes)",
                entry.uncompressed_size
            ),
        });
    }
    zip_archive
        .extract_entry_bytes(found_idx, password)
        .map_err(|s| map_single_entry_status(s, entry_path))
}

fn read_stored_7z_preview(
    mapped: &[u8],
    archive: &crate::sevenz::decoder::SevenZArchive<'_>,
    folder: &SevenZFolder,
    index: usize,
) -> Result<Vec<u8>, TTZipStatus> {
    let signature = crate::sevenz::format::SevenZSignatureHeader::parse(mapped)?;
    let end_of_payload = usize::try_from(signature.next_header_offset)
        .ok()
        .and_then(|offset| offset.checked_add(32))
        .and_then(|end| mapped.get(..end))
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    let packed_end = folder
        .packed_offset
        .checked_add(folder.packed_len)
        .ok_or(TTZipStatus::ErrCorruptHeader)?;
    if folder.packed_offset < 32
        || packed_end > end_of_payload.len()
        || folder.unpack_sizes.as_slice() != [folder.packed_len as u64]
    {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    crate::archive::stream_adapter::extract_7z_single_entry_from_reader_fast(
        &mut std::io::Cursor::new(end_of_payload),
        archive.info(),
        archive.seek_index(),
        index,
        None,
    )
}

fn extract_7z_entry_by_path_bounded(
    mapped: &[u8],
    entry_path: &str,
    password: Option<&str>,
    budget_bytes: u64,
) -> Result<Vec<u8>, TTZipError> {
    let archive = open_sevenz_slice_for_preview(mapped, password)?;
    let index = resolve_7z_entry_index(archive.files(), Some(entry_path), -1)?;
    let info = archive.info();
    let seek_index = archive.seek_index();
    if let Some(loc) = seek_index.get_by_index(index) {
        let stored_folder = loc
            .folder_index
            .and_then(|folder| info.folders.get(folder))
            .filter(|folder| {
                loc.offset_in_folder > budget_bytes
                    && crate::archive::stream_adapter::is_unencrypted_copy_folder(info, folder)
                    && matches!(folder.coders.as_slice(), [coder]
                        if coder.num_in_streams == 1
                            && coder.num_out_streams == 1
                            && coder.properties.is_empty())
            });
        if let Some(folder) = stored_folder {
            enforce_7z_entry_budget(loc, 0)?;
            return read_stored_7z_preview(mapped, &archive, folder, index)
                .map_err(|s| map_single_entry_status(s, entry_path));
        }
        enforce_7z_entry_budget(loc, budget_bytes)?;
    }
    crate::sevenz::decoder::stream::extract_entry_bytes_stream_bounded(
        mapped, info, seek_index, index, password, budget_bytes,
    )
    .map_err(|s| map_single_entry_status(s, entry_path))
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
    let volume_chain = crate::archive::split::detect_volume_chain(p)
        .map_err(|_| TTZipError::FileNotFound {
            path: archive_path.clone(),
        })?;
    if volume_chain.len() > 1 {
        return extract_single_entry_split_guarded(
            &volume_chain,
            p,
            Some(&entry_path),
            -1,
            secure_pwd.as_deref(),
            64,
        );
    }
    if !p.exists() {
        return Err(TTZipError::FileNotFound { path: archive_path });
    }

    let source = crate::archive::source::open_archive_source(p)
        .map_err(|e| TTZipError::IoError { message: e.to_string() })?;
    let mapped = source.as_slice().ok_or_else(|| TTZipError::IoError {
        message: "Failed to map archive bytes".to_string(),
    })?;

    let clean_target = entry_path.trim_start_matches('/');
    const DEFAULT_EXTRACTION_BUDGET_BYTES: u64 = 64 * 1024 * 1024;

    if mapped.starts_with(b"7z\xBC\xAF\x27\x1C") {
        extract_7z_entry_by_path_bounded(
            mapped,
            &entry_path,
            secure_pwd.as_deref(),
            DEFAULT_EXTRACTION_BUDGET_BYTES,
        )
    } else if let Ok(zip_archive) = crate::zip::reader::ZipArchive::open_slice(mapped) {
        extract_zip_entry_by_path_bounded(
            &zip_archive,
            clean_target,
            &entry_path,
            secure_pwd.as_deref(),
            DEFAULT_EXTRACTION_BUDGET_BYTES,
        )
    } else {
        crate::archive::unified::extract_single::extract_single_entry_memory(
            p,
            Some(&entry_path),
            -1,
            secure_pwd.as_deref(),
        )
        .map_err(|s| map_single_entry_status(s, &entry_path))
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

#[cfg(test)]
mod solid_preview_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Seek, SeekFrom, Write};

    const STORED_PREFIX_BYTES: u64 = 65 * 1024 * 1024;
    const STORED_TARGET: &[u8] = b"stored preview";

    fn write_stored_preview_archive(path: &Path, target_crc: u32, packed_len: u64) {
        use crate::crypto::crc32::crc32_fast;
        use crate::sevenz::format::{SevenZSignatureHeader, METHOD_COPY};
        use crate::sevenz::writer::build_7z_metadata_header;
        use crate::zip::writer::ZipInputItem;

        let items = ["prefix.bin", "target.txt"].map(|rel_path| ZipInputItem {
            rel_path: rel_path.to_string(),
            data: vec![1],
            mtime_epoch_secs: 0,
            mode: 0o100644,
            is_directory: false,
        });
        let zeros = vec![0; 1024 * 1024];
        let prefix_crc = (0..65).fold(0, |crc, _| crc32_fast(crc, &zeros));
        let unpacked_len = STORED_PREFIX_BYTES + STORED_TARGET.len() as u64;
        let header = build_7z_metadata_header(
            &items,
            &[STORED_PREFIX_BYTES, STORED_TARGET.len() as u64],
            &[prefix_crc, target_crc],
            packed_len,
            unpacked_len,
            METHOD_COPY,
            &[],
        );
        let signature = SevenZSignatureHeader {
            major_version: 0,
            minor_version: 4,
            start_header_crc: 0,
            next_header_offset: unpacked_len,
            next_header_size: header.len() as u64,
            next_header_crc: crc32_fast(0, &header),
        };
        let mut file = std::fs::File::create(path).expect("create archive");
        file.write_all(&signature.serialize()).expect("write signature");
        file.seek(SeekFrom::Start(32 + STORED_PREFIX_BYTES))
            .expect("seek to target");
        file.write_all(STORED_TARGET).expect("write target");
        file.write_all(&header).expect("write header");
    }

    #[test]
    fn single_volume_stored_preview_seeks_past_budget() {
        let temp = tempfile::tempdir().expect("tempdir");
        let archive = temp.path().join("stored.7z");
        let target_crc = crate::crypto::crc32::crc32_fast(0, STORED_TARGET);
        write_stored_preview_archive(
            &archive,
            target_crc,
            STORED_PREFIX_BYTES + STORED_TARGET.len() as u64,
        );
        let path = archive.to_string_lossy().into_owned();
        let bytes = extract_single_entry_by_path(path.clone(), "target.txt".to_string(), None)
            .expect("stored preview can seek beyond the decompression budget");
        assert_eq!(bytes, STORED_TARGET);
        assert!(matches!(
            extract_single_entry_by_path(path, "prefix.bin".to_string(), None),
            Err(TTZipError::OutOfMemory { .. })
        ));
    }

    #[test]
    fn single_volume_stored_preview_rejects_invalid_bytes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let archive = temp.path().join("stored.7z");
        let target_crc = crate::crypto::crc32::crc32_fast(0, STORED_TARGET);
        let pack_len = STORED_PREFIX_BYTES + STORED_TARGET.len() as u64;
        let extract = || {
            extract_single_entry_by_path(
                archive.to_string_lossy().into_owned(),
                "target.txt".to_string(),
                None,
            )
        };

        write_stored_preview_archive(&archive, target_crc ^ 1, pack_len);
        assert!(matches!(extract(), Err(TTZipError::CorruptHeader { .. })));

        write_stored_preview_archive(&archive, target_crc, pack_len + 1);
        assert!(matches!(extract(), Err(TTZipError::CorruptHeader { .. })));

        write_stored_preview_archive(&archive, target_crc, pack_len);
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&archive)
            .expect("open archive for truncation");
        file.set_len(file.metadata().expect("original length").len() - 1)
            .expect("truncate archive");
        assert!(matches!(extract(), Err(TTZipError::CorruptHeader { .. })));
    }

    fn create_sparse_split_7z_volumes(
        dir: & Path,
        prefix_len: u64,
        jpeg_bytes: &[u8],
    ) -> std::path::PathBuf {
        use crate::crypto::crc32::crc32_fast;
        use crate::sevenz::format::{SevenZSignatureHeader, METHOD_COPY};
        use crate::sevenz::writer::build_7z_metadata_header;
        use crate::zip::writer::ZipInputItem;

        let items = vec![
            ZipInputItem {
                rel_path: "2026.7.31 哲学在证明开始以前/通用4k.mp4".to_string(),
                data: vec![1],
                mtime_epoch_secs: 1_700_000_000,
                mode: 0o100644,
                is_directory: false,
            },
            ZipInputItem {
                rel_path: "2026.7.31 哲学在证明开始以前/布劳威尔.jpeg".to_string(),
                data: vec![1],
                mtime_epoch_secs: 1_700_000_000,
                mode: 0o100644,
                is_directory: false,
            },
        ];
        let stream_sizes = [prefix_len, jpeg_bytes.len() as u64];
        let stream_crcs = [0x1234_5678, crc32_fast(0, jpeg_bytes)];
        let total_uncompressed = prefix_len + jpeg_bytes.len() as u64;
        let end_header = build_7z_metadata_header(
            &items,
            &stream_sizes,
            &stream_crcs,
            total_uncompressed,
            total_uncompressed,
            METHOD_COPY,
            &[],
        );
        let sig_header = SevenZSignatureHeader {
            major_version: 0,
            minor_version: 4,
            start_header_crc: 0,
            next_header_offset: total_uncompressed,
            next_header_size: end_header.len() as u64,
            next_header_crc: crc32_fast(0, &end_header),
        }
        .serialize();

        let vol_size = 5 * 1024 * 1024 * 1024u64; // 5 GiB sparse volumes
        let vol1 = dir.join("2026.7.31 哲学在证明开始以前.7z.001");
        let vol2 = dir.join("2026.7.31 哲学在证明开始以前.7z.002");
        let vol3 = dir.join("2026.7.31 哲学在证明开始以前.7z.003");

        let mut f1 = std::fs::File::create(&vol1).expect("create vol1");
        f1.write_all(&sig_header).expect("write sig_header");
        f1.set_len(vol_size).expect("set_len vol1");

        let f2 = std::fs::File::create(&vol2).expect("create vol2");
        f2.set_len(vol_size).expect("set_len vol2");

        let mut f3 = std::fs::File::create(&vol3).expect("create vol3");
        let offset_in_v3 = (32 + prefix_len) - 2 * vol_size;
        f3.seek(SeekFrom::Start(offset_in_v3)).expect("seek vol3");
        f3.write_all(jpeg_bytes).expect("write jpeg");
        f3.write_all(&end_header).expect("write end_header");
        vol1
    }

    #[test]
    fn test_issue_304_split_7z_preview_session_and_single_entry_past_10gib() {
        use crate::uniffi_api::extraction_session::{
            CollisionPolicy, CommitResult, ExtractionRequest, ExtractionSession, ResolutionSet,
        };

        let tmp = tempfile::tempdir().expect("tempdir");
        let prefix_12_gib = 12 * 1024 * 1024 * 1024u64;
        let jpeg_bytes: Vec<u8> = (0..9441).map(|i| ((i * 41 + 19) & 0xFF) as u8).collect();
        let vol1 = create_sparse_split_7z_volumes(tmp.path(), prefix_12_gib, &jpeg_bytes);
        let entry_rel = "2026.7.31 哲学在证明开始以前/布劳威尔.jpeg".to_string();

        let mem_bytes = extract_single_entry_by_path(
            vol1.to_string_lossy().into_owned(),
            entry_rel.clone(),
            None,
        )
        .expect("extract_single_entry_by_path on split 7z past 12 GiB must succeed");
        assert_eq!(mem_bytes, jpeg_bytes);

        let dest_dir = tmp.path().join("preview_out");
        std::fs::create_dir_all(&dest_dir).expect("create preview_out");
        let req = ExtractionRequest {
            archive_path: vol1.to_string_lossy().into_owned(),
            destination_root: dest_dir.to_string_lossy().into_owned(),
            password: None,
            entry_filter: Some(vec![entry_rel.clone()]),
            root_at_common_parent: false,
            policy: CollisionPolicy::KeepBoth,
        };
        let session = ExtractionSession::new(req).expect("create ExtractionSession");
        let stage_report = session
            .stage(None, None)
            .expect("ExtractionSession::stage past 12 GiB split 7z prefix must not fail with -403");
        assert_eq!(stage_report.staged_items, 1);
        let commit_res = session
            .commit(ResolutionSet {
                default_action: None,
                overrides: Vec::new(),
            })
            .expect("ExtractionSession::commit must succeed");
        assert!(matches!(commit_res, CommitResult::Finished { .. }));
        assert_eq!(
            std::fs::read(dest_dir.join(&entry_rel)).expect("read extracted jpeg"),
            jpeg_bytes
        );
        verify_split_7z_repro_if_configured();
    }

    fn verify_split_7z_repro_if_configured() {
        use crate::crypto::crc32::crc32_fast;
        use crate::uniffi_api::extraction_session::{
            CollisionPolicy, CommitResult, ExtractionRequest, ExtractionSession, ResolutionSet,
        };

        let Ok(repro_path) = std::env::var("TTZIP_ISSUE_304_REPRO_ARCHIVE") else {
            return;
        };
        if !Path::new(&repro_path).exists() {
            return;
        }
        let target_a = "2026.7.31 哲学在证明开始以前/布劳威尔.jpeg".to_string();
        let target_b = "2026.7.31 哲学在证明开始以前/图片/希尔伯特.jpg".to_string();

        let bytes_a = extract_single_entry_by_path(repro_path.clone(), target_a.clone(), None)
            .expect("repro extract_single_entry_by_path target_a");
        assert_eq!(bytes_a.len(), 9441);
        assert_eq!(crc32_fast(0, &bytes_a), 0xFE16_9E1C);

        let tmp = tempfile::tempdir().expect("repro tempdir");
        let req = ExtractionRequest {
            archive_path: repro_path,
            destination_root: tmp.path().to_string_lossy().into_owned(),
            password: None,
            entry_filter: Some(vec![target_a.clone(), target_b.clone()]),
            root_at_common_parent: false,
            policy: CollisionPolicy::KeepBoth,
        };
        let session = ExtractionSession::new(req).expect("repro ExtractionSession::new");
        let report = session.stage(None, None).expect("repro ExtractionSession::stage");
        assert_eq!(report.staged_items, 2);
        let commit_res = session
            .commit(ResolutionSet {
                default_action: None,
                overrides: Vec::new(),
            })
            .expect("repro ExtractionSession::commit");
        assert!(matches!(commit_res, CommitResult::Finished { .. }));
        let disk_a = std::fs::read(tmp.path().join(&target_a)).expect("read disk_a");
        let disk_b = std::fs::read(tmp.path().join(&target_b)).expect("read disk_b");
        assert_eq!(disk_a, bytes_a);
        assert_eq!(disk_b.len(), 269858);
        assert_eq!(crc32_fast(0, &disk_b), 0xB6C6_EFD9);
    }
}

