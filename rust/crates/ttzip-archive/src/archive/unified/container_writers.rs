// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Transaction-bound native Apple Archive (`.aar`) and XAR 1.0 container writers
//! using borrowed-FD streaming (`automatic_close = 0`) and same-handle `O_RDWR` readback.

use std::ffi::CString;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

use libc::c_void;

use super::create_staging::{
    check_xar_progress, insert_xar_tree_node, render_xar_tree_xml, stream_compress_xar_file,
    ScopedTempFile, XarNodeKind, XarStreamState, XarTreeNode,
};
use super::create_transaction::{
    CreateFileAccessMode, CreateTransactionContext, FileFinishSyncPolicy, ResidualPathRole,
};
use crate::types::{TTZipCompressionLevel, TTZipCreateOptions, TTZipStatus};

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Copy, Clone)]
struct AaFieldKey {
    ikey: [u8; 4],
}

#[cfg(target_os = "macos")]
type FnAAFileStreamOpenWithFD = unsafe extern "C" fn(libc::c_int, libc::c_int) -> *mut c_void;
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
    open_file_fd: FnAAFileStreamOpenWithFD,
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
    unsafe {
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
            open_file_fd: sym!("AAFileStreamOpenWithFD", FnAAFileStreamOpenWithFD),
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
}

#[cfg(target_os = "macos")]
unsafe fn write_single_aa_entry(
    api: &AppleArchiveApi,
    e_stream: *mut c_void,
    abs_path: &Path,
    clean_rel: &str,
) -> Result<(), TTZipStatus> {
    use std::os::unix::fs::PermissionsExt;

    let meta = std::fs::symlink_metadata(abs_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let Ok(rel_c) = CString::new(clean_rel) else {
        return Ok(());
    };
    unsafe {
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
            ((api.write_hdr)(e_stream, hdr) >= 0)
                .then_some(())
                .ok_or(TTZipStatus::ErrCompressionFailed)
        } else if ft.is_dir() {
            (api.set_uint)(hdr, u32::MAX, k_typ, b'D' as u64);
            (api.set_str)(hdr, u32::MAX, k_pat, rel_c.as_ptr(), clean_rel.len());
            (api.set_uint)(hdr, u32::MAX, k_mod, perm);
            ((api.write_hdr)(e_stream, hdr) >= 0)
                .then_some(())
                .ok_or(TTZipStatus::ErrCompressionFailed)
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
                && (data.is_empty()
                    || (api.write_blob)(e_stream, k_dat, data.as_ptr() as *const c_void, data.len())
                        >= 0);
            ok.then_some(()).ok_or(TTZipStatus::ErrCompressionFailed)
        };
        (api.hdr_destroy)(hdr);
        res
    }
}

