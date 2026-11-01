// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive write format configuration and compression filter parameters.

use std::ffi::CString;
use libc::c_void;

pub(crate) use super::container_writers::{
    write_apple_archive_entries_in_txn, write_xar_entries_in_txn,
};
use super::create_transaction::CreateTransactionContext;
use crate::ffi::archive_ffi::sys::*;
use crate::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
    TTZipStatus,
};

fn configure_zip_write_format(a: *mut c_void, options: &TTZipCreateOptions) -> libc::c_int {
    let rc = unsafe { archive_write_set_format_zip(a) };
    if rc == 0 {
        let lvl = match options.level {
            TTZipCompressionLevel::Store => 0,
            TTZipCompressionLevel::Fastest => 1,
            TTZipCompressionLevel::Fast => 3,
            TTZipCompressionLevel::Normal => 6,
            TTZipCompressionLevel::Maximum | TTZipCompressionLevel::Ultra => 9,
        };
        if let Ok(lvl_opt) = CString::new(format!("zip:compression-level={lvl}")) {
            unsafe {
                archive_write_set_options(a, lvl_opt.as_ptr());
            }
        }
    }
    rc
}

unsafe fn configure_filtered_tar_or_raw(
    a: *mut c_void,
    resolved_format: TTZipArchiveFormat,
    raw_match: TTZipArchiveFormat,
    is_raw_stream: bool,
    level: TTZipCompressionLevel,
    add_filter: unsafe extern "C" fn(*mut c_void) -> libc::c_int,
    apply_level: bool,
) -> libc::c_int {
    unsafe {
        let r1 = if is_raw_stream && resolved_format == raw_match {
            archive_write_set_format_raw(a)
        } else {
            archive_write_set_format_pax_restricted(a)
        };
        let r2 = add_filter(a);
        if apply_level {
            apply_filter_compression_level(a, resolved_format, level);
        }
        if r1 != 0 { r1 } else { r2 }
    }
}

/// Configures format and filter options on the given libarchive write handle.
pub(crate) unsafe fn configure_archive_write_format(
    a: *mut c_void,
    resolved_format: TTZipArchiveFormat,
    options: &TTZipCreateOptions,
    is_raw_stream: bool,
) -> Result<(), TTZipStatus> {
    unsafe {
        let fmt_rc = match resolved_format {
            TTZipArchiveFormat::Zip | TTZipArchiveFormat::Auto => {
                configure_zip_write_format(a, options)
            }
            TTZipArchiveFormat::SevenZip => {
                if !options.password.is_null() {
                    return Err(TTZipStatus::ErrCompressionFailed);
                }
                archive_write_set_format_7zip(a)
            }
            TTZipArchiveFormat::Tar => archive_write_set_format_pax_restricted(a),
            TTZipArchiveFormat::TarGz | TTZipArchiveFormat::Gzip => configure_filtered_tar_or_raw(
                a,
                resolved_format,
                TTZipArchiveFormat::Gzip,
                is_raw_stream,
                options.level,
                archive_write_add_filter_gzip,
                true,
            ),
            TTZipArchiveFormat::TarBz2 | TTZipArchiveFormat::Bzip2 => configure_filtered_tar_or_raw(
                a,
                resolved_format,
                TTZipArchiveFormat::Bzip2,
                is_raw_stream,
                options.level,
                archive_write_add_filter_bzip2,
                true,
            ),
            TTZipArchiveFormat::TarXz | TTZipArchiveFormat::Xz => configure_filtered_tar_or_raw(
                a,
                resolved_format,
                TTZipArchiveFormat::Xz,
                is_raw_stream,
                options.level,
                archive_write_add_filter_xz,
                true,
            ),
            TTZipArchiveFormat::TarZstd | TTZipArchiveFormat::Zstd => configure_filtered_tar_or_raw(
                a,
                resolved_format,
                TTZipArchiveFormat::Zstd,
                is_raw_stream,
                options.level,
                archive_write_add_filter_zstd,
                true,
            ),
            TTZipArchiveFormat::TarLz4 | TTZipArchiveFormat::Lz4 => configure_filtered_tar_or_raw(
                a,
                resolved_format,
                TTZipArchiveFormat::Lz4,
                is_raw_stream,
                options.level,
                archive_write_add_filter_lz4,
                true,
            ),
            TTZipArchiveFormat::TarBrotli | TTZipArchiveFormat::Brotli => {
                return Err(TTZipStatus::ErrUnsupportedFeature);
            }
            TTZipArchiveFormat::TarLzip | TTZipArchiveFormat::Lzip => configure_filtered_tar_or_raw(
                a,
                resolved_format,
                TTZipArchiveFormat::Lzip,
                is_raw_stream,
                options.level,
                archive_write_add_filter_lzip,
                false,
            ),
            TTZipArchiveFormat::TarLrzip | TTZipArchiveFormat::Lrzip => {
                configure_filtered_tar_or_raw(
                    a,
                    resolved_format,
                    TTZipArchiveFormat::Lrzip,
                    is_raw_stream,
                    options.level,
                    archive_write_add_filter_lrzip,
                    false,
                )
            }
            TTZipArchiveFormat::Iso => archive_write_set_format_iso9660(a),
            TTZipArchiveFormat::Cpio => archive_write_set_format_cpio_newc(a),
            TTZipArchiveFormat::Ar => archive_write_set_format_ar_bsd(a),
            TTZipArchiveFormat::Xar => archive_write_set_format_xar(a),
            TTZipArchiveFormat::Wim => return Err(TTZipStatus::ErrCompressionFailed),
            TTZipArchiveFormat::Rar => return Err(TTZipStatus::ErrInvalidParam),
            _ => return Err(TTZipStatus::ErrUnsupportedFeature),
        };

        if fmt_rc != 0 && fmt_rc != -20 {
            return Err(TTZipStatus::ErrCompressionFailed);
        }

        if !options.password.is_null() {
            archive_write_set_passphrase(a, options.password);
            if resolved_format == TTZipArchiveFormat::Zip
                && options.encryption == TTZipEncryptionMethod::Aes256
            {
                if let Ok(enc_opt) = CString::new("zip:encryption=aes256") {
                    archive_write_set_options(a, enc_opt.as_ptr());
                }
            }
        }

        // Match `archive_write_open_filename` / `archive_write_open_fd` regular-file behavior
        // so `archive_write_open2` callers do not zero-pad the final 10,240-byte block after
        // compressed stream or container trailers.
        if archive_write_set_bytes_in_last_block(a, 1) != 0 {
            return Err(TTZipStatus::ErrCompressionFailed);
        }

        Ok(())
    }
}

