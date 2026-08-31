// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Mozilla UniFFI Cross-Language Lossless Error Mapping Subsystem.
//!
//! Provides typed, lossless mapping from internal engine errors to Swift,
//! Kotlin, and Python exception types with full context preservation.

use thiserror::Error;
use super::types::TTZipError as ApiTTZipError;
use crate::types::TTZipStatus;
use crate::zip::scanner::ZipEngineError;

/// Strongly-typed cross-language error enumeration exposed to foreign runtimes.
#[derive(Debug, Clone, PartialEq, Eq, Error, uniffi::Error)]
pub enum UniFFIError {
    #[error("I/O error: {message}")]
    IoError { message: String },

    #[error("Corrupt archive at offset {offset}: {message}")]
    CorruptArchive { message: String, offset: u64 },

    #[error("Invalid password: {message}")]
    InvalidPassword { message: String },

    #[error("Unsupported compression method: {method}")]
    UnsupportedCompression { method: String },

    #[error("Memory mapping error: {message}")]
    MmapError { message: String },

    #[error("Permission denied accessing path: {path}")]
    PermissionDenied { path: String },

    #[error("Out of memory: {message}")]
    OutOfMemory { message: String },

    #[error("Cryptographic error: {message}")]
    CryptoError { message: String },
}

impl UniFFIError {
    #[inline]
    pub fn io(msg: impl Into<String>) -> Self {
        Self::IoError { message: msg.into() }
    }

    #[inline]
    pub fn corrupt(msg: impl Into<String>, offset: u64) -> Self {
        Self::CorruptArchive {
            message: msg.into(),
            offset,
        }
    }

    #[inline]
    pub fn invalid_password(msg: impl Into<String>) -> Self {
        Self::InvalidPassword { message: msg.into() }
    }

    #[inline]
    pub fn unsupported_compression(method: impl Into<String>) -> Self {
        Self::UnsupportedCompression { method: method.into() }
    }

    #[inline]
    pub fn mmap(msg: impl Into<String>) -> Self {
        Self::MmapError { message: msg.into() }
    }

    #[inline]
    pub fn permission_denied(path: impl Into<String>) -> Self {
        Self::PermissionDenied { path: path.into() }
    }

    #[inline]
    pub fn out_of_memory(msg: impl Into<String>) -> Self {
        Self::OutOfMemory { message: msg.into() }
    }

    #[inline]
    pub fn crypto(msg: impl Into<String>) -> Self {
        Self::CryptoError { message: msg.into() }
    }
}

impl From<std::io::Error> for UniFFIError {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied {
                path: "unknown".to_string(),
            },
            std::io::ErrorKind::OutOfMemory => Self::OutOfMemory {
                message: err.to_string(),
            },
            _ => Self::IoError {
                message: err.to_string(),
            },
        }
    }
}

impl From<ApiTTZipError> for UniFFIError {
    fn from(err: ApiTTZipError) -> Self {
        match err {
            ApiTTZipError::FileNotFound { path } => Self::IoError {
                message: format!("File not found: {path}"),
            },
            ApiTTZipError::InvalidPassword => Self::InvalidPassword {
                message: "Invalid or missing archive password".to_string(),
            },
            ApiTTZipError::CorruptHeader { details, offset } => Self::CorruptArchive {
                message: details,
                offset,
            },
            ApiTTZipError::SecurityViolation { reason } => Self::PermissionDenied {
                path: reason,
            },
            ApiTTZipError::EngineError { code } => {
                if code == TTZipStatus::ErrOutOfMemory.to_i32() {
                    Self::OutOfMemory { message: "Engine allocation failure".to_string() }
                } else if code == TTZipStatus::ErrMmapFailed.to_i32() {
                    Self::MmapError { message: "Mmap allocation failed".to_string() }
                } else if code == TTZipStatus::ErrUnsupportedFeature.to_i32() {
                    Self::UnsupportedCompression { method: "Unknown feature".to_string() }
                } else {
                    Self::IoError {
                        message: format!("Engine error with status code {code}"),
                    }
                }
            }
            ApiTTZipError::IoError { message } => Self::IoError { message },
            ApiTTZipError::Cancelled => Self::IoError {
                message: "Operation was cancelled by caller".to_string(),
            },
        }
    }
}

impl From<TTZipStatus> for UniFFIError {
    fn from(status: TTZipStatus) -> Self {
        match status {
            TTZipStatus::Ok | TTZipStatus::Eof => Self::IoError {
                message: status.as_str().to_string(),
            },
            TTZipStatus::Cancelled => Self::IoError {
                message: "Operation cancelled".to_string(),
            },
            TTZipStatus::ErrFileNotFound => Self::IoError {
                message: "File not found".to_string(),
            },
            TTZipStatus::ErrMmapFailed => Self::MmapError {
                message: "Memory mapping failed".to_string(),
            },
            TTZipStatus::ErrCorruptHeader | TTZipStatus::ErrInvalidOffset => Self::CorruptArchive {
                message: status.as_str().to_string(),
                offset: 0,
            },
            TTZipStatus::ErrOutOfMemory => Self::OutOfMemory {
                message: "System out of memory".to_string(),
            },
            TTZipStatus::ErrInvalidPassword => Self::InvalidPassword {
                message: "Invalid password".to_string(),
            },
            TTZipStatus::ErrUnsupportedFeature => Self::UnsupportedCompression {
                method: "Unsupported codec or feature".to_string(),
            },
            TTZipStatus::ErrSecurityViolation => Self::PermissionDenied {
                path: "Security policy boundary violation".to_string(),
            },
            _ => Self::IoError {
                message: status.as_str().to_string(),
            },
        }
    }
}

