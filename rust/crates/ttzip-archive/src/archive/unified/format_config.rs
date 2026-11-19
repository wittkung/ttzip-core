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
        let ext = crate::types::current_extended_create_options();
        let lvl = ext
            .raw_level
            .map(|raw| i32::from(raw).clamp(0, 9))
            .unwrap_or_else(|| match options.level {
                TTZipCompressionLevel::Store => 0,
                TTZipCompressionLevel::Fastest => 1,
                TTZipCompressionLevel::Fast => 3,
                TTZipCompressionLevel::Normal => 6,
                TTZipCompressionLevel::Maximum | TTZipCompressionLevel::Ultra => 9,
            });
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

#[derive(Clone, Copy)]
struct FilteredWriteSpec {
    tar_format: TTZipArchiveFormat,
    raw_format: TTZipArchiveFormat,
    add_filter: unsafe extern "C" fn(*mut c_void) -> libc::c_int,
    apply_level: bool,
}

const FILTERED_WRITE_SPECS: &[FilteredWriteSpec] = &[
    FilteredWriteSpec {
        tar_format: TTZipArchiveFormat::TarGz,
        raw_format: TTZipArchiveFormat::Gzip,
        add_filter: archive_write_add_filter_gzip,
        apply_level: true,
    },
    FilteredWriteSpec {
        tar_format: TTZipArchiveFormat::TarBz2,
        raw_format: TTZipArchiveFormat::Bzip2,
        add_filter: archive_write_add_filter_bzip2,
        apply_level: true,
    },
    FilteredWriteSpec {
        tar_format: TTZipArchiveFormat::TarXz,
        raw_format: TTZipArchiveFormat::Xz,
        add_filter: archive_write_add_filter_xz,
        apply_level: true,
    },
    FilteredWriteSpec {
        tar_format: TTZipArchiveFormat::TarZstd,
        raw_format: TTZipArchiveFormat::Zstd,
        add_filter: archive_write_add_filter_zstd,
        apply_level: true,
    },
    FilteredWriteSpec {
        tar_format: TTZipArchiveFormat::TarLz4,
        raw_format: TTZipArchiveFormat::Lz4,
        add_filter: archive_write_add_filter_lz4,
        apply_level: true,
    },
    FilteredWriteSpec {
        tar_format: TTZipArchiveFormat::TarLzip,
        raw_format: TTZipArchiveFormat::Lzip,
        add_filter: archive_write_add_filter_lzip,
        apply_level: false,
    },
    FilteredWriteSpec {
        tar_format: TTZipArchiveFormat::TarLrzip,
        raw_format: TTZipArchiveFormat::Lrzip,
        add_filter: archive_write_add_filter_lrzip,
        apply_level: false,
    },
];

const SIMPLE_FORMAT_SETTERS: &[(
    TTZipArchiveFormat,
    unsafe extern "C" fn(*mut c_void) -> libc::c_int,
)] = &[
    (TTZipArchiveFormat::Tar, archive_write_set_format_pax_restricted),
    (TTZipArchiveFormat::Iso, archive_write_set_format_iso9660),
    (TTZipArchiveFormat::Cpio, archive_write_set_format_cpio_newc),
    (TTZipArchiveFormat::Ar, archive_write_set_format_ar_bsd),
    (TTZipArchiveFormat::Xar, archive_write_set_format_xar),
];

fn lookup_filtered_write_spec(format: TTZipArchiveFormat) -> Option<FilteredWriteSpec> {
    FILTERED_WRITE_SPECS
        .iter()
        .find(|spec| spec.tar_format == format || spec.raw_format == format)
        .copied()
}

fn lookup_simple_format_setter(
    format: TTZipArchiveFormat,
) -> Option<unsafe extern "C" fn(*mut c_void) -> libc::c_int> {
    SIMPLE_FORMAT_SETTERS
        .iter()
        .find(|(fmt, _)| *fmt == format)
        .map(|(_, setter)| *setter)
}

