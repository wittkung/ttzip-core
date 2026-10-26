// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Libarchive Stream Reading adapter and C-ABI trampolines.

use super::seek::{archive_seek_callback_trampoline, archive_skip_callback_trampoline};
use super::{
    ARCHIVE_FATAL, ARCHIVE_OK, ARCHIVE_WARN, DEFAULT_STREAM_BUFFER_SIZE, MAX_STREAM_BUFFER_SIZE,
    StreamStateSnapshot,
};
use crate::types::TTZipStatus;
use std::io::{Read, Seek};
use std::pin::Pin;

/// Internal state for custom stream reading callbacks.
pub struct StreamReaderState<R> {
    pub reader: R,
    pub buffer: Vec<u8>,
    pub bytes_consumed: u64,
    pub is_eof: bool,
    pub has_error: bool,
    pub last_error_msg: Option<String>,
}

impl<R: Read> StreamReaderState<R> {
    /// Creates a new `StreamReaderState` with the specified micro-buffer capacity.
    pub fn new(reader: R, buffer_size: usize) -> Self {
        let cap = buffer_size.clamp(DEFAULT_STREAM_BUFFER_SIZE, MAX_STREAM_BUFFER_SIZE);
        Self {
            reader,
            buffer: vec![0u8; cap],
            bytes_consumed: 0,
            is_eof: false,
            has_error: false,
            last_error_msg: None,
        }
    }

    /// Reads the next chunk of data from the underlying reader into the internal buffer.
    pub fn read_chunk(&mut self) -> std::io::Result<(*const u8, usize)> {
        if self.is_eof {
            return Ok((self.buffer.as_ptr(), 0));
        }

        match self.reader.read(&mut self.buffer) {
            Ok(0) => {
                self.is_eof = true;
                Ok((self.buffer.as_ptr(), 0))
            }
            Ok(n) => {
                self.bytes_consumed = self.bytes_consumed.saturating_add(n as u64);
                Ok((self.buffer.as_ptr(), n))
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::Interrupted {
                    return self.read_chunk();
                }
                self.has_error = true;
                self.last_error_msg = Some(e.to_string());
                Err(e)
            }
        }
    }

    /// Reads data directly into a caller-supplied buffer, avoiding borrowing of internal state.
    pub fn read_into_buffer(&mut self, out_buf: &mut [u8]) -> std::io::Result<usize> {
        if self.is_eof || out_buf.is_empty() {
            return Ok(0);
        }

        match self.reader.read(out_buf) {
            Ok(0) => {
                self.is_eof = true;
                Ok(0)
            }
            Ok(n) => {
                self.bytes_consumed = self.bytes_consumed.saturating_add(n as u64);
                Ok(n)
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::Interrupted {
                    return self.read_into_buffer(out_buf);
                }
                self.has_error = true;
                self.last_error_msg = Some(e.to_string());
                Err(e)
            }
        }
    }

