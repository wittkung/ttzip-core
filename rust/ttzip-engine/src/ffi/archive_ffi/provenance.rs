// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

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
        TTZipEngineTag::RustRayonParallelZip => c"RustRayonParallelZip".as_ptr(),
        TTZipEngineTag::RustStreamingParallelZip => c"RustStreamingParallelZip".as_ptr(),
        TTZipEngineTag::RustZeroCopy7zDecoder => c"RustZeroCopy7zDecoder".as_ptr(),
        TTZipEngineTag::RustPure7zEncoder => c"RustPure7zEncoder".as_ptr(),
        TTZipEngineTag::RustTarStreamEngine => c"RustTarStreamEngine".as_ptr(),
        TTZipEngineTag::RustInPlaceZip => c"RustInPlaceZip".as_ptr(),
        TTZipEngineTag::RustInPlaceSevenZip => c"RustInPlaceSevenZip".as_ptr(),
        TTZipEngineTag::RustVfsParallelScanner => c"RustVfsParallelScanner".as_ptr(),
        TTZipEngineTag::LibarchiveLegacy => c"LibarchiveLegacy".as_ptr(),
        TTZipEngineTag::Cli7zFallback => c"Cli7zFallback".as_ptr(),
        TTZipEngineTag::SystemTarFallback => c"SystemTarFallback".as_ptr(),
        TTZipEngineTag::Unknown => c"Unknown".as_ptr(),
    }
}
