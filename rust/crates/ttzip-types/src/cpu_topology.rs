// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Hardware CPU topology detection, Apple Silicon P-Core sensing, and thread budget resolution.
//!
//! Provides zero-cost, cached physical Performance Core (P-Core) detection on macOS
//! platforms via Mach/BSD `sysctlbyname("hw.perflevel0.physicalcpu")`, with robust
//! fallback paths for non-Apple Silicon and non-macOS platforms.
//!
//! # Architecture Invariant
//! Parallel compute operations (such as Rayon compression / decompression pools) must
//! bound their concurrency to `recommended_compute_threads()`:
//! `max(1, get_performance_cores().saturating_sub(1))`
//! This ensures that at least one P-Core and all Efficiency Cores (E-Cores) remain
//! unburdened, preventing starvation of OS scheduling, Swift cooperative thread pools,
//! and UI render loops.

use std::sync::OnceLock;

#[cfg(target_os = "macos")]
fn query_sysctl_u32(name: &[u8]) -> Option<u32> {
    let mut val: u32 = 0;
    let mut size = std::mem::size_of::<u32>();
    // Safety: `name` is a null-terminated static byte slice. `val` and `size` are stack-allocated,
    // properly aligned, and exclusively borrowed during the sysctlbyname syscall.
    let ret = unsafe {
        libc::sysctlbyname(
            name.as_ptr() as *const libc::c_char,
            &mut val as *mut _ as *mut libc::c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if ret == 0 && val > 0 {
        Some(val)
    } else {
        None
    }
}

fn fallback_parallelism() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Detects the physical performance core count.
fn detect_performance_cores() -> usize {
    #[cfg(target_os = "macos")]
    {
        if let Some(p_cores) = query_sysctl_u32(b"hw.perflevel0.physicalcpu\0") {
            return p_cores as usize;
        }
        // Fallback: If perflevel0 is not present (e.g. Intel Mac), try hw.physicalcpu
        if let Some(phys_cores) = query_sysctl_u32(b"hw.physicalcpu\0") {
            return phys_cores as usize;
        }
    }

    fallback_parallelism()
}

static CACHED_P_CORES: OnceLock<usize> = OnceLock::new();

/// Returns the detected physical Performance Core (P-Core) count of the current hardware platform.
///
/// On macOS Apple Silicon systems, this queries `hw.perflevel0.physicalcpu`.
/// On Intel Macs or non-macOS environments, this falls back to physical CPU count or
/// available parallelism reported by the standard library.
#[inline]
pub fn get_performance_cores() -> usize {
    *CACHED_P_CORES.get_or_init(detect_performance_cores)
}

/// Returns the recommended worker thread budget for compute-intensive pipelines.
///
/// Formula: `max(1, get_performance_cores().saturating_sub(1))`
///
/// This formula guarantees that parallel CPU-bound workers never saturate the entire
/// performance cluster, reserving compute headroom for operating system dispatching,
/// UI frame rendering, and asynchronous I/O scheduling.
#[inline]
pub fn recommended_compute_threads() -> usize {
    let p_cores = get_performance_cores();
    p_cores.saturating_sub(1).max(1)
}

/// Detects the physical efficiency core count if available.
pub fn get_efficiency_cores() -> usize {
    #[cfg(target_os = "macos")]
    {
        if let Some(e_cores) = query_sysctl_u32(b"hw.perflevel1.physicalcpu\0") {
            return e_cores as usize;
        }
    }
    0
}

/// Returns the total logical core count reported by the operating system.
#[inline]
pub fn get_total_logical_cores() -> usize {
    fallback_parallelism()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_cores_is_positive() {
        let p_cores = get_performance_cores();
        assert!(p_cores >= 1, "P-Cores must be at least 1, got {}", p_cores);
    }

    #[test]
    fn test_recommended_compute_threads_formula() {
        let p_cores = get_performance_cores();
        let recommended = recommended_compute_threads();
        let expected = p_cores.saturating_sub(1).max(1);
        assert_eq!(recommended, expected);
        assert!(recommended >= 1);
        assert!(recommended <= p_cores);
    }

    #[test]
    fn test_total_logical_cores_is_positive() {
        let total = get_total_logical_cores();
        assert!(total >= 1);
        assert!(total >= get_performance_cores().min(total));
    }

    #[test]
    fn test_cached_consistency() {
        let first = get_performance_cores();
        let second = get_performance_cores();
        assert_eq!(first, second);
    }
}
