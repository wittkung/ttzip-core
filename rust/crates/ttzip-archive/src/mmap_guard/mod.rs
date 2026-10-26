// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Cross-Platform Memory-Mapped Hardware Fault Defense & Safe Slice Barrier.
//!
//! Provides signal-safe (POSIX `SIGBUS` / `SIGSEGV`) and exception-safe (Windows SEH
//! `STATUS_IN_PAGE_ERROR`) isolation for archive files during physical storage
//! disconnects (unplugged USB drives, severed SMB/NFS network mounts), preventing
//! OS hardware faults from abruptly crashing the main host application process.

pub mod posix;
pub mod slice;
pub mod windows;

pub use slice::SafeMmapSlice;
pub use windows::WindowsSehFaultContract;

#[cfg(unix)]
pub use posix::{
    ensure_signal_handlers_installed, is_guarded_address, register_guarded_range,
    unregister_guarded_range, with_mmap_guard, GuardToken,
};

/// Reads bytes from `src` into `dst` using the platform-specific hardware fault barrier.
///
/// - On POSIX (macOS & Linux): Runs within a `sigaction` signal barrier catching `SIGBUS` & `SIGSEGV`.
/// - On Windows: Wraps Win32 `ReadProcessMemory` / SEH barrier catching `STATUS_IN_PAGE_ERROR`.
///
/// # Errors
/// Returns `std::io::Error(ErrorKind::UnexpectedEof)` if a storage/hardware in-page fault occurs.
pub fn platform_safe_read(src: &[u8], dst: &mut [u8]) -> std::io::Result<usize> {
    #[cfg(unix)]
    {
        posix::safe_copy_bytes(src, dst)
    }
    #[cfg(windows)]
    {
        windows::safe_read_memory(src.as_ptr(), dst)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let to_copy = src.len().min(dst.len());
        dst[..to_copy].copy_from_slice(&src[..to_copy]);
        Ok(to_copy)
    }
}

/// Convenience helper to safely map an entire file with hardware fault protection.
pub fn safe_map_file(file: &std::fs::File) -> std::io::Result<SafeMmapSlice> {
    SafeMmapSlice::from_file_full(file)
}

/// Convenience helper to safely map a sub-range of a file with hardware fault protection.
pub fn safe_map_range(file: &std::fs::File, offset: u64, len: usize) -> std::io::Result<SafeMmapSlice> {
    SafeMmapSlice::from_file(file, offset, len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_safe_read_basic() {
        let src = b"METASTUDYLINE_MMAP_FAULT_BARRIER";
        let mut dst = [0u8; 14];
        let n = platform_safe_read(src, &mut dst).expect("safe read");
        assert_eq!(n, 14);
        assert_eq!(&dst, b"METASTUDYLINE_");
    }

    #[test]
    fn test_safe_map_helpers() {
        let empty = SafeMmapSlice::empty();
        assert_eq!(empty.len(), 0);
    }
}
