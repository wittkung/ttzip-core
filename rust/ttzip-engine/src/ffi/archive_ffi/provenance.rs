// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

use std::os::raw::c_char;
use crate::types::{get_execution_provenance, TTZipEngineTag, TTZipExecutionProvenance};

#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_get_last_execution_provenance(
    out_provenance: *mut TTZipExecutionProvenance,
) -> bool {
    get_execution_provenance(out_provenance)
}

#[no_mangle]
pub extern "C" fn ttzip_rust_engine_tag_name(tag: TTZipEngineTag) -> *const c_char {
    match tag {
        TTZipEngineTag::RustRayonParallelZip => b"RustRayonParallelZip\0".as_ptr() as *const c_char,
        TTZipEngineTag::RustStreamingParallelZip => b"RustStreamingParallelZip\0".as_ptr() as *const c_char,
        TTZipEngineTag::RustZeroCopy7zDecoder => b"RustZeroCopy7zDecoder\0".as_ptr() as *const c_char,
        TTZipEngineTag::RustPure7zEncoder => b"RustPure7zEncoder\0".as_ptr() as *const c_char,
        TTZipEngineTag::RustTarStreamEngine => b"RustTarStreamEngine\0".as_ptr() as *const c_char,
        TTZipEngineTag::RustInPlaceZip => b"RustInPlaceZip\0".as_ptr() as *const c_char,
        TTZipEngineTag::RustInPlaceSevenZip => b"RustInPlaceSevenZip\0".as_ptr() as *const c_char,
        TTZipEngineTag::RustVfsParallelScanner => b"RustVfsParallelScanner\0".as_ptr() as *const c_char,
        TTZipEngineTag::LibarchiveLegacy => b"LibarchiveLegacy\0".as_ptr() as *const c_char,
        TTZipEngineTag::Cli7zFallback => b"Cli7zFallback\0".as_ptr() as *const c_char,
        TTZipEngineTag::SystemTarFallback => b"SystemTarFallback\0".as_ptr() as *const c_char,
        TTZipEngineTag::Unknown => b"Unknown\0".as_ptr() as *const c_char,
    }
}
