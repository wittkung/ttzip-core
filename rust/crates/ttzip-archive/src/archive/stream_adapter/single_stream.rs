// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Unified single-file compressed stream and TAR-compressed stream routing adapter.
//!
//! Enforces the R2 Read Routing Contract:
//! 1. Single-volume and multi-volume streams share identical codec detection and decompression.
//! 2. Decompressed streams are inspected via a 512-byte lookahead (`PreReadStream`) to strictly
//!    distinguish TAR-compressed containers (`.tar.br`, `.tar.zst`, `.tar.gz`, etc.) from pure
//!    single-file compressed streams (`.gz`, `.bz2`, `.xz`, `.zst`, `.lz4`, `.br`, `.lz`, `.lrz`, `.sz`).
//! 3. Pure single-file compressed streams expose a consistent logical entry identity (`<stem>`)
//!    across `inspect_archive`, `extract_archive`, `extract_selected_entries`, and
//!    `extract_single_entry_memory`, while preserving `DirJail`, `InFlightJailFile`, and
//!    `SafeExtractEngine` security guarantees.

use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use libc::c_void;

use super::read::ArchiveStreamReader;
use crate::archive::split::VirtualMultiVolumeReader;
use crate::archive::unified::inspect::ArchiveEntryRef;
use crate::archive::unified::selection::EntrySelection;
use crate::ffi::archive_ffi::sys::{archive_read_data, archive_read_next_header};
use crate::fs::abort_guard::InFlightJailFile;
use crate::fs::safe_extract::{sanitize_and_validate_path, DirJail, SafeExtractEngine};
use crate::security::path_sanitizer::ExpansionRatioGuard;
use crate::tar::header::TarHeader;
use crate::types::{ProgressAtomicSheet, TTZipExtractOptions, TTZipStatus};

const STREAM_IO_BUF_SIZE: usize = 65536;
const MAX_IN_MEMORY_STREAM_BYTES: usize = 64 * 1024 * 1024;

/// Confirmed outer compression stream codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmedStreamCodec {
    Gzip,
    Bzip2,
    Xz,
    Zstd,
    Lz4,
    Brotli,
    Lzip,
    Lrzip,
    Snappy,
    Lzfse,
}

impl ConfirmedStreamCodec {
    #[inline]
    pub const fn method_id(self) -> u16 {
        match self {
            Self::Gzip => 8,
            Self::Bzip2 => 12,
            Self::Xz | Self::Lzip | Self::Lrzip => 14,
            Self::Zstd => 93,
            Self::Lz4 => 94,
            Self::Brotli => 95,
            Self::Snappy => 96,
            Self::Lzfse => 97,
        }
    }
}

/// Resolved classification after opening and peeking the first 512 decompressed bytes.
pub enum ResolvedStreamKind {
    /// Decompressed stream is a valid POSIX/GNU/V7 TAR archive.
    TarContainer(PreReadStream),
    /// Decompressed stream is a pure single-file payload with canonical `<stem>` entry identity.
    SingleFileStream {
        stream: PreReadStream,
        entry_name: String,
        compressed_size: u64,
        mtime_epoch_secs: i64,
        codec: ConfirmedStreamCodec,
    },
}

/// Strips a trailing numeric or spanned split suffix (`.001`, `.z01`) while preserving case.
#[inline]
pub fn strip_split_suffix_preserve_case(name: &str) -> &str {
    if let Some((stem, ext)) = name.rsplit_once('.') {
        let is_num = ext.len() >= 3 && ext.bytes().all(|b| b.is_ascii_digit());
        let is_z = (ext.starts_with('z') || ext.starts_with('Z'))
            && ext.len() >= 3
            && ext.as_bytes()[1..].iter().all(|b| b.is_ascii_digit());
        if is_num || is_z {
            return stem;
        }
    }
    name
}