#[cfg(target_os = "macos")]
unsafe fn write_apple_archive_to_borrowed_fd(
    api: &AppleArchiveApi,
    raw_fd: libc::c_int,
    entries: &[(PathBuf, String)],
) -> Result<(), (TTZipStatus, String)> {
    unsafe {
        // Pass automatic_close = 0 so AppleArchive only borrows `raw_fd` and never closes it.
        let f_stream = (api.open_file_fd)(raw_fd, 0);
        if f_stream.is_null() {
            return Err((
                TTZipStatus::ErrOpenFailed,
                "AAFileStreamOpenWithFD returned null".to_string(),
            ));
        }
        let c_stream = (api.open_comp)(f_stream, 0x801, 1 << 20, 0, 0);
        if c_stream.is_null() {
            (api.close_byte)(f_stream);
            return Err((
                TTZipStatus::ErrCompressionFailed,
                "AACompressionOutputStreamOpen returned null".to_string(),
            ));
        }
        let e_stream = (api.open_enc)(c_stream, std::ptr::null_mut(), std::ptr::null_mut(), 0, 0);
        if e_stream.is_null() {
            (api.close_byte)(c_stream);
            (api.close_byte)(f_stream);
            return Err((
                TTZipStatus::ErrCompressionFailed,
                "AAEncodeArchiveOutputStreamOpen returned null".to_string(),
            ));
        }
        let mut entry_status: Result<(), TTZipStatus> = Ok(());
        for (abs_path, rel_name) in entries {
            let clean_rel = rel_name.trim_matches('/');
            if !clean_rel.is_empty() {
                if let Err(e) = write_single_aa_entry(api, e_stream, abs_path, clean_rel) {
                    entry_status = Err(e);
                    break;
                }
            }
        }
        let r1 = (api.close_arch)(e_stream);
        let r2 = (api.close_byte)(c_stream);
        let r3 = (api.close_byte)(f_stream);
        if let Err(st) = entry_status {
            return Err((st, format!("AppleArchive entry write failed: {st:?}")));
        }
        if r1 < 0 || r2 < 0 || r3 < 0 {
            return Err((
                TTZipStatus::ErrCompressionFailed,
                format!("AppleArchive stream close failed: r1={r1}, r2={r2}, r3={r3}"),
            ));
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn write_apple_archive_single_file_in_txn(
    api: &AppleArchiveApi,
    entries: &[(PathBuf, String)],
    destination_path: &Path,
    txn_ctx: Option<&CreateTransactionContext>,
) -> Result<(), TTZipStatus> {
    let (file, guard) = crate::fs::FileAbortGuard::create_staged_file_in_txn(
        destination_path,
        CreateFileAccessMode::WriteOnly,
        txn_ctx,
        ResidualPathRole::StagedArtifact,
    )
    .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let encode_res = unsafe { write_apple_archive_to_borrowed_fd(api, file.as_raw_fd(), entries) };
    guard
        .finish_file_and_settle_in_txn(file, encode_res, None, FileFinishSyncPolicy::None)
        .map(|_| ())
        .map_err(|e| e.primary_status)
}

#[cfg(target_os = "macos")]
pub(crate) fn write_apple_archive_entries_in_txn(
    entries: &[(PathBuf, String)],
    destination_path: &Path,
    split_volume_size_bytes: u64,
    txn_ctx: Option<&CreateTransactionContext>,
) -> Result<(), TTZipStatus> {
    let api = unsafe { load_apple_archive_api()? };
    if split_volume_size_bytes == 0 {
        return write_apple_archive_single_file_in_txn(&api, entries, destination_path, txn_ctx);
    }

    let (mut tmp_guard, mut tmp_file) = ScopedTempFile::create_read_write_in_txn(
        destination_path,
        "aar_tmp_stage",
        txn_ctx,
        ResidualPathRole::StagedTempFile,
    )?;
    let encode_res =
        unsafe { write_apple_archive_to_borrowed_fd(&api, tmp_file.as_raw_fd(), entries) };
    if let Err((status, msg)) = encode_res {
        let _ = tmp_guard.finish_readback_and_cleanup_in_txn(tmp_file, Err((status, msg)));
        return Err(status);
    }
    if crate::fs::abort_guard::fstat_dev_ino(tmp_file.as_raw_fd()).ok()
        != tmp_guard.expected_file_id()
        || tmp_file.seek(SeekFrom::Start(0)).is_err()
    {
        let _ = tmp_guard.finish_readback_and_cleanup_in_txn(
            tmp_file,
            Err((
                TTZipStatus::ErrCompressionFailed,
                "aar temp helper identity or rewind failed".to_string(),
            )),
        );
        return Err(TTZipStatus::ErrCompressionFailed);
    }

    let mut target = match crate::archive::split::StreamOutputTarget::open_in_txn(
        destination_path,
        split_volume_size_bytes,
        txn_ctx,
    ) {
        Ok(t) => t,
        Err(e) => {
            let _ = tmp_guard.finish_readback_and_cleanup_in_txn(
                tmp_file,
                Err((
                    TTZipStatus::ErrOpenFailed,
                    format!("aar split target open failed: {e}"),
                )),
            );
            return Err(TTZipStatus::ErrOpenFailed);
        }
    };
    let copy_res = std::io::copy(&mut tmp_file, &mut target)
        .map(|_| ())
        .map_err(|e| {
            (
                TTZipStatus::ErrCompressionFailed,
                format!("aar split copy failed: {e}"),
            )
        });
    let tmp_finish_res = tmp_guard.finish_readback_and_cleanup_in_txn(tmp_file, copy_res.clone());
    if copy_res.is_err() || tmp_finish_res.is_err() {
        target.abort_in_txn(TTZipStatus::ErrCompressionFailed);
        return Err(TTZipStatus::ErrCompressionFailed);
    }
    target.finish_and_settle_in_txn(Ok(()))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn write_apple_archive_entries_in_txn(
    _entries: &[(PathBuf, String)],
    _destination_path: &Path,
    _split_volume_size_bytes: u64,
    _txn_ctx: Option<&CreateTransactionContext>,
) -> Result<(), TTZipStatus> {
    Err(TTZipStatus::ErrUnsupportedFeature)
}

fn build_xar_heap_and_tree(
    entries: &[(PathBuf, String)],
    heap_file: &mut std::fs::File,
    comp_level: flate2::Compression,
    options: &TTZipCreateOptions,
    total_bytes: u64,
) -> Result<(std::collections::BTreeMap<String, XarTreeNode>, u64), TTZipStatus> {
    use std::os::unix::fs::PermissionsExt;

    let mut state = XarStreamState {
        heap_file,
        heap_pos: 0,
        comp_level,
        options,
        processed_bytes: 0,
        total_bytes,
    };
    let mut tree_root = std::collections::BTreeMap::new();
    for (abs_path, rel_name) in entries {
        let meta = std::fs::symlink_metadata(abs_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
        let parts: Vec<&str> = rel_name
            .trim_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();
        if parts.is_empty() {
            continue;
        }
        let clean_rel = rel_name.trim_matches('/');
        let perm = meta.permissions().mode() & 0o7777;
        let ft = meta.file_type();
        let node = if ft.is_symlink() {
            check_xar_progress(options, state.processed_bytes, total_bytes, clean_rel)?;
            let target =
                std::fs::read_link(abs_path).map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            XarTreeNode {
                mode: 0o755,
                kind: XarNodeKind::Symlink {
                    target: target.to_string_lossy().into_owned(),
                },
            }
        } else if ft.is_dir() {
            check_xar_progress(options, state.processed_bytes, total_bytes, clean_rel)?;
            XarTreeNode {
                mode: perm,
                kind: XarNodeKind::Directory {
                    children: std::collections::BTreeMap::new(),
                },
            }
        } else {
            let kind = stream_compress_xar_file(abs_path, clean_rel, &mut state)?;
            XarTreeNode { mode: perm, kind }
        };
        insert_xar_tree_node(&mut tree_root, &parts, node);
    }
    state
        .heap_file
        .flush()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    Ok((tree_root, state.processed_bytes))
}

fn map_zlib_level(level: TTZipCompressionLevel) -> flate2::Compression {
    match level {
        TTZipCompressionLevel::Store => flate2::Compression::none(),
        TTZipCompressionLevel::Fastest => flate2::Compression::fast(),
        TTZipCompressionLevel::Fast => flate2::Compression::new(3),
        TTZipCompressionLevel::Normal => flate2::Compression::default(),
        TTZipCompressionLevel::Maximum | TTZipCompressionLevel::Ultra => {
            flate2::Compression::best()
        }
    }
}

type EncodedXarToc = ([u8; 28], Vec<u8>, [u8; 32]);

fn encode_xar_toc_and_header(
    tree_root: &std::collections::BTreeMap<String, XarTreeNode>,
    comp_level: flate2::Compression,
) -> Result<EncodedXarToc, TTZipStatus> {
    use sha2::{Digest, Sha256};

    let mut toc_files = String::new();
    let mut next_id = 1u64;
    render_xar_tree_xml(tree_root, &mut next_id, &mut toc_files);
    let toc_xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<xar>\n <toc>\n  <checksum style=\"sha256\"><offset>0</offset><size>32</size></checksum>\n{toc_files} </toc>\n</xar>"
    );
    let mut z_toc = flate2::write::ZlibEncoder::new(Vec::new(), comp_level);
    z_toc
        .write_all(toc_xml.as_bytes())
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    let toc_comp = z_toc
        .finish()
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    let toc_hash: [u8; 32] = Sha256::digest(&toc_comp).into();

    let mut hdr = [0u8; 28];
    hdr[0..4].copy_from_slice(b"xar!");
    hdr[4..6].copy_from_slice(&28u16.to_be_bytes());
    hdr[6..8].copy_from_slice(&1u16.to_be_bytes());
    hdr[8..16].copy_from_slice(&(toc_comp.len() as u64).to_be_bytes());
    hdr[16..24].copy_from_slice(&(toc_xml.len() as u64).to_be_bytes());
    hdr[24..28].copy_from_slice(&3u32.to_be_bytes());
    Ok((hdr, toc_comp, toc_hash))
}

fn stream_xar_sections_to_target(
    target: &mut crate::archive::split::StreamOutputTarget,
    hdr: &[u8; 28],
    toc_comp: &[u8],
    toc_hash: &[u8; 32],
    heap_file: &mut std::fs::File,
    options: &TTZipCreateOptions,
    processed_bytes: u64,
    total_bytes: u64,
) -> Result<(), (TTZipStatus, String)> {
    target.write_all(hdr).map_err(|e| {
        (
            TTZipStatus::ErrCompressionFailed,
            format!("xar header write failed: {e}"),
        )
    })?;
    target.write_all(toc_comp).map_err(|e| {
        (
            TTZipStatus::ErrCompressionFailed,
            format!("xar toc write failed: {e}"),
        )
    })?;
    target.write_all(toc_hash).map_err(|e| {
        (
            TTZipStatus::ErrCompressionFailed,
            format!("xar toc hash write failed: {e}"),
        )
    })?;
    let mut copy_buf = [0u8; 64 * 1024];
    loop {
        let n = heap_file.read(&mut copy_buf).map_err(|e| {
            (
                TTZipStatus::ErrCompressionFailed,
                format!("xar heap readback failed: {e}"),
            )
        })?;
        if n == 0 {
            break;
        }
        check_xar_progress(options, processed_bytes, total_bytes, "")
            .map_err(|st| (st, format!("xar progress cancelled: {st:?}")))?;
        target.write_all(&copy_buf[..n]).map_err(|e| {
            (
                TTZipStatus::ErrCompressionFailed,
                format!("xar heap target write failed: {e}"),
            )
        })?;
    }
    Ok(())
}

fn abort_xar_temp_heap(
    heap_guard: &mut ScopedTempFile,
    heap_file: std::fs::File,
    status: TTZipStatus,
    reason: String,
) -> TTZipStatus {
    let _ = heap_guard.finish_readback_and_cleanup_in_txn(heap_file, Err((status, reason)));
    status
}

struct PreparedXarHeap {
    heap_guard: ScopedTempFile,
    heap_file: std::fs::File,
    hdr: [u8; 28],
    toc_comp: Vec<u8>,
    toc_hash: [u8; 32],
    processed_bytes: u64,
    total_bytes: u64,
}

fn prepare_xar_temp_heap_and_toc(
    entries: &[(PathBuf, String)],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    txn_ctx: Option<&CreateTransactionContext>,
) -> Result<PreparedXarHeap, TTZipStatus> {
    let total_bytes: u64 = entries
        .iter()
        .filter_map(|(p, _)| std::fs::symlink_metadata(p).ok())
        .filter(|m| m.is_file() && !m.file_type().is_symlink())
        .map(|m| m.len())
        .sum();
    let comp_level = map_zlib_level(options.level);
    let (mut heap_guard, mut heap_file) = ScopedTempFile::create_read_write_in_txn(
        destination_path,
        "xar_heap",
        txn_ctx,
        ResidualPathRole::StagedTempFile,
    )?;
    let (tree_root, processed_bytes) =
        match build_xar_heap_and_tree(entries, &mut heap_file, comp_level, options, total_bytes) {
            Ok(v) => v,
            Err(status) => {
                return Err(abort_xar_temp_heap(
                    &mut heap_guard,
                    heap_file,
                    status,
                    format!("xar heap build failed: {status:?}"),
                ));
            }
        };
    if crate::fs::abort_guard::fstat_dev_ino(heap_file.as_raw_fd()).ok()
        != heap_guard.expected_file_id()
        || heap_file.seek(SeekFrom::Start(0)).is_err()
    {
        return Err(abort_xar_temp_heap(
            &mut heap_guard,
            heap_file,
            TTZipStatus::ErrCompressionFailed,
            "xar heap identity or rewind failed".to_string(),
        ));
    }
    let (hdr, toc_comp, toc_hash) = match encode_xar_toc_and_header(&tree_root, comp_level) {
        Ok(v) => v,
        Err(status) => {
            return Err(abort_xar_temp_heap(
                &mut heap_guard,
                heap_file,
                status,
                "xar toc encode failed".to_string(),
            ));
        }
    };
    Ok(PreparedXarHeap {
        heap_guard,
        heap_file,
        hdr,
        toc_comp,
        toc_hash,
        processed_bytes,
        total_bytes,
    })
}

pub(crate) fn write_xar_entries_in_txn(
    entries: &[(PathBuf, String)],
    destination_path: &Path,
    options: &TTZipCreateOptions,
    split_volume_size_bytes: u64,
    txn_ctx: Option<&CreateTransactionContext>,
) -> Result<(), TTZipStatus> {
    let PreparedXarHeap {
        mut heap_guard,
        mut heap_file,
        hdr,
        toc_comp,
        toc_hash,
        processed_bytes,
        total_bytes,
    } = prepare_xar_temp_heap_and_toc(entries, destination_path, options, txn_ctx)?;

    let mut target = match crate::archive::split::StreamOutputTarget::open_in_txn(
        destination_path,
        split_volume_size_bytes,
        txn_ctx,
    ) {
        Ok(t) => t,
        Err(e) => {
            return Err(abort_xar_temp_heap(
                &mut heap_guard,
                heap_file,
                TTZipStatus::ErrOpenFailed,
                format!("xar target open failed: {e}"),
            ));
        }
    };
    let stream_res = stream_xar_sections_to_target(
        &mut target,
        &hdr,
        &toc_comp,
        &toc_hash,
        &mut heap_file,
        options,
        processed_bytes,
        total_bytes,
    );
    let heap_finish_res =
        heap_guard.finish_readback_and_cleanup_in_txn(heap_file, stream_res.clone());
    if let Err((status, _)) = stream_res {
        target.abort_in_txn(status);
        return Err(status);
    }
    if let Err(err) = heap_finish_res {
        target.abort_in_txn(err.primary_status);
        return Err(err.primary_status);
    }
    target.finish_and_settle_in_txn(Ok(()))
}
