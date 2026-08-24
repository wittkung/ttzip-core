// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Canonical C-ABI 2.0 Memory Management and Universal Deallocator.

use crate::types::{TTZipError, TTZipMemoryKind};
use libc::{c_char, c_void};
use std::ffi::CString;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

/// Canonical Universal Deallocator for all Rust-allocated resources across C-ABI boundaries.
///
/// Ensures memory safety and prevents undefined behavior / cross-allocator crashes.
/// Handlers are null-safe and catch-unwind safe.
#[no_mangle]
pub unsafe extern "C" fn ttzip_free(ptr: *mut c_void, kind: TTZipMemoryKind) {
    if ptr.is_null() {
        return;
    }

    let _ = catch_unwind(AssertUnwindSafe(|| match kind {
        TTZipMemoryKind::String => {
            drop(CString::from_raw(ptr as *mut c_char));
        }
        TTZipMemoryKind::Buffer => {
            libc::free(ptr);
        }
        TTZipMemoryKind::Aligned => {
            libc::free(ptr);
        }
        TTZipMemoryKind::Error => {
            drop(Box::from_raw(ptr as *mut TTZipError));
        }
        TTZipMemoryKind::VfsTree => {
            drop(Box::from_raw(ptr as *mut crate::ffi::fs_ffi::TTZipVfsTreeHandle));
        }
        TTZipMemoryKind::VfsCache => {
            drop(Box::from_raw(ptr as *mut crate::ffi::vfs_ffi::TTZipVfsCacheHandle));
        }
        TTZipMemoryKind::Filter => {
            drop(Box::from_raw(ptr as *mut crate::ffi::filter_ffi::TTZipFilterDslEngine));
        }
        TTZipMemoryKind::PathFilter => {
            drop(Box::from_raw(ptr as *mut crate::ffi::filter_ffi::TTZipPathFilterHandle));
        }
        TTZipMemoryKind::SplitReader => {
            drop(Box::from_raw(ptr as *mut crate::ffi::archive_ffi::split::reader::TTZipSplitReaderHandle));
        }
        TTZipMemoryKind::SplitWriter => {
            drop(Box::from_raw(ptr as *mut crate::ffi::archive_ffi::split::writer::TTZipSplitWriterHandle));
        }
        TTZipMemoryKind::StreamReader => {
            drop(Box::from_raw(ptr as *mut crate::ffi::stream_ffi::TTZipStreamReaderHandle));
        }
        TTZipMemoryKind::StreamWriter => {
            drop(Box::from_raw(ptr as *mut crate::ffi::stream_ffi::TTZipStreamWriterHandle));
        }
        TTZipMemoryKind::CancellationToken => {
            Arc::decrement_strong_count(ptr as *const crate::runtime::cancellation::CancellationToken);
        }
        TTZipMemoryKind::InPlaceSession => {
            drop(Box::from_raw(ptr as *mut crate::ffi::archive_ffi::TTZipInPlaceSession));
        }
    }));
}

// MARK: - Legacy Deprecated Free Aliases

#[no_mangle]
#[deprecated(since = "2.0.0", note = "Use ttzip_free(ptr, TTZipMemoryKind::String) instead")]
pub unsafe extern "C" fn ttzip_free_string(ptr: *mut c_char) {
    ttzip_free(ptr as *mut c_void, TTZipMemoryKind::String);
}

#[no_mangle]
#[deprecated(since = "2.0.0", note = "Use ttzip_free(err, TTZipMemoryKind::Error) instead")]
pub unsafe extern "C" fn ttzip_free_error(err: *mut TTZipError) {
    ttzip_free(err as *mut c_void, TTZipMemoryKind::Error);
}

#[no_mangle]
#[deprecated(since = "2.0.0", note = "Use ttzip_free(handle, TTZipMemoryKind::VfsTree) instead")]
pub unsafe extern "C" fn ttzip_free_vfs_tree(handle: *mut crate::ffi::fs_ffi::TTZipVfsTreeHandle) {
    ttzip_free(handle as *mut c_void, TTZipMemoryKind::VfsTree);
}