    /// Takes a point-in-time snapshot of the reader state.
    pub fn snapshot(&self) -> StreamStateSnapshot {
        StreamStateSnapshot {
            bytes_consumed: self.bytes_consumed,
            bytes_written: 0,
            is_eof: self.is_eof,
            has_error: self.has_error,
            last_error_msg: self.last_error_msg.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Libarchive Trampoline Callbacks (with Exception Barrier)
// ---------------------------------------------------------------------------

/// C-ABI read callback trampoline for `libarchive`.
pub unsafe extern "C" fn archive_read_callback_trampoline<R: Read>(
    _archive: *mut libc::c_void,
    client_data: *mut libc::c_void,
    buffer: *mut *const libc::c_void,
) -> libc::ssize_t {
    if client_data.is_null() || buffer.is_null() {
        return ARCHIVE_FATAL as libc::ssize_t;
    }
    let state = &mut *(client_data as *mut StreamReaderState<R>);
    match state.read_chunk() {
        Ok((ptr, len)) => {
            *buffer = ptr as *const libc::c_void;
            len as libc::ssize_t
        }
        Err(_) => ARCHIVE_FATAL as libc::ssize_t,
    }
}

/// C-ABI open callback trampoline for `libarchive`.
pub unsafe extern "C" fn archive_open_callback_trampoline(
    _archive: *mut libc::c_void,
    _client_data: *mut libc::c_void,
) -> libc::c_int {
    ARCHIVE_OK
}

/// C-ABI close callback trampoline for `libarchive`.
pub unsafe extern "C" fn archive_close_callback_trampoline(
    _archive: *mut libc::c_void,
    _client_data: *mut libc::c_void,
) -> libc::c_int {
    ARCHIVE_OK
}

// ---------------------------------------------------------------------------
// External Libarchive Read C-ABI declarations
// ---------------------------------------------------------------------------

extern "C" {
    pub fn archive_read_new() -> *mut libc::c_void;
    pub fn archive_read_support_format_all(a: *mut libc::c_void) -> libc::c_int;
    pub fn archive_read_support_format_raw(a: *mut libc::c_void) -> libc::c_int;
    pub fn archive_read_support_filter_all(a: *mut libc::c_void) -> libc::c_int;
    pub fn archive_read_open2(
        a: *mut libc::c_void,
        client_data: *mut libc::c_void,
        opener: Option<unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_void) -> libc::c_int>,
        reader: Option<
            unsafe extern "C" fn(
                *mut libc::c_void,
                *mut libc::c_void,
                *mut *const libc::c_void,
            ) -> libc::ssize_t,
        >,
        skipper: Option<
            unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_void, i64) -> i64,
        >,
        closer: Option<unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_void) -> libc::c_int>,
    ) -> libc::c_int;
    pub fn archive_read_set_seek_callback(
        a: *mut libc::c_void,
        seeker: Option<
            unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_void, i64, libc::c_int) -> i64,
        >,
    ) -> libc::c_int;
    pub fn archive_read_close(a: *mut libc::c_void) -> libc::c_int;
    pub fn archive_read_free(a: *mut libc::c_void) -> libc::c_int;
}

// ---------------------------------------------------------------------------
// Safe RAII Stream Pipeline Handles
// ---------------------------------------------------------------------------

/// Pinned Safe RAII wrapper for reading archives via custom `Read` / `Seek` streams.
pub struct ArchiveStreamReader<R> {
    archive_ptr: *mut libc::c_void,
    state: Pin<Box<StreamReaderState<R>>>,
}

impl<R: Read + 'static> ArchiveStreamReader<R> {
    /// Creates and opens a new sequential container `ArchiveStreamReader`.
    pub fn open_sequential(reader: R, buffer_size: usize) -> Result<Self, TTZipStatus> {
        let mut state = Box::pin(StreamReaderState::new(reader, buffer_size));
        unsafe {
            let a = archive_read_new();
            if a.is_null() {
                return Err(TTZipStatus::ErrOutOfMemory);
            }
            archive_read_support_format_all(a);
            archive_read_support_filter_all(a);

            let state_raw: *mut StreamReaderState<R> = Pin::get_unchecked_mut(state.as_mut());
            let ret = archive_read_open2(
                a,
                state_raw as *mut libc::c_void,
                Some(archive_open_callback_trampoline),
                Some(archive_read_callback_trampoline::<R>),
                None,
                Some(archive_close_callback_trampoline),
            );

            if ret != ARCHIVE_OK && ret != ARCHIVE_WARN {
                archive_read_free(a);
                return Err(TTZipStatus::ErrOpenFailed);
            }

            Ok(Self {
                archive_ptr: a,
                state,
            })
        }
    }

    /// Creates and opens a raw-filter `ArchiveStreamReader` strictly for confirmed single-stream
    /// filter codecs (`.bz2`, `.xz`, `.lz4`, `.lz`, `.lrz`) after magic signature validation.
    pub fn open_raw_filter(reader: R, buffer_size: usize) -> Result<Self, TTZipStatus> {
        let mut state = Box::pin(StreamReaderState::new(reader, buffer_size));
        unsafe {
            let a = archive_read_new();
            if a.is_null() {
                return Err(TTZipStatus::ErrOutOfMemory);
            }
            archive_read_support_format_raw(a);
            archive_read_support_filter_all(a);

            let state_raw: *mut StreamReaderState<R> = Pin::get_unchecked_mut(state.as_mut());
            let ret = archive_read_open2(
                a,
                state_raw as *mut libc::c_void,
                Some(archive_open_callback_trampoline),
                Some(archive_read_callback_trampoline::<R>),
                None,
                Some(archive_close_callback_trampoline),
            );

            if ret != ARCHIVE_OK && ret != ARCHIVE_WARN {
                archive_read_free(a);
                return Err(TTZipStatus::ErrCorruptHeader);
            }

            Ok(Self {
                archive_ptr: a,
                state,
            })
        }
    }
}