/// Derives the canonical logical entry name (`<stem>`) for a pure single-file compressed stream.
pub fn derive_single_stream_entry_name(archive_path: &Path) -> String {
    let raw_name = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("data");
    let base_name = strip_split_suffix_preserve_case(raw_name);
    let lower = base_name.to_ascii_lowercase();

    const STREAM_EXTS: &[&str] = &[
        ".gzip", ".bzip2", ".brotli", ".snappy", ".lrzip", ".lzfse", ".zstd", ".lzip", ".bz2",
        ".zst", ".lz4", ".lrz", ".gz", ".xz", ".br", ".lz", ".sz",
    ];
    for ext in STREAM_EXTS {
        if lower.ends_with(ext) && base_name.len() > ext.len() {
            let candidate = &base_name[..base_name.len() - ext.len()];
            if !candidate.is_empty() {
                return candidate.to_string();
            }
        }
    }
    if !base_name.is_empty() {
        base_name.to_string()
    } else {
        "data".to_string()
    }
}

/// Returns `true` if `base_lower` (after stripping split suffix) has a compound TAR extension.
#[inline]
pub fn has_compound_tar_extension(base_lower: &str) -> bool {
    const TAR_COMPOUND_EXTS: &[&str] = &[
        ".tar.gz", ".tgz", ".tar.bz2", ".tbz2", ".tbz", ".tar.xz", ".txz", ".tar.zst", ".tzst",
        ".tar.lz4", ".tlz4", ".tar.br", ".tbr", ".tar.lz", ".tlz", ".tar.lrz", ".tlrz", ".tar.sz",
    ];
    TAR_COMPOUND_EXTS
        .iter()
        .any(|ext| base_lower.ends_with(ext))
}

#[inline]
fn has_strict_magic_stream_extension(base_lower: &str) -> bool {
    const EXTS: &[&str] = &[
        ".gz", ".gzip", ".tgz", ".bz2", ".bzip2", ".tbz2", ".tbz", ".xz", ".txz", ".zst", ".zstd",
        ".tzst", ".lz4", ".tlz4", ".lz", ".lzip", ".tlz", ".lrz", ".lrzip", ".tlrz", ".sz",
        ".snappy", ".lzfse",
    ];
    EXTS.iter().any(|ext| base_lower.ends_with(ext))
}

#[inline]
fn is_known_container_magic(head: &[u8]) -> bool {
    if head.len() >= 6 && &head[..6] == b"7z\xBC\xAF\x27\x1C" {
        return true;
    }
    if head.len() >= 4
        && matches!(
            &head[..4],
            b"PK\x03\x04"
                | b"PK\x05\x06"
                | b"PK\x07\x08"
                | b"Rar!"
                | b"xar!"
                | b"MSCF"
                | b"MSWI"
                | b"!<ar"
                | b"0707"
                | b"pbze"
                | b"AA01"
                | b"YAA1"
                | b"AEA1"
                | b"hsqs"
                | b"\xED\xAB\xEE\xDB"
        )
    {
        return true;
    }
    false
}

fn sniff_magic_stream_codec(head: &[u8]) -> Option<ConfirmedStreamCodec> {
    if head.starts_with(b"\x1F\x8B\x08") {
        return Some(ConfirmedStreamCodec::Gzip);
    }
    if head.len() >= 4 && &head[..3] == b"BZh" && (b'1'..=b'9').contains(&head[3]) {
        return Some(ConfirmedStreamCodec::Bzip2);
    }
    if head.starts_with(b"\xFD7zXZ\x00") {
        return Some(ConfirmedStreamCodec::Xz);
    }
    if head.starts_with(b"\x28\xB5\x2F\xFD") {
        return Some(ConfirmedStreamCodec::Zstd);
    }
    if head.starts_with(b"\x04\x22\x4D\x18") || head.starts_with(b"\x02\x21\x4C\x18") {
        return Some(ConfirmedStreamCodec::Lz4);
    }
    if head.starts_with(b"\xFF\x06\x00\x00sNaPpY") {
        return Some(ConfirmedStreamCodec::Snappy);
    }
    sniff_secondary_magic_codec(head)
}