/// Applies compression level option to the matching libarchive filter.
pub(crate) unsafe fn apply_filter_compression_level(
    a: *mut c_void,
    format: TTZipArchiveFormat,
    level: TTZipCompressionLevel,
) {
    let std_level = match level {
        TTZipCompressionLevel::Store | TTZipCompressionLevel::Fastest => 1,
        TTZipCompressionLevel::Fast => 3,
        TTZipCompressionLevel::Normal => 6,
        TTZipCompressionLevel::Maximum | TTZipCompressionLevel::Ultra => 9,
    };
    let (module_name, lvl_num) = match format {
        TTZipArchiveFormat::TarGz | TTZipArchiveFormat::Gzip => ("gzip", std_level),
        TTZipArchiveFormat::TarBz2 | TTZipArchiveFormat::Bzip2 => ("bzip2", std_level),
        TTZipArchiveFormat::TarXz | TTZipArchiveFormat::Xz => {
            let n = if level == TTZipCompressionLevel::Store {
                0
            } else {
                std_level
            };
            ("xz", n)
        }
        TTZipArchiveFormat::TarZstd | TTZipArchiveFormat::Zstd => {
            let n = match level {
                TTZipCompressionLevel::Store | TTZipCompressionLevel::Fastest => 1,
                TTZipCompressionLevel::Fast => 3,
                TTZipCompressionLevel::Normal => 6,
                TTZipCompressionLevel::Maximum => 19,
                TTZipCompressionLevel::Ultra => 22,
            };
            ("zstd", n)
        }
        TTZipArchiveFormat::TarLz4 | TTZipArchiveFormat::Lz4 => ("lz4", std_level),
        _ => return,
    };

    if let (Ok(mod_c), Ok(opt_c), Ok(val_c)) = (
        CString::new(module_name),
        CString::new("compression-level"),
        CString::new(lvl_num.to_string()),
    ) {
        unsafe {
            archive_write_set_filter_option(a, mod_c.as_ptr(), opt_c.as_ptr(), val_c.as_ptr());
        }
    }
}