#[no_mangle]
#[deprecated(since = "2.0.0", note = "Use ttzip_free(handle, TTZipMemoryKind::VfsCache) instead")]
pub unsafe extern "C" fn ttzip_free_vfs_cache(handle: *mut crate::ffi::vfs_ffi::TTZipVfsCacheHandle) {
    ttzip_free(handle as *mut c_void, TTZipMemoryKind::VfsCache);
}

#[no_mangle]
#[deprecated(since = "2.0.0", note = "Use ttzip_free(engine, TTZipMemoryKind::Filter) instead")]
pub unsafe extern "C" fn ttzip_free_filter_dsl_engine(engine: *mut crate::ffi::filter_ffi::TTZipFilterDslEngine) {
    ttzip_free(engine as *mut c_void, TTZipMemoryKind::Filter);
}

#[no_mangle]
#[deprecated(since = "2.0.0", note = "Use ttzip_free(handle, TTZipMemoryKind::PathFilter) instead")]
pub unsafe extern "C" fn ttzip_free_path_filter(handle: *mut crate::ffi::filter_ffi::TTZipPathFilterHandle) {
    ttzip_free(handle as *mut c_void, TTZipMemoryKind::PathFilter);
}

#[no_mangle]
#[deprecated(since = "2.0.0", note = "Use ttzip_free(handle, TTZipMemoryKind::SplitReader) instead")]
pub unsafe extern "C" fn ttzip_free_split_reader(handle: *mut crate::ffi::archive_ffi::split::reader::TTZipSplitReaderHandle) {
    ttzip_free(handle as *mut c_void, TTZipMemoryKind::SplitReader);
}

#[no_mangle]
#[deprecated(since = "2.0.0", note = "Use ttzip_free(handle, TTZipMemoryKind::SplitWriter) instead")]
pub unsafe extern "C" fn ttzip_free_split_writer(handle: *mut crate::ffi::archive_ffi::split::writer::TTZipSplitWriterHandle) {
    ttzip_free(handle as *mut c_void, TTZipMemoryKind::SplitWriter);
}

#[no_mangle]
#[deprecated(since = "2.0.0", note = "Use ttzip_free(handle, TTZipMemoryKind::StreamReader) instead")]
pub unsafe extern "C" fn ttzip_free_stream_reader(handle: *mut crate::ffi::stream_ffi::TTZipStreamReaderHandle) {
    ttzip_free(handle as *mut c_void, TTZipMemoryKind::StreamReader);
}

#[no_mangle]
#[deprecated(since = "2.0.0", note = "Use ttzip_free(handle, TTZipMemoryKind::StreamWriter) instead")]
pub unsafe extern "C" fn ttzip_free_stream_writer(handle: *mut crate::ffi::stream_ffi::TTZipStreamWriterHandle) {
    ttzip_free(handle as *mut c_void, TTZipMemoryKind::StreamWriter);
}

#[no_mangle]
#[deprecated(since = "2.0.0", note = "Use ttzip_free(token, TTZipMemoryKind::CancellationToken) instead")]
pub unsafe extern "C" fn ttzip_free_cancellation_token(token: *const crate::runtime::cancellation::CancellationToken) {
    ttzip_free(token as *mut c_void, TTZipMemoryKind::CancellationToken);
}

#[no_mangle]
#[deprecated(since = "2.0.0", note = "Use ttzip_free(session, TTZipMemoryKind::InPlaceSession) instead")]
pub unsafe extern "C" fn ttzip_free_inplace_session(session: *mut crate::ffi::archive_ffi::TTZipInPlaceSession) {
    ttzip_free(session as *mut c_void, TTZipMemoryKind::InPlaceSession);
}
