// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Libarchive raw C-ABI external symbols declarations.

use libc::{c_char, c_int, c_long, c_uint, c_void, mode_t, size_t, ssize_t, time_t};

pub type ArchiveOpenCallback = unsafe extern "C" fn(a: *mut c_void, client_data: *mut c_void) -> c_int;
pub type ArchiveReadCallback = unsafe extern "C" fn(a: *mut c_void, client_data: *mut c_void, buffer: *mut *const c_void) -> ssize_t;
pub type ArchiveSkipCallback = unsafe extern "C" fn(a: *mut c_void, client_data: *mut c_void, request: i64) -> i64;
pub type ArchiveCloseCallback = unsafe extern "C" fn(a: *mut c_void, client_data: *mut c_void) -> c_int;
pub type ArchiveSeekCallback = unsafe extern "C" fn(a: *mut c_void, client_data: *mut c_void, offset: i64, whence: c_int) -> i64;
pub type ArchiveWriteCallback = unsafe extern "C" fn(a: *mut c_void, client_data: *mut c_void, buffer: *const c_void, length: size_t) -> ssize_t;
pub type ArchiveFreeCallback = unsafe extern "C" fn(a: *mut c_void, client_data: *mut c_void) -> c_int;

#[allow(dead_code)]
extern "C" {
    pub(crate) fn archive_read_new() -> *mut c_void;
    pub(crate) fn archive_read_support_format_all(a: *mut c_void) -> c_int;
    pub(crate) fn archive_read_support_filter_all(a: *mut c_void) -> c_int;
    pub(crate) fn archive_read_add_passphrase(a: *mut c_void, passphrase: *const c_char) -> c_int;
    pub(crate) fn archive_read_open_filename(
        a: *mut c_void,
        filename: *const c_char,
        block_size: size_t,
    ) -> c_int;
    pub(crate) fn archive_read_open_memory(
        a: *mut c_void,
        buff: *const c_void,
        size: size_t,
    ) -> c_int;
    pub(crate) fn archive_read_open2(
        a: *mut c_void,
        client_data: *mut c_void,
        opener: Option<ArchiveOpenCallback>,
        reader: Option<ArchiveReadCallback>,
        skipper: Option<ArchiveSkipCallback>,
        closer: Option<ArchiveCloseCallback>,
    ) -> c_int;
    pub(crate) fn archive_read_set_seek_callback(
        a: *mut c_void,
        seeker: Option<ArchiveSeekCallback>,
    ) -> c_int;
    pub(crate) fn archive_read_next_header(a: *mut c_void, entry: *mut *mut c_void) -> c_int;
    pub(crate) fn archive_read_data(a: *mut c_void, buff: *mut c_void, len: size_t) -> ssize_t;
    pub(crate) fn archive_read_data_block(
        a: *mut c_void,
        buff: *mut *const c_void,
        size: *mut size_t,
        offset: *mut i64,
    ) -> c_int;
    pub(crate) fn archive_read_data_skip(a: *mut c_void) -> c_int;
    pub(crate) fn archive_read_close(a: *mut c_void) -> c_int;
    pub(crate) fn archive_read_free(a: *mut c_void) -> c_int;

    pub(crate) fn archive_entry_pathname(e: *mut c_void) -> *const c_char;
    pub(crate) fn archive_entry_size(e: *mut c_void) -> i64;
    pub(crate) fn archive_entry_filetype(e: *mut c_void) -> mode_t;
    pub(crate) fn archive_entry_mode(e: *mut c_void) -> mode_t;
    pub(crate) fn archive_entry_mtime(e: *mut c_void) -> time_t;
    pub(crate) fn archive_entry_symlink(e: *mut c_void) -> *const c_char;
    pub(crate) fn archive_entry_set_symlink(e: *mut c_void, symlink: *const c_char);
    pub(crate) fn archive_entry_is_data_encrypted(e: *mut c_void) -> c_int;
    pub(crate) fn archive_entry_is_metadata_encrypted(e: *mut c_void) -> c_int;

    pub(crate) fn archive_entry_new() -> *mut c_void;
    pub(crate) fn archive_entry_free(e: *mut c_void);
    pub(crate) fn archive_entry_set_pathname(e: *mut c_void, pathname: *const c_char);
    pub(crate) fn archive_entry_set_size(e: *mut c_void, size: i64);
    pub(crate) fn archive_entry_set_filetype(e: *mut c_void, filetype: c_uint);
    pub(crate) fn archive_entry_set_perm(e: *mut c_void, perm: mode_t);
    pub(crate) fn archive_entry_set_mtime(e: *mut c_void, mtime: time_t, nanos: c_long);

    pub(crate) fn archive_write_new() -> *mut c_void;
    pub(crate) fn archive_write_set_format_zip(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_set_format_pax_restricted(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_set_format_pax(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_set_format_gnutar(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_set_format_ustar(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_set_format_7zip(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_set_format_iso9660(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_set_format_cpio(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_set_format_cpio_newc(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_set_format_ar_bsd(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_set_format_ar_svr4(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_set_format_xar(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_set_format_raw(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_add_filter_gzip(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_add_filter_bzip2(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_add_filter_xz(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_add_filter_zstd(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_add_filter_lz4(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_add_filter_lzip(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_add_filter_lrzip(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_add_filter_compress(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_add_filter_none(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_set_filter_option(
        a: *mut c_void,
        module: *const c_char,
        option: *const c_char,
        value: *const c_char,
    ) -> c_int;
    pub(crate) fn archive_write_set_passphrase(a: *mut c_void, passphrase: *const c_char) -> c_int;
    pub(crate) fn archive_write_set_options(a: *mut c_void, opts: *const c_char) -> c_int;
    pub(crate) fn archive_write_open_filename(a: *mut c_void, filename: *const c_char) -> c_int;
    pub(crate) fn archive_write_open2(
        a: *mut c_void,
        client_data: *mut c_void,
        opener: Option<ArchiveOpenCallback>,
        writer: Option<ArchiveWriteCallback>,
        closer: Option<ArchiveCloseCallback>,
        freer: Option<ArchiveFreeCallback>,
    ) -> c_int;
    pub(crate) fn archive_write_header(a: *mut c_void, entry: *mut c_void) -> c_int;
    pub(crate) fn archive_write_data(a: *mut c_void, buff: *const c_void, len: size_t) -> ssize_t;
    pub(crate) fn archive_write_finish_entry(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_close(a: *mut c_void) -> c_int;
    pub(crate) fn archive_write_free(a: *mut c_void) -> c_int;

    pub(crate) fn archive_error_string(a: *mut c_void) -> *const c_char;
    pub(crate) fn archive_errno(a: *mut c_void) -> c_int;
}

/// Retrieves the libarchive error message as an `Option<String>`.
///
/// SAFETY: `a` must be a valid pointer to a libarchive struct or null.
#[allow(dead_code)]
pub(crate) unsafe fn get_archive_error_string(a: *mut c_void) -> Option<String> {
    if a.is_null() {
        return None;
    }
    let err_ptr = archive_error_string(a);
    if err_ptr.is_null() {
        return None;
    }
    std::ffi::CStr::from_ptr(err_ptr)
        .to_str()
        .ok()
        .map(|s| s.to_string())
}

/// Formats the libarchive error with errno and message into a readable string.
///
/// SAFETY: `a` must be a valid pointer to a libarchive struct or null.
#[allow(dead_code)]
pub(crate) unsafe fn format_archive_error(a: *mut c_void) -> String {
    if a.is_null() {
        return "libarchive handle is null".to_string();
    }
    let err_no = archive_errno(a);
    let err_str = get_archive_error_string(a).unwrap_or_else(|| "unknown error".to_string());
    if err_no != 0 {
        format!("libarchive error (errno {}): {}", err_no, err_str)
    } else {
        format!("libarchive error: {}", err_str)
    }
}
