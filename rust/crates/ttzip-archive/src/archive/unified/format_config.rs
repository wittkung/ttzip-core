// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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
        TTZipArchiveFormat::Ar => {
            archive_write_set_format_ar_bsd(a)
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
            return Err(TTZipStatus::ErrUnsupportedFeature);
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

/// Writes a genuine macOS Apple Archive (`.aar`, `pbze` LZFSE stream) using in-process `/usr/lib/libAppleArchive.dylib`.
#[cfg(target_os = "macos")]
pub(crate) fn write_apple_archive_entries(
    entries: &[(std::path::PathBuf, String)],
    destination_path: &std::path::Path,
    split_volume_size_bytes: u64,
) -> Result<(), TTZipStatus> {
    use std::io::Write;
    if split_volume_size_bytes == 0 {
        return write_apple_archive_single_file(entries, destination_path);
    }
    let tmp_aar = destination_path.with_extension("aar.tmp_stage");
    if let Err(e) = write_apple_archive_single_file(entries, &tmp_aar) {
        let _ = std::fs::remove_file(&tmp_aar);
        return Err(e);
    }
    let copy_res = (|| -> Result<(), TTZipStatus> {
        let mut in_f = std::fs::File::open(&tmp_aar).map_err(|_| TTZipStatus::ErrOpenFailed)?;
        let mut target =
            crate::archive::split::StreamOutputTarget::open(destination_path, split_volume_size_bytes)
                .map_err(|_| TTZipStatus::ErrOpenFailed)?;
        std::io::copy(&mut in_f, &mut target).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        target.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        target.commit().map_err(|_| TTZipStatus::ErrCompressionFailed)
    })();
    let _ = std::fs::remove_file(&tmp_aar);
    copy_res
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Copy, Clone)]
struct AaFieldKey {
    ikey: [u8; 4],
}

#[cfg(target_os = "macos")]
type FnAAFileStreamOpenWithPath =
    unsafe extern "C" fn(*const libc::c_char, libc::c_int, libc::mode_t) -> *mut c_void;
#[cfg(target_os = "macos")]
type FnAACompressionOutputStreamOpen =
    unsafe extern "C" fn(*mut c_void, u32, usize, u64, libc::c_int) -> *mut c_void;
#[cfg(target_os = "macos")]
type FnAAByteStreamClose = unsafe extern "C" fn(*mut c_void) -> libc::c_int;
#[cfg(target_os = "macos")]
type FnAAEncodeArchiveOutputStreamOpen =
    unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, u64, libc::c_int) -> *mut c_void;
#[cfg(target_os = "macos")]
type FnAAArchiveStreamClose = unsafe extern "C" fn(*mut c_void) -> libc::c_int;
#[cfg(target_os = "macos")]
type FnAAHeaderCreate = unsafe extern "C" fn() -> *mut c_void;
#[cfg(target_os = "macos")]
type FnAAHeaderDestroy = unsafe extern "C" fn(*mut c_void);
#[cfg(target_os = "macos")]
type FnAAHeaderSetFieldBlob =
    unsafe extern "C" fn(*mut c_void, u32, AaFieldKey, u64) -> libc::c_int;
#[cfg(target_os = "macos")]
type FnAAHeaderSetFieldString =
    unsafe extern "C" fn(*mut c_void, u32, AaFieldKey, *const libc::c_char, usize) -> libc::c_int;
#[cfg(target_os = "macos")]
type FnAAHeaderSetFieldUInt =
    unsafe extern "C" fn(*mut c_void, u32, AaFieldKey, u64) -> libc::c_int;
#[cfg(target_os = "macos")]
type FnAAArchiveStreamWriteHeader =
    unsafe extern "C" fn(*mut c_void, *mut c_void) -> libc::c_int;
#[cfg(target_os = "macos")]
type FnAAArchiveStreamWriteBlob =
    unsafe extern "C" fn(*mut c_void, AaFieldKey, *const c_void, usize) -> libc::c_int;

