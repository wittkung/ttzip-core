// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Strongly typed error hierarchy for Android device communication and storage operations.

use thiserror::Error;

/// Error conditions encountered during Android device lifecycle, transport, and storage operations.
#[derive(Debug, Error)]
pub enum DeviceError {
    /// The specified Android device could not be found or identified.
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    /// A USB pipe stall occurred on the specified endpoint.
    #[error("USB pipe stall on endpoint 0x{0:02x}")]
    PipeStall(u8),

    /// Exclusive interface seizing failed (e.g., Apple IOKit `USBInterfaceOpenSeize` returned error).
    #[error("Failed to seize USB interface, kernel status: {0}")]
    InterfaceSeizeFailed(i32),

    /// Protocol communication timed out.
    #[error("Protocol timeout: {0}")]
    ProtocolTimeout(String),

    /// Operation was denied due to missing permissions or authorization.
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// The target Android device was abruptly disconnected.
    #[error("Device disconnected")]
    DeviceDisconnected,

    /// Available storage space on the partition is insufficient for the requested operation.
    #[error("Insufficient device storage: required {required_bytes} bytes, available {available_bytes} bytes")]
    InsufficientDeviceStorage {
        /// Number of bytes required to perform the transfer.
        required_bytes: u64,
        /// Number of bytes available on the target storage partition.
        available_bytes: u64,
    },

    /// Direct access to Android 11+ restricted paths (e.g. `/Android/data`) is blocked in MTP mode.
    #[error("Restricted directory access: {0}")]
    RestrictedDirectoryAccess(String),

    /// The provided path is invalid or failed sanitization checks.
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    /// Lower-level protocol framing, packet codec, or transaction error.
    #[error("Protocol error: {0}")]
    ProtocolError(String),

    /// Hardware transport or socket level failure.
    #[error("Transport error: {0}")]
    TransportError(String),

    /// Underlying standard I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
