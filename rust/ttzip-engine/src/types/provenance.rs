// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Kernel execution provenance, engine tagging, and hardware utilization telemetries.

use std::cell::RefCell;
use libc::c_char;
use serde::{Deserialize, Serialize};

use super::options::TTZIP_ABI_VERSION_2;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TTZipEngineTag {
    Unknown = 0,
    RustRayonParallelZip = 1,
    RustStreamingParallelZip = 2,
    RustZeroCopy7zDecoder = 3,
    RustPure7zEncoder = 4,
    RustTarStreamEngine = 5,
    RustInPlaceZip = 6,
    RustInPlaceSevenZip = 7,
    RustVfsParallelScanner = 8,
    LibarchiveLegacy = 100,
    Cli7zFallback = 101,
    SystemTarFallback = 102,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TTZipExecutionProvenance {
    pub struct_size: u32,
    pub abi_version: u32,
    pub engine_tag: TTZipEngineTag,
    pub thread_count: u32,
    pub uncompressed_bytes: u64,
    pub compressed_bytes: u64,
    pub kernel_duration_nanos: u64,
    pub is_fallback: bool,
    pub fallback_reason: [c_char; 128],
}

impl Default for TTZipExecutionProvenance {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            abi_version: TTZIP_ABI_VERSION_2,
            engine_tag: TTZipEngineTag::Unknown,
            thread_count: 1,
            uncompressed_bytes: 0,
            compressed_bytes: 0,
            kernel_duration_nanos: 0,
            is_fallback: false,
            fallback_reason: [0; 128],
        }
    }
}

thread_local! {
    static LAST_PROVENANCE: RefCell<TTZipExecutionProvenance> = const { RefCell::new(TTZipExecutionProvenance {
        struct_size: std::mem::size_of::<TTZipExecutionProvenance>() as u32,
        abi_version: TTZIP_ABI_VERSION_2,
        engine_tag: TTZipEngineTag::Unknown,
        thread_count: 1,
        uncompressed_bytes: 0,
        compressed_bytes: 0,
        kernel_duration_nanos: 0,
        is_fallback: false,
        fallback_reason: [0; 128],
    }) };
}

pub fn record_execution_provenance(prov: TTZipExecutionProvenance) {
    LAST_PROVENANCE.with(|cell| {
        *cell.borrow_mut() = prov;
    });
}

pub fn get_execution_provenance(out: *mut TTZipExecutionProvenance) -> bool {
    if out.is_null() {
        return false;
    }
    LAST_PROVENANCE.with(|cell| unsafe {
        *out = *cell.borrow();
    });
    true
}
