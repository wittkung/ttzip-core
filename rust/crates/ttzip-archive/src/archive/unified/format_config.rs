// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Archive write format configuration and compression filter parameters.

use std::ffi::CString;
use libc::c_void;

use crate::ffi::archive_ffi::sys::*;
use crate::types::{
    TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions, TTZipEncryptionMethod,
    TTZipStatus,
};

/// Configures format and filter options on the given libarchive write handle.
pub(crate) unsafe fn configure_archive_write_format(
    a: *mut c_void,
    resolved_format: TTZipArchiveFormat,
    options: &TTZipCreateOptions,
    is_raw_stream: bool,
) -> Result<(), TTZipStatus> {
    let fmt_rc = match resolved_format {
        TTZipArchiveFormat::Zip | TTZipArchiveFormat::Auto => {
            let rc = archive_write_set_format_zip(a);
            if rc == 0 {
                let lvl = match options.level {
                    TTZipCompressionLevel::Store => 0,
                    TTZipCompressionLevel::Fastest => 1,
                    TTZipCompressionLevel::Fast => 3,
                    TTZipCompressionLevel::Normal => 6,
                    TTZipCompressionLevel::Maximum => 9,
                    TTZipCompressionLevel::Ultra => 9,
                };
                let lvl_opt = CString::new(format!("zip:compression-level={}", lvl)).unwrap();
                archive_write_set_options(a, lvl_opt.as_ptr());
            }
            rc
        }
        TTZipArchiveFormat::SevenZip => {
            if !options.password.is_null() {
                return Err(TTZipStatus::ErrCompressionFailed);
            }
            archive_write_set_format_7zip(a)
        }
        TTZipArchiveFormat::Tar => {
            archive_write_set_format_pax_restricted(a)
        }
        TTZipArchiveFormat::TarGz | TTZipArchiveFormat::Gzip => {
            let r1 = if is_raw_stream && resolved_format == TTZipArchiveFormat::Gzip {
                archive_write_set_format_raw(a)
            } else {
                archive_write_set_format_pax_restricted(a)
            };
            let r2 = archive_write_add_filter_gzip(a);
            apply_filter_compression_level(a, resolved_format, options.level);
            if r1 != 0 { r1 } else { r2 }
        }
        TTZipArchiveFormat::TarBz2 | TTZipArchiveFormat::Bzip2 => {
            let r1 = if is_raw_stream && resolved_format == TTZipArchiveFormat::Bzip2 {
                archive_write_set_format_raw(a)
            } else {
                archive_write_set_format_pax_restricted(a)
            };
            let r2 = archive_write_add_filter_bzip2(a);
            apply_filter_compression_level(a, resolved_format, options.level);
            if r1 != 0 { r1 } else { r2 }
        }
        TTZipArchiveFormat::TarXz | TTZipArchiveFormat::Xz => {
            let r1 = if is_raw_stream && resolved_format == TTZipArchiveFormat::Xz {
                archive_write_set_format_raw(a)
            } else {
                archive_write_set_format_pax_restricted(a)
            };
            let r2 = archive_write_add_filter_xz(a);
            apply_filter_compression_level(a, resolved_format, options.level);
            if r1 != 0 { r1 } else { r2 }
        }
        TTZipArchiveFormat::TarZstd | TTZipArchiveFormat::Zstd => {
            let r1 = if is_raw_stream && resolved_format == TTZipArchiveFormat::Zstd {
                archive_write_set_format_raw(a)
            } else {
                archive_write_set_format_pax_restricted(a)
            };
            let r2 = archive_write_add_filter_zstd(a);
            apply_filter_compression_level(a, resolved_format, options.level);
            if r1 != 0 { r1 } else { r2 }
        }
        TTZipArchiveFormat::TarLz4 | TTZipArchiveFormat::Lz4 => {
            let r1 = if is_raw_stream && resolved_format == TTZipArchiveFormat::Lz4 {
                archive_write_set_format_raw(a)
            } else {
                archive_write_set_format_pax_restricted(a)
            };
            let r2 = archive_write_add_filter_lz4(a);
            apply_filter_compression_level(a, resolved_format, options.level);
            if r1 != 0 { r1 } else { r2 }
        }
        TTZipArchiveFormat::TarBrotli | TTZipArchiveFormat::Brotli => {
            return Err(TTZipStatus::ErrUnsupportedFeature);
        }
        TTZipArchiveFormat::TarLzip | TTZipArchiveFormat::Lzip => {
            let r1 = if is_raw_stream && resolved_format == TTZipArchiveFormat::Lzip {
                archive_write_set_format_raw(a)
            } else {
                archive_write_set_format_pax_restricted(a)
            };
            let r2 = archive_write_add_filter_lzip(a);
            if r1 != 0 { r1 } else { r2 }
        }
        TTZipArchiveFormat::TarLrzip | TTZipArchiveFormat::Lrzip => {
            let r1 = if is_raw_stream && resolved_format == TTZipArchiveFormat::Lrzip {
                archive_write_set_format_raw(a)
            } else {
                archive_write_set_format_pax_restricted(a)
            };
            let r2 = archive_write_add_filter_lrzip(a);
            if r1 != 0 { r1 } else { r2 }
        }
        TTZipArchiveFormat::Iso => {
            archive_write_set_format_iso9660(a)
        }
        TTZipArchiveFormat::Cpio => {
            archive_write_set_format_cpio_newc(a)
        }
        TTZipArchiveFormat::Ar | TTZipArchiveFormat::Deb => {
            archive_write_set_format_ar_svr4(a)
        }
        TTZipArchiveFormat::Xar => {
            archive_write_set_format_xar(a)
        }
        TTZipArchiveFormat::Wim => {
            return Err(TTZipStatus::ErrCompressionFailed);
        }
        TTZipArchiveFormat::Rar => {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        _ => {
            archive_write_set_format_zip(a)
        }
    };

    if fmt_rc != 0 && fmt_rc != -20 {
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    if !options.password.is_null() {
        archive_write_set_passphrase(a, options.password);
        if resolved_format == TTZipArchiveFormat::Zip && options.encryption == TTZipEncryptionMethod::Aes256 {
            let enc_opt = CString::new("zip:encryption=aes256").unwrap();
            archive_write_set_options(a, enc_opt.as_ptr());
        }
    }

    Ok(())
}

/// Applies compression level option to the matching libarchive filter.
pub(crate) unsafe fn apply_filter_compression_level(
    a: *mut c_void,
    format: TTZipArchiveFormat,
    level: TTZipCompressionLevel,
) {
    let (module_name, lvl_num) = match format {
        TTZipArchiveFormat::TarGz | TTZipArchiveFormat::Gzip => {
            let n = match level {
                TTZipCompressionLevel::Store => 1,
                TTZipCompressionLevel::Fastest => 1,
                TTZipCompressionLevel::Fast => 3,
                TTZipCompressionLevel::Normal => 6,
                TTZipCompressionLevel::Maximum | TTZipCompressionLevel::Ultra => 9,
            };
            ("gzip", n)
        }
        TTZipArchiveFormat::TarBz2 | TTZipArchiveFormat::Bzip2 => {
            let n = match level {
                TTZipCompressionLevel::Store => 1,
                TTZipCompressionLevel::Fastest => 1,
                TTZipCompressionLevel::Fast => 3,
                TTZipCompressionLevel::Normal => 6,
                TTZipCompressionLevel::Maximum | TTZipCompressionLevel::Ultra => 9,
            };
            ("bzip2", n)
        }
        TTZipArchiveFormat::TarXz | TTZipArchiveFormat::Xz => {
            let n = match level {
                TTZipCompressionLevel::Store => 0,
                TTZipCompressionLevel::Fastest => 1,
                TTZipCompressionLevel::Fast => 3,
                TTZipCompressionLevel::Normal => 6,
                TTZipCompressionLevel::Maximum | TTZipCompressionLevel::Ultra => 9,
            };
            ("xz", n)
        }
        TTZipArchiveFormat::TarZstd | TTZipArchiveFormat::Zstd => {
            let n = match level {
                TTZipCompressionLevel::Store => 1,
                TTZipCompressionLevel::Fastest => 1,
                TTZipCompressionLevel::Fast => 3,
                TTZipCompressionLevel::Normal => 6,
                TTZipCompressionLevel::Maximum => 19,
                TTZipCompressionLevel::Ultra => 22,
            };
            ("zstd", n)
        }
        TTZipArchiveFormat::TarLz4 | TTZipArchiveFormat::Lz4 => {
            let n = match level {
                TTZipCompressionLevel::Store => 1,
                TTZipCompressionLevel::Fastest => 1,
                TTZipCompressionLevel::Fast => 3,
                TTZipCompressionLevel::Normal => 6,
                TTZipCompressionLevel::Maximum | TTZipCompressionLevel::Ultra => 9,
            };
            ("lz4", n)
        }
        _ => return,
    };

    if let (Ok(mod_c), Ok(opt_c), Ok(val_c)) = (
        CString::new(module_name),
        CString::new("compression-level"),
        CString::new(lvl_num.to_string()),
    ) {
        archive_write_set_filter_option(a, mod_c.as_ptr(), opt_c.as_ptr(), val_c.as_ptr());
    }
}