#[cfg(target_os = "macos")]
struct AppleArchiveApi {
    open_file: FnAAFileStreamOpenWithPath,
    open_comp: FnAACompressionOutputStreamOpen,
    close_byte: FnAAByteStreamClose,
    open_enc: FnAAEncodeArchiveOutputStreamOpen,
    close_arch: FnAAArchiveStreamClose,
    hdr_create: FnAAHeaderCreate,
    hdr_destroy: FnAAHeaderDestroy,
    set_blob: FnAAHeaderSetFieldBlob,
    set_str: FnAAHeaderSetFieldString,
    set_uint: FnAAHeaderSetFieldUInt,
    write_hdr: FnAAArchiveStreamWriteHeader,
    write_blob: FnAAArchiveStreamWriteBlob,
}

#[cfg(target_os = "macos")]
unsafe fn load_apple_archive_api() -> Result<AppleArchiveApi, TTZipStatus> {
    let lib = libc::dlopen(c"/usr/lib/libAppleArchive.dylib".as_ptr(), libc::RTLD_LAZY);
    if lib.is_null() {
        return Err(TTZipStatus::ErrUnsupportedFeature);
    }
    macro_rules! sym {
        ($name:literal, $ty:ty) => {{
            let p = libc::dlsym(lib, concat!($name, "\0").as_ptr() as *const libc::c_char);
            if p.is_null() {
                return Err(TTZipStatus::ErrUnsupportedFeature);
            }
            std::mem::transmute::<*mut libc::c_void, $ty>(p)
        }};
    }
    Ok(AppleArchiveApi {
        open_file: sym!("AAFileStreamOpenWithPath", FnAAFileStreamOpenWithPath),
        open_comp: sym!("AACompressionOutputStreamOpen", FnAACompressionOutputStreamOpen),
        close_byte: sym!("AAByteStreamClose", FnAAByteStreamClose),
        open_enc: sym!("AAEncodeArchiveOutputStreamOpen", FnAAEncodeArchiveOutputStreamOpen),
        close_arch: sym!("AAArchiveStreamClose", FnAAArchiveStreamClose),
        hdr_create: sym!("AAHeaderCreate", FnAAHeaderCreate),
        hdr_destroy: sym!("AAHeaderDestroy", FnAAHeaderDestroy),
        set_blob: sym!("AAHeaderSetFieldBlob", FnAAHeaderSetFieldBlob),
        set_str: sym!("AAHeaderSetFieldString", FnAAHeaderSetFieldString),
        set_uint: sym!("AAHeaderSetFieldUInt", FnAAHeaderSetFieldUInt),
        write_hdr: sym!("AAArchiveStreamWriteHeader", FnAAArchiveStreamWriteHeader),
        write_blob: sym!("AAArchiveStreamWriteBlob", FnAAArchiveStreamWriteBlob),
    })
}

