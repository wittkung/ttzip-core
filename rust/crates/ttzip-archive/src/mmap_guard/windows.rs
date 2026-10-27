// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Windows SEH (Structured Exception Handling) Fault Awareness and Safe Memory Access.
//!
//! When external storage devices (USB drives, external SSDs) or network shares (SMB/NFS)
//! are disconnected while mapped in virtual memory, Windows triggers `STATUS_IN_PAGE_ERROR`
//! (0xC0000006) during page-in. This module defines the SEH fault contract and wraps
//! the Win32 `ReadProcessMemory` interface to safely read memory without causing
//! unhandled hardware exception crashes in the GUI process.

use std::io::{Error, ErrorKind, Result};

/// NTSTATUS / SEH Exception code for in-page read fault (0xC0000006).
pub const STATUS_IN_PAGE_ERROR: u32 = 0xC000_0006;

/// NTSTATUS / SEH Exception code for access violation (0xC0000005).
pub const STATUS_ACCESS_VIOLATION: u32 = 0xC000_0005;

/// NTSTATUS sub-status: physical device hardware data error (e.g. bad block / I/O CRC).
pub const STATUS_DEVICE_DATA_ERROR: u32 = 0xC000_009C;

/// NTSTATUS sub-status: physical drive disconnected or powered down.
pub const STATUS_DEVICE_NOT_CONNECTED: u32 = 0xC000_009D;

/// NTSTATUS sub-status: unexpected network disconnect (SMB/NFS share dropped).
pub const STATUS_UNEXPECTED_NETWORK_ERROR: u32 = 0xC000_00C4;

/// NTSTATUS sub-status: device does not exist or removed.
pub const STATUS_NO_SUCH_DEVICE: u32 = 0xC000_000E;

/// NTSTATUS sub-status: physical disk operation failed.
pub const STATUS_DISK_OPERATION_FAILED: u32 = 0xC000_016A;

/// NTSTATUS sub-status: physical file system metadata or data block corrupted.
pub const STATUS_FILE_CORRUPT_ERROR: u32 = 0xC000_0102;

/// NTSTATUS sub-status: device I/O error occurred.
pub const STATUS_IO_DEVICE_ERROR: u32 = 0xC000_0185;

/// Win32 WinError code for invalid access (998).
pub const ERROR_NOACCESS: u32 = 998;

/// Win32 WinError code for partial memory copy (299).
pub const ERROR_PARTIAL_COPY: u32 = 299;

/// Structured representation of a Windows SEH hardware memory fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowsSehFaultContract {
    /// Primary exception code (e.g. `STATUS_IN_PAGE_ERROR` or `STATUS_ACCESS_VIOLATION`).
    pub exception_code: u32,
    /// Faulting virtual memory address.
    pub fault_address: usize,
    /// Underlying NTSTATUS sub-code from `ExceptionInformation[2]`.
    pub ntstatus_code: u32,
    /// Whether the faulting instruction was an attempted write (`true`) or read (`false`).
    pub is_write: bool,
}

impl WindowsSehFaultContract {
    /// Constructs a new SEH fault descriptor.
    #[must_use]
    pub const fn new(
        exception_code: u32,
        fault_address: usize,
        ntstatus_code: u32,
        is_write: bool,
    ) -> Self {
        Self {
            exception_code,
            fault_address,
            ntstatus_code,
            is_write,
        }
    }

    /// Parses an SEH exception code and parameters array into a contract descriptor.
    ///
    /// On Windows, `EXCEPTION_RECORD::ExceptionInformation` for `STATUS_IN_PAGE_ERROR` contains:
    /// - `[0]`: 0 = read, 1 = write, 8 = execute
    /// - `[1]`: Faulting virtual address
    /// - `[2]`: Underlying NTSTATUS code causing page-in failure
    #[must_use]
    pub fn parse_from_record(exception_code: u32, exception_info: &[usize]) -> Option<Self> {
        if exception_code != STATUS_IN_PAGE_ERROR && exception_code != STATUS_ACCESS_VIOLATION {
            return None;
        }

        let is_write = exception_info.first().copied().unwrap_or(0) == 1;
        let fault_address = exception_info.get(1).copied().unwrap_or(0);
        let ntstatus_code = exception_info.get(2).copied().unwrap_or(0) as u32;

        Some(Self {
            exception_code,
            fault_address,
            ntstatus_code,
            is_write,
        })
    }

    /// Returns `true` if this fault represents an in-page storage failure.
    #[must_use]
    pub const fn is_in_page_error(&self) -> bool {
        self.exception_code == STATUS_IN_PAGE_ERROR
    }