/// Resolves the effective creation format, upgrading single-stream enums to compound `Tar*`
/// only when the destination filename explicitly ends with a `.tar.*` / `.t*` suffix.
pub(crate) fn effective_create_format(
    format: TTZipArchiveFormat,
    dest_path: &std::path::Path,
) -> TTZipArchiveFormat {
    const TAR_UPGRADES: &[(TTZipArchiveFormat, &[&str], TTZipArchiveFormat)] = &[
        (TTZipArchiveFormat::Gzip, &[".tar.gz", ".tgz"], TTZipArchiveFormat::TarGz),
        (TTZipArchiveFormat::Bzip2, &[".tar.bz2", ".tbz2", ".tbz"], TTZipArchiveFormat::TarBz2),
        (TTZipArchiveFormat::Xz, &[".tar.xz", ".txz"], TTZipArchiveFormat::TarXz),
        (TTZipArchiveFormat::Zstd, &[".tar.zst", ".tzst"], TTZipArchiveFormat::TarZstd),
        (TTZipArchiveFormat::Lz4, &[".tar.lz4", ".tlz4"], TTZipArchiveFormat::TarLz4),
        (TTZipArchiveFormat::Brotli, &[".tar.br", ".tbr"], TTZipArchiveFormat::TarBrotli),
        (TTZipArchiveFormat::Lzip, &[".tar.lz", ".tlz"], TTZipArchiveFormat::TarLzip),
        (TTZipArchiveFormat::Lrzip, &[".tar.lrz", ".tlrz"], TTZipArchiveFormat::TarLrzip),
    ];
    let base_fmt = crate::archive::unified::detect::resolve_create_format(format, dest_path);
    let raw_name = dest_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    let stem_name = raw_name
        .rsplit_once('.')
        .filter(|(_, ext)| ext.len() >= 3 && ext.bytes().all(|b| b.is_ascii_digit()))
        .map_or(raw_name.as_str(), |(s, _)| s);
    for (single_fmt, suffixes, tar_fmt) in TAR_UPGRADES {
        if *single_fmt == base_fmt && suffixes.iter().any(|suf| stem_name.ends_with(suf)) {
            return *tar_fmt;
        }
    }
    base_fmt
}

/// Validates whether `resolved_format` is supported for archive creation before touching the destination path.
pub(crate) fn validate_creatable_format(
    resolved_format: TTZipArchiveFormat,
    is_explicit_entrypoint: bool,
) -> Result<(), TTZipStatus> {
    match resolved_format {
        TTZipArchiveFormat::Rar => Err(TTZipStatus::ErrInvalidParam),
        TTZipArchiveFormat::Wim => {
            if is_explicit_entrypoint {
                Err(TTZipStatus::ErrUnsupportedFeature)
            } else {
                Err(TTZipStatus::ErrCompressionFailed)
            }
        }
        TTZipArchiveFormat::Dmg
        | TTZipArchiveFormat::Cab
        | TTZipArchiveFormat::Rpm
        | TTZipArchiveFormat::Deb
        | TTZipArchiveFormat::Lzh
        | TTZipArchiveFormat::Squashfs
        | TTZipArchiveFormat::Lzfse => Err(TTZipStatus::ErrUnsupportedFeature),
        _ => Ok(()),
    }
}

fn map_brotli_quality(level: TTZipCompressionLevel) -> u32 {
    match level {
        TTZipCompressionLevel::Store => 0,
        TTZipCompressionLevel::Fastest => 1,
        TTZipCompressionLevel::Fast => 3,
        TTZipCompressionLevel::Normal => 6,
        TTZipCompressionLevel::Maximum => 9,
        TTZipCompressionLevel::Ultra => 11,
    }
}

fn build_zstd_stream_config(options: &TTZipCreateOptions) -> crate::codecs::zstd::types::ZstdConfig {
    let z_lvl = match options.level {
        TTZipCompressionLevel::Store | TTZipCompressionLevel::Fastest => 1,
        TTZipCompressionLevel::Fast => 3,
        TTZipCompressionLevel::Normal => 6,
        TTZipCompressionLevel::Maximum => 19,
        TTZipCompressionLevel::Ultra => 22,
    };
    let nb_workers = if options.thread_budget > 0 {
        options.thread_budget
    } else {
        std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(4)
    };
    crate::codecs::zstd::types::ZstdConfig {
        level: z_lvl,
        nb_workers,
        enable_checksum: true,
        ..Default::default()
    }
}