fn sniff_secondary_magic_codec(head: &[u8]) -> Option<ConfirmedStreamCodec> {
    let prefix4: &[u8; 4] = head.get(..4)?.try_into().ok()?;
    match prefix4 {
        b"LZIP" => Some(ConfirmedStreamCodec::Lzip),
        b"LRZI" => Some(ConfirmedStreamCodec::Lrzip),
        b"bvx2" | b"bvx1" | b"bvxn" | b"bvx-" => Some(ConfirmedStreamCodec::Lzfse),
        _ => None,
    }
}

fn is_brotli_extension(base_lower: &str) -> bool {
    base_lower.ends_with(".tar.br")
        || base_lower.ends_with(".tbr")
        || base_lower.ends_with(".br")
        || base_lower.ends_with(".brotli")
}

/// Classifies a file or volume chain into a `ConfirmedStreamCodec` based on magic bytes and filename.
pub fn classify_stream_codec(
    archive_path: &Path,
    head: &[u8],
) -> Result<Option<ConfirmedStreamCodec>, TTZipStatus> {
    if let Some(codec) = sniff_magic_stream_codec(head) {
        return Ok(Some(codec));
    }
    let raw_name = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let base_lower = strip_split_suffix_preserve_case(&raw_name);

    if is_brotli_extension(base_lower) {
        if head.is_empty() {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        if is_known_container_magic(head) {
            return Ok(None);
        }
        return Ok(Some(ConfirmedStreamCodec::Brotli));
    }

    if has_strict_magic_stream_extension(base_lower) && !is_known_container_magic(head) {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    Ok(None)
}

/// Safe `Read` adapter over `libarchive`'s raw-filter pipeline for `.bz2`, `.xz`, `.lz4`, `.lz`, `.lrz`.
pub struct LibarchiveRawFilterReader<R> {
    stream: ArchiveStreamReader<R>,
    eof: bool,
}

impl<R: Read + 'static> LibarchiveRawFilterReader<R> {
    pub fn new(reader: R) -> Result<Self, TTZipStatus> {
        let stream = ArchiveStreamReader::open_raw_filter(reader, STREAM_IO_BUF_SIZE)?;
        let mut entry: *mut c_void = std::ptr::null_mut();
        let rc = unsafe { archive_read_next_header(stream.as_raw_archive(), &mut entry) };
        if rc != 0 {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        Ok(Self { stream, eof: false })
    }
}

impl<R> Read for LibarchiveRawFilterReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.eof || buf.is_empty() {
            return Ok(0);
        }
        let r = unsafe {
            archive_read_data(
                self.stream.as_raw_archive(),
                buf.as_mut_ptr() as *mut c_void,
                buf.len(),
            )
        };
        if r < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "corrupted compressed filter stream",
            ));
        }
        if r == 0 {
            self.eof = true;
            return Ok(0);
        }
        Ok(r as usize)
    }
}

/// Stream wrapper that buffers the first `N` decompressed bytes for container header inspection
/// and seamlessly replays them on subsequent `Read` calls.
pub struct PreReadStream {
    prefix: Vec<u8>,
    pos: usize,
    inner: Box<dyn Read>,
}

impl PreReadStream {
    pub fn new(mut inner: Box<dyn Read>, target_peek: usize) -> Result<Self, TTZipStatus> {
        let mut prefix = Vec::with_capacity(target_peek);
        let mut scratch = [0u8; 512];
        while prefix.len() < target_peek {
            let want = (target_peek - prefix.len()).min(scratch.len());
            match inner.read(&mut scratch[..want]) {
                Ok(0) => break,
                Ok(n) => prefix.extend_from_slice(&scratch[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(TTZipStatus::ErrCorruptHeader),
            }
        }
        Ok(Self {
            prefix,
            pos: 0,
            inner,
        })
    }

    #[inline]
    pub fn peeked(&self) -> &[u8] {
        &self.prefix
    }
}

impl Read for PreReadStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos < self.prefix.len() {
            let avail = &self.prefix[self.pos..];
            let n = avail.len().min(buf.len());
            buf[..n].copy_from_slice(&avail[..n]);
            self.pos += n;
            return Ok(n);
        }
        self.inner.read(buf)
    }
}