    /// Resolves the diagnostic human-readable description of the underlying NTSTATUS.
    #[must_use]
    pub fn ntstatus_description(&self) -> &'static str {
        match self.ntstatus_code {
            STATUS_DEVICE_DATA_ERROR => "STATUS_DEVICE_DATA_ERROR (hardware I/O / CRC defect)",
            STATUS_DEVICE_NOT_CONNECTED => "STATUS_DEVICE_NOT_CONNECTED (external drive unplugged)",
            STATUS_UNEXPECTED_NETWORK_ERROR => {
                "STATUS_UNEXPECTED_NETWORK_ERROR (SMB/NFS partition lost)"
            }
            STATUS_NO_SUCH_DEVICE => "STATUS_NO_SUCH_DEVICE (device removed)",
            STATUS_DISK_OPERATION_FAILED => "STATUS_DISK_OPERATION_FAILED (controller fault)",
            STATUS_FILE_CORRUPT_ERROR => "STATUS_FILE_CORRUPT_ERROR (filesystem corrupt)",
            STATUS_IO_DEVICE_ERROR => "STATUS_IO_DEVICE_ERROR (hardware I/O failure)",
            _ => "UNKNOWN_NTSTATUS (unhandled in-page hardware failure)",
        }
    }

    /// Converts the SEH fault into a standard `std::io::Error(ErrorKind::UnexpectedEof)`.
    #[must_use]
    pub fn to_io_error(&self) -> Error {
        let op = if self.is_write { "write" } else { "read" };
        let msg = format!(
            "Windows mmap in-page hardware fault (code: {:#x}, addr: {:#x}, op: {}, substatus: {})",
            self.exception_code,
            self.fault_address,
            op,
            self.ntstatus_description()
        );
        Error::new(ErrorKind::UnexpectedEof, msg)
    }
}

#[cfg(windows)]
mod sys {
    use super::*;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn ReadProcessMemory(
            hProcess: *mut std::ffi::c_void,
            lpBaseAddress: *const std::ffi::c_void,
            lpBuffer: *mut std::ffi::c_void,
            nSize: usize,
            lpNumberOfBytesRead: *mut usize,
        ) -> i32;
        fn GetLastError() -> u32;
    }

    /// Reads memory safely using Win32 `ReadProcessMemory` to guard against `STATUS_IN_PAGE_ERROR`.
    pub fn win32_safe_read_memory(src: *const u8, dst: &mut [u8]) -> Result<usize> {
        let len = dst.len();
        if len == 0 {
            return Ok(0);
        }
        let mut bytes_read: usize = 0;
        let success = unsafe {
            ReadProcessMemory(
                GetCurrentProcess(),
                src as *const std::ffi::c_void,
                dst.as_mut_ptr() as *mut std::ffi::c_void,
                len,
                &mut bytes_read,
            )
        };

        if success != 0 {
            Ok(bytes_read)
        } else {
            let last_error = unsafe { GetLastError() };
            Err(Error::new(
                ErrorKind::UnexpectedEof,
                format!(
                    "Win32 SafeReadMemory failed (GetLastError: {}): storage in-page fault",
                    last_error
                ),
            ))
        }
    }
}

/// Safely reads memory on Windows using the Win32 `ReadProcessMemory` barrier.
/// On non-Windows platforms, executes direct copy or delegates to fallback barrier.
pub fn safe_read_memory(src: *const u8, dst: &mut [u8]) -> Result<usize> {
    #[cfg(windows)]
    {
        sys::win32_safe_read_memory(src, dst)
    }
    #[cfg(not(windows))]
    {
        let len = dst.len();
        if len == 0 {
            return Ok(0);
        }
        if src.is_null() {
            return Err(Error::new(ErrorKind::InvalidInput, "null source pointer"));
        }
        let src_slice = unsafe { std::slice::from_raw_parts(src, len) };
        dst.copy_from_slice(src_slice);
        Ok(len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status_in_page_error() {
        let args = [
            0usize,                   // read op
            0x0000_7FFF_1234_5000,    // fault address
            STATUS_DEVICE_NOT_CONNECTED as usize, // subcode
        ];
        let fault = WindowsSehFaultContract::parse_from_record(STATUS_IN_PAGE_ERROR, &args)
            .expect("must parse");

        assert!(fault.is_in_page_error());
        assert_eq!(fault.fault_address, 0x0000_7FFF_1234_5000);
        assert_eq!(fault.ntstatus_code, STATUS_DEVICE_NOT_CONNECTED);
        assert!(!fault.is_write);
        assert!(fault.ntstatus_description().contains("STATUS_DEVICE_NOT_CONNECTED"));

        let io_err = fault.to_io_error();
        assert_eq!(io_err.kind(), ErrorKind::UnexpectedEof);
        assert!(io_err.to_string().contains("0xc0000006"));
    }

    #[test]
    fn test_parse_network_disconnected_fault() {
        let args = [0usize, 0x1000_0000, STATUS_UNEXPECTED_NETWORK_ERROR as usize];
        let fault = WindowsSehFaultContract::parse_from_record(STATUS_IN_PAGE_ERROR, &args).unwrap();
        assert!(fault.ntstatus_description().contains("SMB/NFS"));
        let err = fault.to_io_error();
        assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
    }

    #[test]
    fn test_safe_read_memory_non_windows_fallback() {
        let data = [42u8, 43, 44, 45];
        let mut out = [0u8; 4];
        let res = safe_read_memory(data.as_ptr(), &mut out);
        assert!(res.is_ok());
        assert_eq!(out, data);
    }

    #[test]
    fn test_safe_read_memory_null_rejection() {
        let mut out = [0u8; 4];
        let res = safe_read_memory(std::ptr::null(), &mut out);
        assert!(res.is_err());
    }
}