unsafe fn dispatch_archive_write_format(
    a: *mut c_void,
    resolved_format: TTZipArchiveFormat,
    options: &TTZipCreateOptions,
    is_raw_stream: bool,
) -> Result<libc::c_int, TTZipStatus> {
    if matches!(resolved_format, TTZipArchiveFormat::Zip | TTZipArchiveFormat::Auto) {
        return Ok(configure_zip_write_format(a, options));
    }
    if resolved_format == TTZipArchiveFormat::SevenZip {
        if !options.password.is_null() {
            return Err(TTZipStatus::ErrCompressionFailed);
        }
        return Ok(unsafe { archive_write_set_format_7zip(a) });
    }
    if let Some(spec) = lookup_filtered_write_spec(resolved_format) {
        return Ok(unsafe {
            configure_filtered_tar_or_raw(
                a,
                resolved_format,
                spec.raw_format,
                is_raw_stream,
                options.level,
                spec.add_filter,
                spec.apply_level,
            )
        });
    }
    if let Some(setter) = lookup_simple_format_setter(resolved_format) {
        return Ok(unsafe { setter(a) });
    }
    match resolved_format {
        TTZipArchiveFormat::Wim => Err(TTZipStatus::ErrCompressionFailed),
        TTZipArchiveFormat::Rar => Err(TTZipStatus::ErrInvalidParam),
        _ => Err(TTZipStatus::ErrUnsupportedFeature),
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
        let fmt_rc = dispatch_archive_write_format(a, resolved_format, options, is_raw_stream)?;
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

fn map_standard_compression_level(level: TTZipCompressionLevel) -> i32 {
    match level {
        TTZipCompressionLevel::Store | TTZipCompressionLevel::Fastest => 1,
        TTZipCompressionLevel::Fast => 3,
        TTZipCompressionLevel::Normal => 6,
        TTZipCompressionLevel::Maximum | TTZipCompressionLevel::Ultra => 9,
    }
}

fn map_zstd_compression_level(level: TTZipCompressionLevel) -> i32 {
    match level {
        TTZipCompressionLevel::Store | TTZipCompressionLevel::Fastest => 1,
        TTZipCompressionLevel::Fast => 3,
        TTZipCompressionLevel::Normal => 6,
        TTZipCompressionLevel::Maximum => 19,
        TTZipCompressionLevel::Ultra => 22,
    }
}

fn map_lz4_compression_level(level: TTZipCompressionLevel) -> i32 {
    match level {
        TTZipCompressionLevel::Store
        | TTZipCompressionLevel::Fastest
        | TTZipCompressionLevel::Fast
        | TTZipCompressionLevel::Normal => 1,
        TTZipCompressionLevel::Maximum | TTZipCompressionLevel::Ultra => 9,
    }
}

fn resolve_filter_module_and_level(
    format: TTZipArchiveFormat,
    level: TTZipCompressionLevel,
) -> Option<(&'static str, i32)> {
    let ext = crate::types::current_extended_create_options();
    let std_level = ext
        .raw_level
        .map(|raw| i32::from(raw).clamp(1, 9))
        .unwrap_or_else(|| map_standard_compression_level(level));
    match format {
        TTZipArchiveFormat::TarGz | TTZipArchiveFormat::Gzip => Some(("gzip", std_level)),
        TTZipArchiveFormat::TarBz2 | TTZipArchiveFormat::Bzip2 => Some(("bzip2", std_level)),
        TTZipArchiveFormat::TarXz | TTZipArchiveFormat::Xz => {
            let xz_level = ext
                .raw_level
                .map(|raw| i32::from(raw).clamp(0, 9))
                .unwrap_or_else(|| {
                    if level == TTZipCompressionLevel::Store { 0 } else { std_level }
                });
            Some(("xz", xz_level))
        }
        TTZipArchiveFormat::TarZstd | TTZipArchiveFormat::Zstd => {
            let zstd_level = ext
                .raw_level
                .map(|raw| i32::from(raw).clamp(1, 22))
                .unwrap_or_else(|| map_zstd_compression_level(level));
            Some(("zstd", zstd_level))
        }
        TTZipArchiveFormat::TarLz4 | TTZipArchiveFormat::Lz4 => {
            let lz4_level = ext
                .raw_level
                .map(|raw| i32::from(raw).clamp(1, 9))
                .unwrap_or_else(|| map_lz4_compression_level(level));
            Some(("lz4", lz4_level))
        }
        _ => None,
    }
}

/// Applies compression level option to the matching libarchive filter.
pub(crate) unsafe fn apply_filter_compression_level(
    a: *mut c_void,
    format: TTZipArchiveFormat,
    level: TTZipCompressionLevel,
) {
    let Some((module_name, lvl_num)) = resolve_filter_module_and_level(format, level) else {
        return;
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

    if matches!(format, TTZipArchiveFormat::TarXz | TTZipArchiveFormat::Xz) {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        if let Ok(threads_c) = CString::new(threads.to_string()) {
            unsafe {
                archive_write_set_filter_option(
                    a,
                    c"xz".as_ptr(),
                    c"threads".as_ptr(),
                    threads_c.as_ptr(),
                );
            }
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

#[path = "format_config_gzip.rs"]
mod format_config_gzip;
#[path = "format_config_zstd.rs"]
mod format_config_zstd;

pub(crate) use format_config_gzip::{
    build_gzip_stream_config, write_gzip_target_in_txn, GzipTargetStreamWriter,
};
#[cfg(test)]
use format_config_zstd::build_zstd_stream_config;
pub(crate) use format_config_zstd::write_zstd_target_in_txn;
use format_config_zstd::zstd_window_log_from_mb;

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
    let ext = crate::types::current_extended_create_options();
    let q = ext
        .raw_level
        .map(|raw| u32::from(raw).clamp(0, 11))
        .unwrap_or_else(|| map_brotli_quality(options.level));
    let lgwin = if ext.dictionary_size_mb > 0 {
        zstd_window_log_from_mb(ext.dictionary_size_mb, false).clamp(10, 24)
    } else {
        22
    };
    let mut brotli_writer = brotli::CompressorWriter::new(target, 64 * 1024, q, lgwin);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{with_extended_create_options, TTZipExtendedCreateOptions};

    #[test]
    fn test_extended_create_options_propagate_to_zstd_and_filter_levels() {
        let opts = TTZipCreateOptions {
            level: TTZipCompressionLevel::Ultra,
            thread_budget: 2,
            ..Default::default()
        };

        let ext = TTZipExtendedCreateOptions {
            dictionary_size_mb: 32,
            enable_ldm: true,
            raw_level: Some(12),
        };
        with_extended_create_options(ext, || {
            let zstd_cfg = build_zstd_stream_config(&opts);
            assert_eq!(zstd_cfg.level, 12);
            assert!(zstd_cfg.enable_ldm);
            assert_eq!(zstd_cfg.window_log, 25);
            assert_eq!(
                resolve_filter_module_and_level(
                    TTZipArchiveFormat::TarZstd,
                    TTZipCompressionLevel::Normal
                ),
                Some(("zstd", 12))
            );
        });

        let gz_ext = TTZipExtendedCreateOptions {
            dictionary_size_mb: 16,
            enable_ldm: false,
            raw_level: Some(7),
        };
        with_extended_create_options(gz_ext, || {
            assert_eq!(
                resolve_filter_module_and_level(
                    TTZipArchiveFormat::TarGz,
                    TTZipCompressionLevel::Maximum
                ),
                Some(("gzip", 7))
            );
        });
    }

    fn run_zstd_create(
        out_path: &std::path::Path,
        src_path: &std::path::Path,
        opts: &TTZipCreateOptions,
        enable_ldm: bool,
    ) -> Vec<u8> {
        let ext = TTZipExtendedCreateOptions {
            dictionary_size_mb: 16,
            enable_ldm,
            raw_level: Some(1),
        };
        with_extended_create_options(ext, || {
            write_zstd_target_in_txn(out_path, 0, opts, Some(src_path), None, Ok)
                .expect("write zstd archive");
        });
        std::fs::read(out_path).expect("read zstd archive")
    }

    #[test]
    fn test_write_zstd_target_resets_ldm_between_sequential_archives() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let src_path = tmp.path().join("payload.bin");
        let mut payload = Vec::with_capacity(2 * 1024 * 1024 + 512 * 1024);
        let mut state = 0x1234_5678_9ABC_DEF0u64;
        let mut repeated_block = vec![0u8; 256 * 1024];
        for chunk in repeated_block.as_chunks_mut::<8>().0 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            chunk.copy_from_slice(&state.to_le_bytes());
        }
        payload.extend_from_slice(&repeated_block);
        for i in 0..8_192u32 {
            payload.extend_from_slice(&[0u8; 64]);
            payload.extend_from_slice(&i.wrapping_mul(2_654_435_761).to_le_bytes());
        }
        payload.extend_from_slice(&repeated_block);
        std::fs::write(&src_path, &payload).expect("write payload");

        let opts = TTZipCreateOptions {
            level: TTZipCompressionLevel::Fastest,
            thread_budget: 2,
            ..Default::default()
        };

        let mut baseline_opts = opts;
        baseline_opts.thread_budget = 3;
        let base_p = tmp.path().join("baseline_no_ldm.zst");
        let on_p = tmp.path().join("ldm_on.zst");
        let off_p = tmp.path().join("ldm_off.zst");
        let baseline_bytes = run_zstd_create(&base_p, &src_path, &baseline_opts, false);
        let ldm_on_bytes = run_zstd_create(&on_p, &src_path, &opts, true);
        let ldm_off_bytes = run_zstd_create(&off_p, &src_path, &opts, false);
        assert_ne!(ldm_on_bytes, baseline_bytes);
        assert_eq!(ldm_off_bytes, baseline_bytes);
    }
}