#[inline]
pub fn is_valid_tar_header_block(peeked: &[u8]) -> bool {
    let Some(hdr) = TarHeader::from_slice(peeked) else {
        return false;
    };
    if !hdr.verify_checksum() {
        return false;
    }
    hdr.is_ustar() || hdr.is_gnu() || !hdr.name().is_empty()
}

fn wrap_codec_decompressor<R: Read + 'static>(
    reader: R,
    codec: ConfirmedStreamCodec,
) -> Result<Box<dyn Read>, TTZipStatus> {
    match codec {
        ConfirmedStreamCodec::Gzip => Ok(Box::new(flate2::read::MultiGzDecoder::new(
            BufReader::with_capacity(STREAM_IO_BUF_SIZE, reader),
        ))),
        ConfirmedStreamCodec::Zstd => Ok(Box::new(crate::codecs::zstd::ZstdStreamReader::new(
            reader,
        )?)),
        ConfirmedStreamCodec::Brotli => Ok(Box::new(
            crate::codecs::brotli::BrotliDecompressorReader::new(reader, STREAM_IO_BUF_SIZE),
        )),
        ConfirmedStreamCodec::Snappy => Ok(Box::new(snap::read::FrameDecoder::new(reader))),
        ConfirmedStreamCodec::Lzfse => Ok(Box::new(
            crate::codecs::lzfse::LzfseReader::new(reader)
                .map_err(|_| TTZipStatus::ErrCorruptHeader)?,
        )),
        ConfirmedStreamCodec::Bzip2
        | ConfirmedStreamCodec::Xz
        | ConfirmedStreamCodec::Lz4
        | ConfirmedStreamCodec::Lzip
        | ConfirmedStreamCodec::Lrzip => Ok(Box::new(LibarchiveRawFilterReader::new(reader)?)),
    }
}

type OpenedRawStream = (usize, [u8; 16], u64, i64, Box<dyn Read>);