impl<R: Read> ArchiveStreamReader<R> {
    /// Drains any remaining trailing padding/checksum trailer from the underlying stream after
    /// `libarchive` finishes reading an inner container (e.g. TAR zero blocks inside `.tar.gz`/`.tar.br`/`.tar.zst`).
    pub fn verify_underlying_eof(&mut self) -> Result<(), TTZipStatus> {
        if self.state.has_error {
            return Err(TTZipStatus::ErrCorruptHeader);
        }
        let state = unsafe { Pin::get_unchecked_mut(self.state.as_mut()) };
        if state.is_eof {
            return Ok(());
        }
        let mut scratch = [0u8; 16384];
        let mut drained = 0usize;
        const MAX_TRAILER_DRAIN: usize = 2 * 1024 * 1024;
        while drained < MAX_TRAILER_DRAIN {
            match state.reader.read(&mut scratch) {
                Ok(0) => {
                    state.is_eof = true;
                    break;
                }
                Ok(n) => {
                    drained = drained.saturating_add(n);
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(TTZipStatus::ErrCorruptHeader),
            }
        }
        Ok(())
    }
}

impl<R: Read + Seek + 'static> ArchiveStreamReader<R> {
    /// Creates and opens a new seekable container `ArchiveStreamReader`.
    pub fn open_seekable(reader: R, buffer_size: usize) -> Result<Self, TTZipStatus> {
        let mut state = Box::pin(StreamReaderState::new(reader, buffer_size));
        unsafe {
            let a = archive_read_new();
            if a.is_null() {
                return Err(TTZipStatus::ErrOutOfMemory);
            }
            archive_read_support_format_all(a);
            archive_read_support_filter_all(a);

            let state_raw: *mut StreamReaderState<R> = Pin::get_unchecked_mut(state.as_mut());
            archive_read_set_seek_callback(a, Some(archive_seek_callback_trampoline::<R>));

            let ret = archive_read_open2(
                a,
                state_raw as *mut libc::c_void,
                Some(archive_open_callback_trampoline),
                Some(archive_read_callback_trampoline::<R>),
                Some(archive_skip_callback_trampoline::<R>),
                Some(archive_close_callback_trampoline),
            );

            if ret != ARCHIVE_OK && ret != ARCHIVE_WARN {
                archive_read_free(a);
                return Err(TTZipStatus::ErrOpenFailed);
            }

            Ok(Self {
                archive_ptr: a,
                state,
            })
        }
    }
}

impl<R> ArchiveStreamReader<R> {
    /// Returns the underlying raw `libarchive` handle pointer.
    pub fn as_raw_archive(&self) -> *mut libc::c_void {
        self.archive_ptr
    }

    /// Returns the current stream state snapshot.
    pub fn snapshot(&self) -> StreamStateSnapshot {
        StreamStateSnapshot {
            bytes_consumed: self.state.bytes_consumed,
            bytes_written: 0,
            is_eof: self.state.is_eof,
            has_error: self.state.has_error,
            last_error_msg: self.state.last_error_msg.clone(),
        }
    }
}

impl<R> Drop for ArchiveStreamReader<R> {
    fn drop(&mut self) {
        if !self.archive_ptr.is_null() {
            unsafe {
                archive_read_close(self.archive_ptr);
                archive_read_free(self.archive_ptr);
            }
            self.archive_ptr = std::ptr::null_mut();
        }
    }
}