#[cfg(target_os = "macos")]
unsafe fn write_single_aa_entry(
    api: &AppleArchiveApi,
    e_stream: *mut c_void,
    abs_path: &std::path::Path,
    clean_rel: &str,
) -> Result<(), TTZipStatus> {
    use std::io::Read;
    use std::os::unix::fs::PermissionsExt;

    let meta = std::fs::symlink_metadata(abs_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let Ok(rel_c) = CString::new(clean_rel) else { return Ok(()); };
    let hdr = (api.hdr_create)();
    if hdr.is_null() {
        return Err(TTZipStatus::ErrOutOfMemory);
    }
    let k_typ = AaFieldKey { ikey: *b"TYP\0" };
    let k_pat = AaFieldKey { ikey: *b"PAT\0" };
    let k_lnk = AaFieldKey { ikey: *b"LNK\0" };
    let k_mod = AaFieldKey { ikey: *b"MOD\0" };
    let k_dat = AaFieldKey { ikey: *b"DAT\0" };
    let ft = meta.file_type();
    let perm = (meta.permissions().mode() & 0o7777) as u64;
    let res = if ft.is_symlink() {
        (api.set_uint)(hdr, u32::MAX, k_typ, b'L' as u64);
        (api.set_str)(hdr, u32::MAX, k_pat, rel_c.as_ptr(), clean_rel.len());
        (api.set_uint)(hdr, u32::MAX, k_mod, 0o777);
        if let Ok(target) = std::fs::read_link(abs_path) {
            let t_str = target.to_string_lossy();
            if let Ok(t_c) = CString::new(t_str.as_bytes()) {
                (api.set_str)(hdr, u32::MAX, k_lnk, t_c.as_ptr(), t_str.len());
            }
        }
        ((api.write_hdr)(e_stream, hdr) >= 0).then_some(()).ok_or(TTZipStatus::ErrCompressionFailed)
    } else if ft.is_dir() {
        (api.set_uint)(hdr, u32::MAX, k_typ, b'D' as u64);
        (api.set_str)(hdr, u32::MAX, k_pat, rel_c.as_ptr(), clean_rel.len());
        (api.set_uint)(hdr, u32::MAX, k_mod, perm);
        ((api.write_hdr)(e_stream, hdr) >= 0).then_some(()).ok_or(TTZipStatus::ErrCompressionFailed)
    } else {
        let mut f = std::fs::File::open(abs_path).map_err(|_| {
            (api.hdr_destroy)(hdr);
            TTZipStatus::ErrFileNotFound
        })?;
        let mut data = Vec::new();
        f.read_to_end(&mut data).map_err(|_| {
            (api.hdr_destroy)(hdr);
            TTZipStatus::ErrCompressionFailed
        })?;
        (api.set_uint)(hdr, u32::MAX, k_typ, b'F' as u64);
        (api.set_str)(hdr, u32::MAX, k_pat, rel_c.as_ptr(), clean_rel.len());
        (api.set_uint)(hdr, u32::MAX, k_mod, perm);
        (api.set_blob)(hdr, u32::MAX, k_dat, data.len() as u64);
        let ok = (api.write_hdr)(e_stream, hdr) >= 0
            && (data.is_empty() || (api.write_blob)(e_stream, k_dat, data.as_ptr() as *const c_void, data.len()) >= 0);
        ok.then_some(()).ok_or(TTZipStatus::ErrCompressionFailed)
    };
    (api.hdr_destroy)(hdr);
    res
}

#[cfg(target_os = "macos")]
fn write_apple_archive_single_file(
    entries: &[(std::path::PathBuf, String)],
    destination_path: &std::path::Path,
) -> Result<(), TTZipStatus> {
    unsafe {
        let api = load_apple_archive_api()?;
        let dest_c = CString::new(destination_path.to_str().ok_or(TTZipStatus::ErrInvalidParam)?)
            .map_err(|_| TTZipStatus::ErrInvalidParam)?;
        let f_stream = (api.open_file)(dest_c.as_ptr(), libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);
        if f_stream.is_null() {
            return Err(TTZipStatus::ErrOpenFailed);
        }
        let c_stream = (api.open_comp)(f_stream, 0x801, 1 << 20, 0, 0);
        if c_stream.is_null() {
            (api.close_byte)(f_stream);
            return Err(TTZipStatus::ErrCompressionFailed);
        }
        let e_stream = (api.open_enc)(c_stream, std::ptr::null_mut(), std::ptr::null_mut(), 0, 0);
        if e_stream.is_null() {
            (api.close_byte)(c_stream);
            (api.close_byte)(f_stream);
            return Err(TTZipStatus::ErrCompressionFailed);
        }
        let mut status = Ok(());
        for (abs_path, rel_name) in entries {
            let clean_rel = rel_name.trim_matches('/');
            if !clean_rel.is_empty() {
                if let Err(e) = write_single_aa_entry(&api, e_stream, abs_path, clean_rel) {
                    status = Err(e);
                    break;
                }
            }
        }
        let r1 = (api.close_arch)(e_stream);
        let r2 = (api.close_byte)(c_stream);
        let r3 = (api.close_byte)(f_stream);
        status?;
        if r1 < 0 || r2 < 0 || r3 < 0 {
            return Err(TTZipStatus::ErrCompressionFailed);
        }
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn write_apple_archive_entries(
    _entries: &[(std::path::PathBuf, String)],
    _destination_path: &std::path::Path,
    _split_volume_size_bytes: u64,
) -> Result<(), TTZipStatus> {
    Err(TTZipStatus::ErrUnsupportedFeature)
}

fn build_xar_heap_and_tree(
    entries: &[(std::path::PathBuf, String)],
    heap_path: &std::path::Path,
    comp_level: flate2::Compression,
    options: &TTZipCreateOptions,
    total_bytes: u64,
) -> Result<(std::collections::BTreeMap<String, super::create_staging::XarTreeNode>, u64), TTZipStatus> {
    use super::create_staging::{
        check_xar_progress, insert_xar_tree_node, stream_compress_xar_file, XarNodeKind,
        XarStreamState, XarTreeNode,
    };
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let mut heap_file = std::fs::File::create(heap_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let mut state = XarStreamState {
        heap_file: &mut heap_file,
        heap_pos: 0,
        comp_level,
        options,
        processed_bytes: 0,
        total_bytes,
    };
    let mut tree_root = std::collections::BTreeMap::new();
    for (abs_path, rel_name) in entries {
        let meta = std::fs::symlink_metadata(abs_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
        let parts: Vec<&str> = rel_name.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            continue;
        }
        let clean_rel = rel_name.trim_matches('/');
        let perm = meta.permissions().mode() & 0o7777;
        let ft = meta.file_type();
        let node = if ft.is_symlink() {
            check_xar_progress(options, state.processed_bytes, total_bytes, clean_rel)?;
            let target = std::fs::read_link(abs_path).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            XarTreeNode { mode: 0o755, kind: XarNodeKind::Symlink { target: target.to_string_lossy().into_owned() } }
        } else if ft.is_dir() {
            check_xar_progress(options, state.processed_bytes, total_bytes, clean_rel)?;
            XarTreeNode { mode: perm, kind: XarNodeKind::Directory { children: std::collections::BTreeMap::new() } }
        } else {
            let kind = stream_compress_xar_file(abs_path, clean_rel, &mut state)?;
            XarTreeNode { mode: perm, kind }
        };
        insert_xar_tree_node(&mut tree_root, &parts, node);
    }
    state.heap_file.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    Ok((tree_root, state.processed_bytes))
}

fn map_zlib_level(level: TTZipCompressionLevel) -> flate2::Compression {
    match level {
        TTZipCompressionLevel::Store => flate2::Compression::none(),
        TTZipCompressionLevel::Fastest => flate2::Compression::fast(),
        TTZipCompressionLevel::Fast => flate2::Compression::new(3),
        TTZipCompressionLevel::Normal => flate2::Compression::default(),
        TTZipCompressionLevel::Maximum | TTZipCompressionLevel::Ultra => flate2::Compression::best(),
    }
}

pub(crate) fn write_xar_entries(
    entries: &[(std::path::PathBuf, String)],
    destination_path: &std::path::Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
) -> Result<(), TTZipStatus> {
    use super::create_staging::{check_xar_progress, render_xar_tree_xml, ScopedTempFile};
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};

    let total_bytes: u64 = entries
        .iter()
        .filter_map(|(p, _)| std::fs::symlink_metadata(p).ok())
        .filter(|m| m.is_file() && !m.file_type().is_symlink())
        .map(|m| m.len())
        .sum();
    let comp_level = map_zlib_level(options.level);
    let heap_guard = ScopedTempFile::in_parent_of(destination_path, "xar_heap");
    let (tree_root, processed_bytes) =
        build_xar_heap_and_tree(entries, &heap_guard.0, comp_level, options, total_bytes)?;

    let mut toc_files = String::new();
    let mut next_id = 1u64;
    render_xar_tree_xml(&tree_root, &mut next_id, &mut toc_files);
    let toc_xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<xar>\n <toc>\n  <checksum style=\"sha256\"><offset>0</offset><size>32</size></checksum>\n{toc_files} </toc>\n</xar>"
    );
    let mut z_toc = flate2::write::ZlibEncoder::new(Vec::new(), comp_level);
    z_toc.write_all(toc_xml.as_bytes()).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    let toc_comp = z_toc.finish().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    let toc_hash = Sha256::digest(&toc_comp);

    let mut hdr = [0u8; 28];
    hdr[0..4].copy_from_slice(b"xar!");
    hdr[4..6].copy_from_slice(&28u16.to_be_bytes());
    hdr[6..8].copy_from_slice(&1u16.to_be_bytes());
    hdr[8..16].copy_from_slice(&(toc_comp.len() as u64).to_be_bytes());
    hdr[16..24].copy_from_slice(&(toc_xml.len() as u64).to_be_bytes());
    hdr[24..28].copy_from_slice(&3u32.to_be_bytes());

    let mut target =
        crate::archive::split::StreamOutputTarget::open(destination_path, split_volume_size_bytes)
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    target.write_all(&hdr).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    target.write_all(&toc_comp).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    target.write_all(&toc_hash).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    let mut heap_reader = std::fs::File::open(&heap_guard.0).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    let mut copy_buf = [0u8; 64 * 1024];
    loop {
        let n = heap_reader.read(&mut copy_buf).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        if n == 0 {
            break;
        }
        check_xar_progress(options, processed_bytes, total_bytes, "")?;
        target.write_all(&copy_buf[..n]).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    }
    drop(heap_reader);
    drop(heap_guard);
    target.commit().map_err(|_| TTZipStatus::ErrCompressionFailed)
}

pub(crate) fn write_brotli_target<F>(
    destination_path: &std::path::Path,
    split_volume_size_bytes: u64,
    options: &TTZipCreateOptions,
    raw_source: Option<&std::path::Path>,
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
    let target =
        crate::archive::split::StreamOutputTarget::open(destination_path, split_volume_size_bytes)
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let q = match options.level {
        TTZipCompressionLevel::Store => 0,
        TTZipCompressionLevel::Fastest => 1,
        TTZipCompressionLevel::Fast => 3,
        TTZipCompressionLevel::Normal => 6,
        TTZipCompressionLevel::Maximum => 9,
        TTZipCompressionLevel::Ultra => 11,
    };
    let mut brotli_writer = brotli::CompressorWriter::new(target, 64 * 1024, q, 22);
    if let Some(src_file) = raw_source {
        let mut in_file = std::fs::File::open(src_file).map_err(|_| TTZipStatus::ErrFileNotFound)?;
        std::io::copy(&mut in_file, &mut brotli_writer)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    } else {
        brotli_writer = write_tar(brotli_writer)?;
    }
    brotli_writer
        .flush()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    brotli_writer
        .into_inner()
        .commit()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)
}

pub(crate) fn write_snappy_target<F>(
    destination_path: &std::path::Path,
    split_volume_size_bytes: u64,
    raw_source: Option<&std::path::Path>,
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
    let target =
        crate::archive::split::StreamOutputTarget::open(destination_path, split_volume_size_bytes)
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let mut snappy_encoder = snap::write::FrameEncoder::new(target);
    if let Some(src_file) = raw_source {
        let mut in_file = std::fs::File::open(src_file).map_err(|_| TTZipStatus::ErrFileNotFound)?;
        std::io::copy(&mut in_file, &mut snappy_encoder)
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
        .commit()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)
}

pub(crate) fn write_zstd_target<F>(
    destination_path: &std::path::Path,
    split_volume_size_bytes: u64,
    options: &TTZipCreateOptions,
    raw_source: Option<&std::path::Path>,
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
    let target =
        crate::archive::split::StreamOutputTarget::open(destination_path, split_volume_size_bytes)
            .map_err(|_| TTZipStatus::ErrOpenFailed)?;
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
    let config = crate::codecs::zstd::types::ZstdConfig {
        level: z_lvl,
        nb_workers,
        enable_checksum: true,
        ..Default::default()
    };
    let mut zstd_writer = crate::codecs::zstd::ZstdStreamWriter::new(target, &config)?;
    if let Some(src_file) = raw_source {
        let mut in_file = std::fs::File::open(src_file).map_err(|_| TTZipStatus::ErrFileNotFound)?;
        std::io::copy(&mut in_file, &mut zstd_writer)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    } else {
        zstd_writer = write_tar(zstd_writer)?;
    }
    zstd_writer
        .finish()?
        .commit()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)
}