fn open_raw_stream_with_header(
    archive_path: &Path,
    volume_chain: &[PathBuf],
) -> Result<OpenedRawStream, TTZipStatus> {
    let first_path = volume_chain
        .first()
        .map(|p| p.as_path())
        .unwrap_or(archive_path);
    let mtime = extract_file_mtime_secs(first_path);
    let mut head = [0u8; 16];
    if volume_chain.len() > 1 {
        let mut vr = VirtualMultiVolumeReader::from_volumes(volume_chain.to_vec())
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let n = vr.read(&mut head).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let _ = vr.seek(SeekFrom::Start(0));
        let total = vr.total_size();
        Ok((n, head, total, mtime, Box::new(vr)))
    } else {
        let mut f = File::open(first_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
        let n = f.read(&mut head).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let _ = f.seek(SeekFrom::Start(0));
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        Ok((n, head, len, mtime, Box::new(f)))
    }
}

/// Attempts to open `archive_path` / `volume_chain` as a confirmed compressed stream and resolves
/// whether the inner payload is a `TarContainer` or a `SingleFileStream`.
pub fn try_open_resolved_stream(
    archive_path: &Path,
    volume_chain: &[PathBuf],
) -> Result<Option<ResolvedStreamKind>, TTZipStatus> {
    let (head_len, head, compressed_size, mtime_epoch_secs, raw_reader) =
        open_raw_stream_with_header(archive_path, volume_chain)?;

    let Some(codec) = classify_stream_codec(archive_path, &head[..head_len])? else {
        return Ok(None);
    };

    let decomp = wrap_codec_decompressor(raw_reader, codec)?;
    let pre = PreReadStream::new(decomp, 512)?;

    let raw_name = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let base_lower = strip_split_suffix_preserve_case(&raw_name);
    let peeked = pre.peeked();

    if has_compound_tar_extension(base_lower) {
        let is_zero_tar = peeked.len() == 512 && peeked.iter().all(|&b| b == 0);
        if !is_valid_tar_header_block(peeked) && !is_zero_tar {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        return Ok(Some(ResolvedStreamKind::TarContainer(pre)));
    }

    if is_valid_tar_header_block(peeked) {
        return Ok(Some(ResolvedStreamKind::TarContainer(pre)));
    }

    if codec == ConfirmedStreamCodec::Brotli && peeked.is_empty() && compressed_size > 4 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }

    Ok(Some(ResolvedStreamKind::SingleFileStream {
        stream: pre,
        entry_name: derive_single_stream_entry_name(archive_path),
        compressed_size,
        mtime_epoch_secs,
        codec,
    }))
}

fn extract_file_mtime_secs(path: &Path) -> i64 {
    path.metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Inspects a pure single-file compressed stream by draining it to verify integrity and compute
/// exact `uncompressed_size` and `crc32`.
pub fn inspect_single_file_stream<F>(
    mut stream: PreReadStream,
    entry_name: &str,
    compressed_size: u64,
    mtime_epoch_secs: i64,
    codec: ConfirmedStreamCodec,
    mut callback: F,
) -> Result<usize, TTZipStatus>
where
    F: FnMut(&ArchiveEntryRef<'_>) -> bool,
{
    let bomb_guard = ExpansionRatioGuard::default();
    let mut hasher = crc32fast::Hasher::new();
    let mut total_uncompressed = 0u64;
    let mut buf = [0u8; STREAM_IO_BUF_SIZE];

    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                total_uncompressed = total_uncompressed.saturating_add(n as u64);
                bomb_guard.check(total_uncompressed, compressed_size)?;
                hasher.update(&buf[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(TTZipStatus::ErrCorruptHeader),
        }
    }

    let entry_ref = ArchiveEntryRef {
        path: entry_name,
        raw_path: entry_name.as_bytes(),
        uncompressed_size: total_uncompressed,
        compressed_size,
        crc32: hasher.finalize(),
        mtime_epoch_secs,
        mode: 0o100644,
        is_directory: false,
        is_encrypted: false,
        compression_method: codec.method_id(),
        detected_encoding: None,
    };
    let _ = callback(&entry_ref);
    Ok(1)
}

fn report_single_stream_progress(
    active_sheet: &ProgressAtomicSheet,
    options: &TTZipExtractOptions,
    c_name: &CStr,
    n: u64,
) -> Result<(), TTZipStatus> {
    let done = active_sheet.add_processed(n);
    if let Some(cb) = options.progress_callback {
        let target = active_sheet.total_bytes.load(Ordering::Relaxed).max(done);
        if !unsafe { cb(done, target, c_name.as_ptr(), options.user_data) } {
            active_sheet.cancel();
            return Err(TTZipStatus::Cancelled);
        }
    }
    Ok(())
}

fn pump_single_stream_to_inflight(
    stream: &mut PreReadStream,
    inflight: &mut Option<InFlightJailFile>,
    compressed_size: u64,
    active_sheet: &ProgressAtomicSheet,
    options: &TTZipExtractOptions,
    c_name: &CStr,
) -> Result<u64, TTZipStatus> {
    let bomb_guard = ExpansionRatioGuard::default();
    let mut total_written = 0u64;
    let mut buf = [0u8; STREAM_IO_BUF_SIZE];
    loop {
        if active_sheet.is_cancelled() {
            return Err(TTZipStatus::Cancelled);
        }
        match stream.read(&mut buf) {
            Ok(0) => return Ok(total_written),
            Ok(n) => {
                total_written = total_written.saturating_add(n as u64);
                bomb_guard.check(total_written, compressed_size)?;
                if let Some(ref mut f) = inflight {
                    f.file_mut()
                        .write_all(&buf[..n])
                        .map_err(|_| TTZipStatus::ErrExtractionFailed)?;
                }
                report_single_stream_progress(active_sheet, options, c_name, n as u64)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(TTZipStatus::ErrCorruptHeader),
        }
    }
}

/// Extracts a pure single-file compressed stream into `destination_path` using `DirJail`,
/// `InFlightJailFile`, `ExpansionRatioGuard`, and `SafeExtractEngine`.
pub fn extract_single_file_stream(
    mut stream: PreReadStream,
    entry_name: &str,
    compressed_size: u64,
    mtime_epoch_secs: i64,
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    let local_sheet = ProgressAtomicSheet::new();
    let active_sheet = sheet.unwrap_or(&local_sheet);
    if active_sheet.is_cancelled() {
        return Err(TTZipStatus::Cancelled);
    }

    let target_path = sanitize_and_validate_path(destination_path, entry_name)?;
    let jail = DirJail::open(destination_path)?;
    let mut engine = SafeExtractEngine::for_root(destination_path);
    let c_name = CString::new(entry_name).unwrap_or_default();
    let mut inflight = (!options.dry_run)
        .then(|| InFlightJailFile::open(&jail, entry_name, true))
        .transpose()?;

    let total_written = pump_single_stream_to_inflight(
        &mut stream,
        &mut inflight,
        compressed_size,
        active_sheet,
        options,
        &c_name,
    )?;

    if let Some(f) = inflight {
        f.commit(&jail)?;
        engine.register_entry_typed(target_path, 0o644, mtime_epoch_secs, 0, false, false);
        jail.verify_all_jail_symlinks()?;
        engine.apply_deferred_metadata(options.preserve_permissions)?;
    }
    active_sheet.mark_finished();
    Ok(total_written)
}

/// Extracts a pure single-file compressed stream when selected via `EntrySelection`.
pub fn extract_selected_single_file_stream(
    stream: PreReadStream,
    entry_name: &str,
    compressed_size: u64,
    mtime_epoch_secs: i64,
    selection: &EntrySelection,
    staging: &crate::fs::collision::staging::StagingRoot,
    options: &TTZipExtractOptions,
) -> Result<usize, TTZipStatus> {
    let Some(out_rel) = selection
        .output_path(entry_name)
        .or_else(|| selection.output_path("data"))
    else {
        return Ok(0);
    };
    let out_name = if out_rel == "data" {
        entry_name
    } else {
        out_rel
    };
    std::fs::create_dir_all(staging.path()).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    extract_single_file_stream(
        stream,
        out_name,
        compressed_size,
        mtime_epoch_secs,
        staging.path(),
        options,
        None,
    )?;
    Ok(1)
}

/// Reads a pure single-file compressed stream into an in-memory buffer when matched by path or index.
pub fn extract_memory_single_file_stream(
    mut stream: PreReadStream,
    entry_name: &str,
    compressed_size: u64,
    entry_path: Option<&str>,
    entry_index: i64,
) -> Result<Vec<u8>, TTZipStatus> {
    let matched = if entry_index >= 0 {
        entry_index == 0
    } else if let Some(target) = entry_path {
        let clean = target.trim_start_matches("./").trim_start_matches('/');
        clean == entry_name || clean == "data"
    } else {
        false
    };
    if !matched {
        return Err(TTZipStatus::ErrFileNotFound);
    }

    let bomb_guard = ExpansionRatioGuard::default();
    let mut payload = Vec::new();
    let mut buf = [0u8; STREAM_IO_BUF_SIZE];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if payload.len().saturating_add(n) > MAX_IN_MEMORY_STREAM_BYTES {
                    return Err(TTZipStatus::ErrOutOfMemory);
                }
                payload.extend_from_slice(&buf[..n]);
                bomb_guard.check(payload.len() as u64, compressed_size)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(TTZipStatus::ErrCorruptHeader),
        }
    }
    Ok(payload)
}
