// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive extraction lifecycle submodule for Unified Orchestrator.

#[path = "handle_pump.rs"]
mod handle_pump;

use handle_pump::{
    StreamEofVerify, extract_from_archive_handle, extract_from_archive_handle_with_stream_verify,
};
use std::ffi::{CStr, CString};
use std::fs;
use std::io::{Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::fallback_policy::{has_7z_signature, has_zip_signature, should_abort_without_fallback};
use crate::archive::split::{detect_volume_chain, VirtualMultiVolumeReader};
use crate::ffi::archive_ffi::guards::ArchiveReadGuard;
use crate::ffi::archive_ffi::sys::*;
use crate::types::{ProgressAtomicSheet, TTZipExtractOptions, TTZipStatus};
use libc::c_void;

/// Extracts an archive to destination directory with security verification and APFS preallocation.
pub fn extract_archive(
    archive_path: &Path,
    staging: &crate::fs::collision::staging::StagingRoot,
    options: &TTZipExtractOptions,
) -> Result<(), TTZipStatus> {
    extract_archive_with_sheet(archive_path, staging, options, None).map(|_| ())
}

/// Extracts an archive and returns total extracted uncompressed bytes.
pub fn extract_archive_with_metrics(
    archive_path: &Path,
    staging: &crate::fs::collision::staging::StagingRoot,
    options: &TTZipExtractOptions,
) -> Result<u64, TTZipStatus> {
    extract_archive_with_sheet(archive_path, staging, options, None)
}

/// Extracts an archive with an optional atomic progress sheet for non-blocking cancellation and progress tracking.
pub fn extract_archive_with_sheet(
    archive_path: &Path,
    staging: &crate::fs::collision::staging::StagingRoot,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    let volume_chain = detect_volume_chain(archive_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;

    if options.dry_run {
        return extract_archive_internal(archive_path, &volume_chain, staging.path(), options, sheet);
    }

    let result = extract_archive_internal(archive_path, &volume_chain, staging.path(), options, sheet);
    match result {
        Ok(bytes) => {
            if let Some(s) = sheet {
                s.mark_finished();
            }
            Ok(bytes)
        }
        Err(err) => Err(err),
    }
}

fn strip_split_suffix(name_lower: &str) -> &str {
    if let Some(dot_pos) = name_lower.rfind('.') {
        let ext = &name_lower[dot_pos + 1..];
        let is_num = ext.len() >= 3 && ext.chars().all(|c| c.is_ascii_digit());
        let is_z = ext.starts_with('z') && ext.len() >= 3 && ext[1..].chars().all(|c| c.is_ascii_digit());
        if is_num || is_z {
            return &name_lower[..dot_pos];
        }
    }
    name_lower
}

fn extract_multivolume_7z(
    virtual_reader: &mut VirtualMultiVolumeReader,
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    let pwd_str = if !options.password.is_null() {
        unsafe { CStr::from_ptr(options.password).to_str().ok() }
    } else {
        None
    };
    let info = crate::sevenz::encrypted_header::EncodedHeaderDecoder::default()
        .decode_from_reader(virtual_reader, pwd_str)
        .map_err(|e| match e {
            crate::sevenz::dag::SevenZError::BadPassword => TTZipStatus::ErrInvalidPassword,
            _ => TTZipStatus::ErrCorruptHeader,
        })?;
    crate::sevenz::decoder::archive::extract_7z_from_reader(
        virtual_reader,
        &info,
        destination_path,
        options,
        sheet,
    )
}

unsafe fn apply_archive_passphrase(a: *mut c_void, password: *const libc::c_char) {
    if !password.is_null() && unsafe { CStr::from_ptr(password).to_str().is_ok_and(|s| !s.is_empty()) } {
        unsafe { archive_read_add_passphrase(a, password) };
    }
}

fn extract_multivolume_seekable_libarchive(
    virtual_reader: VirtualMultiVolumeReader,
    archive_path: &Path,
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    let reader = crate::archive::stream_adapter::read::ArchiveStreamReader::open_seekable(virtual_reader, 65536)?;
    let raw_a = reader.as_raw_archive();
    unsafe {
        apply_archive_passphrase(raw_a, options.password);
        extract_from_archive_handle(raw_a, archive_path, destination_path, options, sheet)
    }
}

fn verify_multivolume_tar_eof_trailer(
    virtual_reader: &mut VirtualMultiVolumeReader,
) -> Result<(), TTZipStatus> {
    use std::io::Read;
    if virtual_reader.total_size() < 1024 {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let mut tail = [0u8; 512];
    if virtual_reader.seek(SeekFrom::End(-512)).is_err()
        || virtual_reader.read_exact(&mut tail).is_err()
        || tail.iter().any(|&b| b != 0)
    {
        return Err(TTZipStatus::ErrCorruptHeader);
    }
    let _ = virtual_reader.seek(SeekFrom::Start(0));
    Ok(())
}

fn extract_compressed_tar_stream<R: std::io::Read + 'static>(
    pre: R,
    archive_path: &Path,
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    let mut stream_reader =
        crate::archive::stream_adapter::read::ArchiveStreamReader::open_sequential(pre, 65536)?;
    let raw_a = stream_reader.as_raw_archive();
    let mut verify_eof = || stream_reader.verify_underlying_eof();
    unsafe {
        apply_archive_passphrase(raw_a, options.password);
        extract_from_archive_handle_with_stream_verify(
            raw_a,
            archive_path,
            destination_path,
            options,
            sheet,
            StreamEofVerify::Custom(&mut verify_eof),
        )
    }
}

fn extract_special_or_compressed_stream(
    archive_path: &Path,
    volume_chain: &[PathBuf],
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<Option<u64>, TTZipStatus> {
    if let Some(transcoded) =
        crate::archive::stream_adapter::try_transcode_special_container(archive_path, volume_chain)?
    {
        if transcoded.entry_count == 0 {
            return Ok(Some(0));
        }
        let reader = crate::archive::stream_adapter::read::ArchiveStreamReader::open_sequential(
            transcoded.cursor,
            65536,
        )?;
        let bytes = unsafe {
            extract_from_archive_handle(reader.as_raw_archive(), archive_path, destination_path, options, sheet)
        }?;
        return Ok(Some(bytes));
    }

    match crate::archive::stream_adapter::try_open_resolved_stream(archive_path, volume_chain)? {
        Some(crate::archive::stream_adapter::ResolvedStreamKind::TarContainer(pre)) => {
            extract_compressed_tar_stream(pre, archive_path, destination_path, options, sheet).map(Some)
        }
        Some(crate::archive::stream_adapter::ResolvedStreamKind::SingleFileStream {
            stream,
            entry_name,
            compressed_size,
            mtime_epoch_secs,
            ..
        }) => crate::archive::stream_adapter::extract_single_file_stream(
            stream, &entry_name, compressed_size, mtime_epoch_secs, destination_path, options, sheet,
        )
        .map(Some),
        None => Ok(None),
    }
}

fn extract_multivolume_tar_or_libarchive(
    mut virtual_reader: VirtualMultiVolumeReader,
    volume_chain: &[PathBuf],
    base_lower: &str,
    archive_path: &Path,
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    if base_lower.ends_with(".tar") {
        verify_multivolume_tar_eof_trailer(&mut virtual_reader)?;
    }
    if let Some(bytes) =
        extract_special_or_compressed_stream(archive_path, volume_chain, destination_path, options, sheet)?
    {
        return Ok(bytes);
    }
    extract_multivolume_seekable_libarchive(virtual_reader, archive_path, destination_path, options, sheet)
}

fn extract_multivolume_archive(
    volume_chain: Vec<PathBuf>,
    archive_path: &Path,
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    use std::io::Read;
    let first_name_lower = volume_chain
        .first()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    let base_lower = strip_split_suffix(&first_name_lower);

    let mut virtual_reader = VirtualMultiVolumeReader::from_volumes(volume_chain.clone())
        .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let total_len = virtual_reader.total_size();
    let mut magic = [0u8; 6];
    let has_magic = virtual_reader.read_exact(&mut magic).is_ok();
    let _ = virtual_reader.seek(SeekFrom::Start(0));

    if has_magic && &magic == b"7z\xBC\xAF\x27\x1C" {
        return extract_multivolume_7z(&mut virtual_reader, destination_path, options, sheet);
    }

    let is_zip = has_magic
        && (&magic[..4] == b"PK\x03\x04"
            || &magic[..4] == b"PK\x05\x06"
            || &magic[..4] == b"PK\x07\x08");
    if is_zip || base_lower.ends_with(".zip") {
        match crate::zip::reader_stream::extract_zip_from_reader(
            &mut virtual_reader,
            total_len,
            destination_path,
            options,
            sheet,
        ) {
            Ok(report) => return Ok(report.total_uncompressed_bytes),
            Err(err) if should_abort_without_fallback(err) => return Err(err),
            Err(_) => {
                let _ = virtual_reader.seek(SeekFrom::Start(0));
            }
        }
    }

    extract_multivolume_tar_or_libarchive(
        virtual_reader,
        &volume_chain,
        base_lower,
        archive_path,
        destination_path,
        options,
        sheet,
    )
}

fn try_fast_extract_tar_slice(
    mapped: &[u8],
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<Option<u64>, TTZipStatus> {
    if mapped.len() < 1024
        || !crate::archive::stream_adapter::is_valid_tar_header_block(&mapped[..512])
        || mapped[mapped.len() - 512..].iter().any(|&b| b != 0)
    {
        return Ok(None);
    }
    let Ok(tar_archive) = crate::archive::tar::TarArchive::open_slice(mapped) else {
        return Ok(None);
    };
    let can_fast_extract = !tar_archive.is_empty()
        && tar_archive.entries().iter().all(|e| {
            matches!(
                e.typeflag,
                crate::archive::tar::header::TYPE_REGULAR
                    | crate::archive::tar::header::TYPE_REGULAR_ALT
                    | crate::archive::tar::header::TYPE_DIRECTORY
                    | crate::archive::tar::header::TYPE_SYMLINK
                    | crate::archive::tar::header::TYPE_HARDLINK
            ) && e
                .pax_attributes
                .as_ref()
                .is_none_or(|p| p.raw_map.is_empty())
        });
    if !can_fast_extract {
        return Ok(None);
    }
    if sheet.is_some_and(|s| s.is_cancelled()) {
        return Err(TTZipStatus::Cancelled);
    }
    match tar_archive.extract_all_with_sheet(destination_path, options, sheet) {
        Ok(report) => {
            if let Some(s) = sheet {
                s.set_total(report.total_uncompressed_bytes);
                let done = s
                    .processed_bytes
                    .load(std::sync::atomic::Ordering::Relaxed);
                if done < report.total_uncompressed_bytes {
                    s.add_processed(report.total_uncompressed_bytes - done);
                }
            }
            Ok(Some(report.total_uncompressed_bytes))
        }
        Err(err) if should_abort_without_fallback(err) => Err(err),
        Err(_) => Ok(None),
    }
}

fn is_sequential_compressed_stream_path(archive_path: &Path) -> bool {
    let raw_name = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let base = strip_split_suffix(&raw_name);
    const SEQ_EXTS: &[&str] = &[
        ".tar.gz", ".tgz", ".tar.zst", ".tzst", ".tar.xz", ".txz", ".tar.bz2", ".tbz2", ".tbz",
        ".tar.lz4", ".tlz4", ".tar.br", ".tbr", ".tar.lz", ".tlz", ".tar.lrz", ".tlrz", ".tar.sz",
        ".gz", ".gzip", ".zst", ".zstd", ".xz", ".bz2", ".bzip2", ".lz4", ".br", ".brotli", ".lz",
        ".lzip", ".lrz", ".lrzip", ".sz", ".snappy", ".lzfse",
    ];
    SEQ_EXTS.iter().any(|ext| base.ends_with(ext))
}

fn try_extract_mapped_7z(
    mapped: &[u8],
    pwd_str: Option<&str>,
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<Option<u64>, TTZipStatus> {
    if !has_7z_signature(mapped) {
        return Ok(None);
    }
    match crate::sevenz::decoder::archive::SevenZArchive::open_slice_with_password(mapped, pwd_str)
    {
        Ok(sevenz) => match sevenz.extract_all_with_sheet(destination_path, options, sheet) {
            Ok(report) => Ok(Some(report.total_uncompressed_bytes)),
            Err(err) if should_abort_without_fallback(err) => Err(err),
            Err(_) => Ok(None),
        },
        Err(err) if should_abort_without_fallback(err) => Err(err),
        Err(_) => Ok(None),
    }
}

fn try_extract_mapped_zip(
    mapped: &[u8],
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<Option<u64>, TTZipStatus> {
    match crate::zip::reader::ZipArchive::open_slice(mapped) {
        Ok(zip_archive) => match zip_archive.extract_all_with_sheet(destination_path, options, sheet) {
            Ok(report) => Ok(Some(report.total_uncompressed_bytes)),
            Err(err) if should_abort_without_fallback(err) => Err(err),
            Err(_) => Ok(None),
        },
        Err(err @ (TTZipStatus::ErrSecurityViolation | TTZipStatus::ErrOutOfMemory))
            if has_zip_signature(mapped) =>
        {
            Err(err)
        }
        Err(_) => Ok(None),
    }
}

fn try_extract_mapped_archive(
    archive_path: &Path,
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<Option<u64>, TTZipStatus> {
    let Ok(source) = crate::archive::source::open_archive_source(archive_path) else {
        return Ok(None);
    };
    let Some(mapped) = source.as_slice() else {
        return Ok(None);
    };
    let pwd_str = if !options.password.is_null() {
        unsafe { CStr::from_ptr(options.password).to_str().ok() }
    } else {
        None
    };
    if let Some(bytes) = try_extract_mapped_7z(mapped, pwd_str, destination_path, options, sheet)? {
        return Ok(Some(bytes));
    }
    if let Some(bytes) = try_extract_mapped_zip(mapped, destination_path, options, sheet)? {
        return Ok(Some(bytes));
    }
    if options.password.is_null() {
        if let Some(bytes) = try_fast_extract_tar_slice(mapped, destination_path, options, sheet)? {
            return Ok(Some(bytes));
        }
    }
    Ok(None)
}

fn extract_archive_internal(
    archive_path: &Path,
    volume_chain: &[PathBuf],
    destination_path: &Path,
    options: &TTZipExtractOptions,
    sheet: Option<&ProgressAtomicSheet>,
) -> Result<u64, TTZipStatus> {
    fs::create_dir_all(destination_path).map_err(|_| TTZipStatus::ErrExtractionFailed)?;

    if volume_chain.len() > 1 {
        return extract_multivolume_archive(
            volume_chain.to_vec(),
            archive_path,
            destination_path,
            options,
            sheet,
        );
    }

    if let Some(bytes) =
        try_extract_mapped_archive(archive_path, destination_path, options, sheet)?
    {
        return Ok(bytes);
    }

    if let Some(bytes) = extract_special_or_compressed_stream(
        archive_path,
        volume_chain,
        destination_path,
        options,
        sheet,
    )? {
        return Ok(bytes);
    }

    let arch_c = CString::new(archive_path.to_str().ok_or(TTZipStatus::ErrInvalidParam)?)
        .map_err(|_| TTZipStatus::ErrInvalidParam)?;

    unsafe {
        let a = archive_read_new();
        if a.is_null() {
            return Err(TTZipStatus::ErrOutOfMemory);
        }
        let _guard = ArchiveReadGuard(a);

        archive_read_support_format_all(a);
        archive_read_support_filter_all(a);
        apply_archive_passphrase(a, options.password);

        let open_rc = archive_read_open_filename(a, arch_c.as_ptr(), 65536);
        if open_rc != 0 {
            return Err(TTZipStatus::ErrOpenFailed);
        }

        extract_from_archive_handle(a, archive_path, destination_path, options, sheet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::tar::writer::TarWriter;

    #[test]
    fn test_extract_archive_with_sheet_tar_honors_cancellation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut tar_bytes = Vec::new();
        {
            let mut writer = TarWriter::new(&mut tar_bytes);
            writer
                .append_file("item1.bin", &vec![0x33u8; 512 * 1024], 0o644, 0)
                .expect("append item1");
            writer
                .append_file("item2.bin", &vec![0x44u8; 512 * 1024], 0o644, 0)
                .expect("append item2");
        }
        tar_bytes.extend_from_slice(&[0u8; 1024]);

        let sheet = ProgressAtomicSheet::new();
        let dest = dir.path().join("out");
        let opts = TTZipExtractOptions::default();

        // Also verify TarArchive::extract_all_with_sheet cancels mid-extraction when sheet is cancelled
        // by a background watcher thread as soon as item1.bin is created or progress advances.
        let tar_archive = crate::archive::tar::reader::TarArchive::open_slice(&tar_bytes).unwrap();
        sheet.cancel();
        let direct_res = tar_archive.extract_all_with_sheet(&dest, &opts, Some(&sheet));
        assert_eq!(
            direct_res.err(),
            Some(TTZipStatus::Cancelled),
            "TarArchive::extract_all_with_sheet must honor cancelled ProgressAtomicSheet"
        );
        assert!(!dest.join("item1.bin").exists());
    }

    fn build_tar_bytes(entry_name: &str, payload: &[u8], extra_trailing_zeros: usize) -> Vec<u8> {
        let mut tar_bytes = Vec::new();
        {
            let mut writer = TarWriter::new(&mut tar_bytes);
            writer
                .append_file(entry_name, payload, 0o644, 0)
                .expect("append entry");
        }
        tar_bytes.extend_from_slice(&[0u8; 1024]);
        if extra_trailing_zeros > 0 {
            tar_bytes.resize(tar_bytes.len() + extra_trailing_zeros, 0);
        }
        tar_bytes
    }

    #[test]
    fn test_extract_compressed_tar_gz_rejects_corrupt_trailer_before_mark_finished() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tar_bytes = build_tar_bytes("hello.txt", b"hello trailer check", 0);
        let mut gz = crate::codecs::deflate::libdeflate_gzip_compress(&tar_bytes, 6)
            .expect("gzip compress");
        let last = gz.len() - 1;
        gz[last - 4] ^= 0xFF;
        let archive_path = dir.path().join("bad_trailer.tar.gz");
        fs::write(&archive_path, &gz).expect("write bad_trailer.tar.gz");

        let staging = crate::fs::collision::staging::StagingRoot(dir.path().join("out"));
        let sheet = ProgressAtomicSheet::new();
        let res = extract_archive_with_sheet(
            &archive_path,
            &staging,
            &TTZipExtractOptions::default(),
            Some(&sheet),
        );
        assert_eq!(res.err(), Some(TTZipStatus::ErrCorruptHeader));
        assert!(
            !sheet.is_finished(),
            "sheet must not mark_finished when underlying gzip EOF/CRC verification fails"
        );
    }

    #[test]
    fn test_extract_compressed_tar_gz_rejects_uncompressed_trailer_past_drain_limit() {
        use std::io::Read;

        let dir = tempfile::tempdir().expect("tempdir");
        let tar_bytes = build_tar_bytes("hello.txt", b"hello over-limit", 0);
        let mut gz = crate::codecs::deflate::libdeflate_gzip_compress(&tar_bytes, 6)
            .expect("gzip compress tar member");
        let trailer_len = 2 * 1024 * 1024 + 128 * 1024;
        let trailing_member =
            crate::codecs::deflate::libdeflate_gzip_compress(&vec![0xABu8; trailer_len], 1)
                .expect("gzip compress trailing member");
        gz.extend_from_slice(&trailing_member);

        let mut verify_decoder = flate2::read::MultiGzDecoder::new(gz.as_slice());
        let mut decoded = Vec::new();
        verify_decoder
            .read_to_end(&mut decoded)
            .expect("concatenated multi-member gzip fixture must be valid and uncorrupted");
        assert_eq!(decoded.len(), tar_bytes.len() + trailer_len);

        let archive_path = dir.path().join("over_limit.tar.gz");
        fs::write(&archive_path, &gz).expect("write over_limit.tar.gz");

        let staging = crate::fs::collision::staging::StagingRoot(dir.path().join("out"));
        let sheet = ProgressAtomicSheet::new();
        let res = extract_archive_with_sheet(
            &archive_path,
            &staging,
            &TTZipExtractOptions::default(),
            Some(&sheet),
        );
        assert_eq!(res.err(), Some(TTZipStatus::ErrCorruptHeader));
        assert!(
            !sheet.is_finished(),
            "sheet must not mark_finished when valid trailing stream exceeds MAX_TRAILER_DRAIN without EOF"
        );
    }
}