pub(crate) fn write_brotli_target_in_txn<F>(
    destination_path: &std::path::Path,
    split_volume_size_bytes: u64,
    options: &TTZipCreateOptions,
    raw_source: Option<&std::path::Path>,
    txn_ctx: Option<&CreateTransactionContext>,
    write_tar: F,
) -> Result<(), TTZipStatus>
where
    F: FnOnce(
        brotli::CompressorWriter<crate::archive::split::StreamOutputTarget>,
    ) -> Result<
        brotli::CompressorWriter<crate::archive::split::StreamOutputTarget>,
        TTZipStatus,
    >,
{
    use std::io::Write;
    let mut raw_in = match raw_source {
        Some(src_file) => {
            Some(std::fs::File::open(src_file).map_err(|_| TTZipStatus::ErrFileNotFound)?)
        }
        None => None,
    };
    let target = crate::archive::split::StreamOutputTarget::open_in_txn(
        destination_path,
        split_volume_size_bytes,
        txn_ctx,
    )
    .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let q = map_brotli_quality(options.level);
    let mut brotli_writer = brotli::CompressorWriter::new(target, 64 * 1024, q, 22);
    let mut prior_res: Result<(), (TTZipStatus, String)> = Ok(());
    if let Some(ref mut in_file) = raw_in {
        if let Err(e) = std::io::copy(in_file, &mut brotli_writer) {
            prior_res = Err((
                TTZipStatus::ErrCompressionFailed,
                format!("brotli copy failed: {e}"),
            ));
        }
    } else {
        brotli_writer = write_tar(brotli_writer)?;
    }
    if prior_res.is_ok() {
        if let Err(e) = brotli_writer.flush() {
            prior_res = Err((
                TTZipStatus::ErrCompressionFailed,
                format!("brotli flush failed: {e}"),
            ));
        }
    }
    brotli_writer
        .into_inner()
        .finish_and_settle_in_txn(prior_res)
}

pub(crate) fn write_snappy_target_in_txn<F>(
    destination_path: &std::path::Path,
    split_volume_size_bytes: u64,
    raw_source: Option<&std::path::Path>,
    txn_ctx: Option<&CreateTransactionContext>,
    write_tar: F,
) -> Result<(), TTZipStatus>
where
    F: FnOnce(
        snap::write::FrameEncoder<crate::archive::split::StreamOutputTarget>,
    ) -> Result<
        snap::write::FrameEncoder<crate::archive::split::StreamOutputTarget>,
        TTZipStatus,
    >,
{
    use std::io::Write;
    let mut raw_in = match raw_source {
        Some(src_file) => {
            Some(std::fs::File::open(src_file).map_err(|_| TTZipStatus::ErrFileNotFound)?)
        }
        None => None,
    };
    let target = crate::archive::split::StreamOutputTarget::open_in_txn(
        destination_path,
        split_volume_size_bytes,
        txn_ctx,
    )
    .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let mut snappy_encoder = snap::write::FrameEncoder::new(target);
    if let Some(ref mut in_file) = raw_in {
        std::io::copy(in_file, &mut snappy_encoder)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    } else {
        snappy_encoder = write_tar(snappy_encoder)?;
    }
    snappy_encoder
        .flush()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    snappy_encoder
        .into_inner()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?
        .finish_and_settle_in_txn(Ok(()))
}

pub(crate) fn write_zstd_target_in_txn<F>(
    destination_path: &std::path::Path,
    split_volume_size_bytes: u64,
    options: &TTZipCreateOptions,
    raw_source: Option<&std::path::Path>,
    txn_ctx: Option<&CreateTransactionContext>,
    write_tar: F,
) -> Result<(), TTZipStatus>
where
    F: FnOnce(
        crate::codecs::zstd::ZstdStreamWriter<crate::archive::split::StreamOutputTarget>,
    ) -> Result<
        crate::codecs::zstd::ZstdStreamWriter<crate::archive::split::StreamOutputTarget>,
        TTZipStatus,
    >,
{
    let mut raw_in = match raw_source {
        Some(src_file) => {
            Some(std::fs::File::open(src_file).map_err(|_| TTZipStatus::ErrFileNotFound)?)
        }
        None => None,
    };
    let target = crate::archive::split::StreamOutputTarget::open_in_txn(
        destination_path,
        split_volume_size_bytes,
        txn_ctx,
    )
    .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let config = build_zstd_stream_config(options);
    let mut zstd_writer = crate::codecs::zstd::ZstdStreamWriter::new(target, &config)?;
    if let Some(ref mut in_file) = raw_in {
        std::io::copy(in_file, &mut zstd_writer)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    } else {
        zstd_writer = write_tar(zstd_writer)?;
    }
    zstd_writer
        .finish()?
        .finish_and_settle_in_txn(Ok(()))
}
