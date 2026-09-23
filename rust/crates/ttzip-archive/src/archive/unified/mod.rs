// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Unified Archive Lifecycle Orchestrator Engine.
//!
//! Supports:
//! 1. Zero-copy / stream auto-detection across 17 archive/compression formats.
//! 2. Single-entry and multi-file recursive archive creation with optional encryption and split volumes.
//! 3. Two-stage secure extraction with ZipSlip defense, APFS extent preallocation, and bottom-up metadata.
//! 4. Non-destructive archive inspection with optional automatic charset detection.
//! 5. Self-healing archive repair integrating RS-FEC streaming and damaged header recovery.

pub mod create;
pub mod detect;
pub mod entry;
pub mod extract;
pub mod extract_single;
pub mod filter_pipeline;
pub mod format_config;
pub mod format_sniffer;
pub mod inspect;
pub mod repair;
pub mod stream_adapter;
pub mod verify;
#[cfg(test)]
pub mod tests_detection;
#[cfg(test)]
pub mod tests_lifecycle;

pub use create::{create_archive, create_archive_to_writer};
pub use detect::{detect_archive_sniff, detect_format, resolve_create_format};
pub use entry::{
    coalesce_sparse_extents, clean_sparse_extents, EntryFields, LinkAction, LinkResolver,
    LinkResolverStrategy, SparseExtent, TTZipEntry, TTZipFileType, TTZipTimestamp,
};
pub use extract::{extract_archive, extract_archive_with_metrics};
pub use extract_single::{extract_selected_entries, extract_single_entry_memory};
pub use filter_pipeline::{
    FilterKind, FilterPipeline, FilterPipelineError, FilterPipelineResult,
    StreamFilter, MAX_FILTER_CHAIN_DEPTH,
};
pub use format_sniffer::{
    ArchiveFormat, ArchiveFormatBidder, BidResult, BidScore, FormatBidderRegistry, FormatSniffer,
    SeekableStream, SniffAnchor, SniffResult, SniffRule, SNIFF_RULES,
};
pub use inspect::{inspect_archive, inspect_archive_c_compat, ArchiveEntryRef};
pub use repair::repair_archive;
pub use stream_adapter::{
    LookaheadRead, SlidingLookaheadReader, DEFAULT_INITIAL_LOOKAHEAD_CAPACITY,
    MAX_LOOKAHEAD_CAPACITY, MICRO_BUFFER_CHUNK_SIZE,
};
pub use verify::{verify_archive_stream, ArchiveIntegrityReport, CorruptedEntryDetail};

use std::path::{Path, PathBuf};

use crate::standards::signatures::{CompoundFormat, DetectedFormat};
use crate::types::{
    TTZipArchiveFormat, TTZipCreateOptions, TTZipExtractOptions, TTZipInspectCallback, TTZipStatus,
};
use libc::c_void;

/// Unified Archive Orchestrator Engine.
pub struct UnifiedArchiveOrchestrator;

impl UnifiedArchiveOrchestrator {
    /// Three-state sniffing across 50+ archive, disk image, and compressed formats from memory buffer.
    #[inline]
    pub fn sniff_format(buffer: &[u8]) -> SniffResult {
        FormatSniffer::sniff(buffer)
    }

    /// Three-state sniffing across 50+ archive, disk image, and compressed formats from file.
    #[inline]
    pub fn sniff_file(path: &Path) -> std::io::Result<SniffResult> {
        FormatSniffer::sniff_file(path)
    }

    /// Detects format of an archive from file headers with extension fallback.
    #[inline]
    pub fn detect_format(path: &Path) -> Result<(DetectedFormat, Option<CompoundFormat>), TTZipStatus> {
        detect::detect_format(path)
    }

    /// Resolves target archive creation format from explicit format and destination path.
    #[inline]
    pub fn resolve_create_format(format: TTZipArchiveFormat, dest_path: &Path) -> TTZipArchiveFormat {
        detect::resolve_create_format(format, dest_path)
    }

    /// Recursively compresses source paths into the destination archive path.
    #[inline]
    pub fn create_archive(
        source_paths: &[PathBuf],
        destination_path: &Path,
        options: &TTZipCreateOptions,
        split_volume_size_bytes: u64,
    ) -> Result<(), TTZipStatus> {
        create::create_archive(source_paths, destination_path, options, split_volume_size_bytes)
    }

    /// Recursively compresses explicit pre-collected entries into the destination archive path.
    #[inline]
    pub fn create_archive_explicit(
        entries: &[(PathBuf, String)],
        destination_path: &Path,
        options: &TTZipCreateOptions,
        split_volume_size_bytes: u64,
    ) -> Result<(), TTZipStatus> {
        create::create_archive_explicit(entries, destination_path, options, split_volume_size_bytes)
    }

    /// Recursively compresses source paths into a streaming writer (0% disk write amplification).
    #[inline]
    pub fn create_archive_to_writer<W: std::io::Write>(
        source_paths: &[PathBuf],
        writer: W,
        options: &TTZipCreateOptions,
    ) -> Result<u64, TTZipStatus> {
        create::create_archive_to_writer(source_paths, writer, options)
    }

    /// Extracts an archive to destination directory with security verification and APFS preallocation.
    #[inline]
    pub fn extract_archive(
        archive_path: &Path,
        destination_path: &Path,
        options: &TTZipExtractOptions,
    ) -> Result<(), TTZipStatus> {
        extract::extract_archive(archive_path, destination_path, options)
    }

    /// Extracts an archive and returns total extracted uncompressed bytes.
    #[inline]
    pub fn extract_archive_with_metrics(
        archive_path: &Path,
        destination_path: &Path,
        options: &TTZipExtractOptions,
    ) -> Result<u64, TTZipStatus> {
        extract::extract_archive_with_metrics(archive_path, destination_path, options)
    }

    /// Inspects an archive and invokes the callback for every discovered entry metadata item.
    #[inline]
    pub fn inspect_archive(
        archive_path: &Path,
        password: Option<&str>,
        detect_encoding: bool,
        callback: TTZipInspectCallback,
        user_data: *mut c_void,
    ) -> Result<usize, TTZipStatus> {
        inspect::inspect_archive_c_compat(archive_path, password, detect_encoding, callback, user_data)
    }

    /// Inspects an archive using pure Safe Rust closure with zero-copy borrowed `ArchiveEntryRef`.
    #[inline]
    pub fn inspect_archive_safe<F>(
        archive_path: &Path,
        password: Option<&str>,
        detect_encoding: bool,
        callback: F,
    ) -> Result<usize, TTZipStatus>
    where
        F: FnMut(&ArchiveEntryRef<'_>) -> bool,
    {
        inspect::inspect_archive(archive_path, password, detect_encoding, callback)
    }

    /// Auto-detects format and repairs damaged archive.
    #[inline]
    pub fn repair_archive(
        damaged_path: &Path,
        repaired_path: &Path,
    ) -> Result<usize, TTZipStatus> {
        repair::repair_archive(damaged_path, repaired_path)
    }

    /// Batch selective extraction to destination directory.
    #[inline]
    pub fn extract_selected(
        archive_path: &Path,
        target_paths: &[String],
        destination_dir: &Path,
        options: &TTZipExtractOptions,
    ) -> Result<usize, TTZipStatus> {
        extract_single::extract_selected_entries(archive_path, target_paths, destination_dir, options)
    }
}