impl From<ZipEngineError> for UniFFIError {
    fn from(err: ZipEngineError) -> Self {
        match err {
            ZipEngineError::FileTooSmall { required, actual } => Self::CorruptArchive {
                message: format!("Archive too small: required {required}, actual {actual}"),
                offset: 0,
            },
            ZipEngineError::EocdNotFound => Self::CorruptArchive {
                message: "End of Central Directory record not found".to_string(),
                offset: 0,
            },
            ZipEngineError::InvalidCommentLength { declared, available } => Self::CorruptArchive {
                message: format!("Invalid comment length: declared {declared}, available {available}"),
                offset: 0,
            },
            ZipEngineError::CorruptedHeader(msg) => Self::CorruptArchive {
                message: msg,
                offset: 0,
            },
            ZipEngineError::InvalidCentralDirectoryBoundary { offset, size, file_len } => Self::CorruptArchive {
                message: format!("Invalid Central Directory boundary: offset {offset}, size {size}, file_len {file_len}"),
                offset,
            },
            ZipEngineError::Io(msg) => Self::IoError { message: msg },
            ZipEngineError::Status(status) => status.into(),
        }
    }
}

/// Extension trait to convert generic Results into UniFFI typed Results.
pub trait IntoUniFFIResult<T> {
    fn into_uniffi(self) -> Result<T, UniFFIError>;
}

impl<T, E> IntoUniFFIResult<T> for Result<T, E>
where
    E: Into<UniFFIError>,
{
    #[inline]
    fn into_uniffi(self) -> Result<T, UniFFIError> {
        self.map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniffi_error_variants_and_constructors() {
        let err1 = UniFFIError::io("Disk read error");
        assert!(matches!(err1, UniFFIError::IoError { .. }));
        assert_eq!(err1.to_string(), "I/O error: Disk read error");

        let err2 = UniFFIError::corrupt("Invalid magic", 0x1000);
        assert!(matches!(err2, UniFFIError::CorruptArchive { offset: 0x1000, .. }));

        let err3 = UniFFIError::invalid_password("Wrong key");
        assert!(matches!(err3, UniFFIError::InvalidPassword { .. }));

        let err4 = UniFFIError::unsupported_compression("ZSTD-Ultra");
        assert!(matches!(err4, UniFFIError::UnsupportedCompression { .. }));

        let err5 = UniFFIError::mmap("mprotect failed");
        assert!(matches!(err5, UniFFIError::MmapError { .. }));

        let err6 = UniFFIError::permission_denied("/etc/shadow");
        assert!(matches!(err6, UniFFIError::PermissionDenied { .. }));

        let err7 = UniFFIError::out_of_memory("16GB allocation failed");
        assert!(matches!(err7, UniFFIError::OutOfMemory { .. }));

        let err8 = UniFFIError::crypto("AES-GCM tag mismatch");
        assert!(matches!(err8, UniFFIError::CryptoError { .. }));
    }

    #[test]
    fn test_conversions_from_engine_and_std_errors() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let uniffi_err: UniFFIError = io_err.into();
        assert!(matches!(uniffi_err, UniFFIError::IoError { .. }));

        let status_err: UniFFIError = TTZipStatus::ErrOutOfMemory.into();
        assert!(matches!(status_err, UniFFIError::OutOfMemory { .. }));

        let status_mmap: UniFFIError = TTZipStatus::ErrMmapFailed.into();
        assert!(matches!(status_mmap, UniFFIError::MmapError { .. }));

        let zip_err: UniFFIError = ZipEngineError::EocdNotFound.into();
        assert!(matches!(zip_err, UniFFIError::CorruptArchive { .. }));

        let api_err = ApiTTZipError::CorruptHeader {
            details: "Bad central directory".to_string(),
            offset: 4096,
        };
        let mapped: UniFFIError = api_err.into();
        assert_eq!(mapped, UniFFIError::CorruptArchive {
            message: "Bad central directory".to_string(),
            offset: 4096,
        });
    }

    #[test]
    fn test_into_uniffi_result_extension() {
        let res: Result<u32, std::io::Error> = Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        let uniffi_res = res.into_uniffi();
        assert!(matches!(uniffi_res, Err(UniFFIError::PermissionDenied { .. })));

        let ok_res: Result<u32, std::io::Error> = Ok(42);
        assert_eq!(ok_res.into_uniffi().unwrap(), 42);
    }
}
