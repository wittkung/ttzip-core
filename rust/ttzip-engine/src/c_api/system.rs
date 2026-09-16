// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! C-ABI System & Telemetry Endpoints aligned with `sdk/include/ttzip.h`.
//!
//! Exposes hardware SIMD capabilities, CPU core topology, and execution provenance telemetry.

use crate::platform::cpu::{CpuCapabilities, TTZipCpuCapsRaw};
use crate::types::{
    get_execution_provenance, TTZipExecutionProvenance, TTZipStatus, TTZIP_ABI_VERSION_2,
};
use std::panic::catch_unwind;

/// C-ABI exported CPU hardware SIMD and feature detection capabilities.
///
/// # Safety
/// - `out_caps` must be a valid, aligned, writable pointer to `TTZipCpuCapsRaw`.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_cpu_get_capabilities(
    out_caps: *mut TTZipCpuCapsRaw,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        if out_caps.is_null() {
            return TTZipStatus::ErrInvalidParam;
        }
        let caps = CpuCapabilities::get();
        *out_caps = TTZipCpuCapsRaw {
            struct_size: std::mem::size_of::<TTZipCpuCapsRaw>() as u32,
            abi_version: TTZIP_ABI_VERSION_2,
            logical_cores: caps.logical_cores,
            physical_page_size: caps.physical_page_size,
            p_cores: caps.p_cores,
            e_cores: caps.e_cores,
            has_arm_neon: caps.has_arm_neon,
            has_arm_crypto: caps.has_arm_crypto,
            has_aes_ni: caps.has_aes_ni,
            has_avx2: caps.has_avx2,
            has_avx512: caps.has_avx512,
            has_hardware_crc32: caps.has_hardware_crc32,
        };
        TTZipStatus::Ok
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

/// C-ABI exported P-core, E-core, and total core topology counts.
///
/// # Safety
/// - If non-null, `out_p_cores`, `out_e_cores`, and `out_total_cores` must point to writable `u32`s.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_cpu_get_topology(
    out_p_cores: *mut u32,
    out_e_cores: *mut u32,
    out_total_cores: *mut u32,
) -> TTZipStatus {
    let result = catch_unwind(|| {
        let caps = CpuCapabilities::get();
        if !out_p_cores.is_null() {
            *out_p_cores = caps.p_cores;
        }
        if !out_e_cores.is_null() {
            *out_e_cores = caps.e_cores;
        }
        if !out_total_cores.is_null() {
            *out_total_cores = caps.logical_cores;
        }
        TTZipStatus::Ok
    });
    result.unwrap_or(TTZipStatus::ErrPanicCaught)
}

/// Retrieves the execution provenance telemetry from the last archive operation.
///
/// Returns `true` if provenance data was recorded and copied, or `false` otherwise.
///
/// # Safety
/// - `out_provenance` must be a valid, aligned, writable pointer to `TTZipExecutionProvenance`.
#[no_mangle]
pub unsafe extern "C" fn ttzip_rust_get_last_execution_provenance(
    out_provenance: *mut TTZipExecutionProvenance,
) -> bool {
    let result = catch_unwind(|| {
        if out_provenance.is_null() {
            return false;
        }
        get_execution_provenance(out_provenance)
    });
    result.unwrap_or(false)
}
