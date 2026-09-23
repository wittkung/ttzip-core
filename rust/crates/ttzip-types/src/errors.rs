// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Format-specific error models and domain failure definitions.

use thiserror::Error;
use crate::status::TTZipStatus;

/// Errors that can occur during 7z Coder DAG construction and topological sorting.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SevenZError {
    /// Cyclic dependency detected among coder bind pairs (deadlock).
    #[error("cyclic dependency detected in coder bind pairs")]
    CyclicBindPairs,

    /// Self-loop detected where a coder connects its output directly to its own input.
    #[error("self-loop detected on coder {coder_index}")]
    SelfLoop {
        /// Coder index exhibiting the self-loop.
        coder_index: usize,
    },

    /// Coder index is out of bounds.
    #[error("invalid coder index: {index} (total coders: {num_coders})")]
    InvalidCoderIndex {
        /// Provided index.
        index: usize,
        /// Total coders available in folder.
        num_coders: usize,
    },

    /// Stream index is out of bounds.
    #[error("invalid stream index: {stream_index} (total streams: {max_streams})")]
    InvalidStreamIndex {
        /// Provided stream index.
        stream_index: u64,
        /// Maximum allowed streams.
        max_streams: usize,
    },

    /// Substream index is out of bounds.
    #[error("invalid substream index: {index} (total substreams: {total})")]
    InvalidSubstreamIndex {
        /// Requested substream index.
        index: usize,
        /// Total substreams in folder.
        total: usize,
    },

    /// Input or output stream is bound more than once.
    #[error("stream {stream_index} is already bound")]
    StreamAlreadyBound {
        /// Stream index that was bound multiple times.
        stream_index: u64,
    },

    /// Corrupted 7z folder header metadata.
    #[error("corrupt 7z header: {0}")]
    CorruptHeader(&'static str),

    /// CRC32 mismatch during substream extraction.
    #[error("substream CRC32 mismatch: expected 0x{expected:08x}, computed 0x{computed:08x}")]
    CrcMismatch {
        /// Expected CRC32 value.
        expected: u32,
        /// Computed CRC32 value.
        computed: u32,
    },

    /// Unexpected end of stream.
    #[error("unexpected EOF in 7z stream: required {required} bytes, got {actual}")]
    UnexpectedEof {
        /// Required remaining byte count.
        required: u64,
        /// Actual byte count read.
        actual: u64,
    },

    /// No main terminal coder found in folder.
    #[error("no main coder found in folder")]
    NoMainCoder,

    /// Folder contains zero coders.
    #[error("empty folder: no coders present")]
    EmptyFolder,

    /// Bounded count exceeded limit (memory safety guard).
    #[error("bounded count limit exceeded for '{field_name}': {value} > {limit}")]
    CountLimitExceeded {
        /// Field name being checked.
        field_name: &'static str,
        /// Actual value from 7z archive.
        value: u64,
        /// Upper limit allowed.
        limit: usize,
    },

    /// Insecure or malicious archive entry path (Zip-Slip defense).
    #[error("insecure entry path: {0}")]
    InsecurePath(String),

    /// AES-256 KDF cycles power exceeds maximum security threshold (DoS / CPU exhaustion defense).
    #[error("7z AES-256 KDF cycles power exceeds maximum security threshold (max 24)")]
    CryptoExhaustion,

    /// Compression or encoding error.
    #[error("compression error: {0}")]
    CompressionError(String),

    /// Standard I/O operation error.
    #[error("i/o error: {0}")]
    Io(String),

    /// Standard I/O operation error (alias).
    #[error("i/o error: {0}")]
    IoError(String),

    /// Invalid parameter passed to 7z engine.
    #[error("invalid parameter: {0}")]
    InvalidParameter(String),

    /// Bad or incorrect password provided for encrypted archive.
    #[error("incorrect password for encrypted archive")]
    BadPassword,

    /// Password appears incorrect based on early probabilistic / heuristic checks.
    #[error("password check failed (likely incorrect password)")]
    MaybeBadPassword,

    /// Unsupported compression or encryption codec method ID.
    #[error("unsupported 7z codec method ID: {0:#x}")]
    UnsupportedCodec(u64),

    /// Decompression pipeline failure.
    #[error("decompression failed: {0}")]
    DecompressionFailed(String),

    /// Memory allocation budget exceeded (OOM protection).
    #[error("memory budget exceeded")]
    OutOfMemory,
}

impl From<SevenZError> for TTZipStatus {
    fn from(err: SevenZError) -> Self {
        match err {
            SevenZError::BadPassword | SevenZError::MaybeBadPassword => TTZipStatus::ErrInvalidPassword,
            SevenZError::CyclicBindPairs
            | SevenZError::SelfLoop { .. }
            | SevenZError::InvalidCoderIndex { .. }
            | SevenZError::InvalidStreamIndex { .. }
            | SevenZError::StreamAlreadyBound { .. }
            | SevenZError::CorruptHeader(_)
            | SevenZError::NoMainCoder
            | SevenZError::EmptyFolder
            | SevenZError::CrcMismatch { .. }
            | SevenZError::UnexpectedEof { .. } => TTZipStatus::ErrCorruptHeader,
            SevenZError::InvalidSubstreamIndex { .. } => TTZipStatus::ErrInvalidOffset,
            SevenZError::CountLimitExceeded { .. } | SevenZError::OutOfMemory => TTZipStatus::ErrOutOfMemory,
            SevenZError::InsecurePath(_) | SevenZError::CryptoExhaustion => TTZipStatus::ErrSecurityViolation,
            SevenZError::CompressionError(_) => TTZipStatus::ErrCompressionFailed,
            SevenZError::UnsupportedCodec(_) => TTZipStatus::ErrUnsupportedFeature,
            SevenZError::DecompressionFailed(_) => TTZipStatus::ErrExtractionFailed,
            SevenZError::Io(_) | SevenZError::IoError(_) => TTZipStatus::ErrOpenFailed,
            SevenZError::InvalidParameter(_) => TTZipStatus::ErrInvalidParam,
        }
    }
}

impl From<std::io::Error> for SevenZError {
    fn from(err: std::io::Error) -> Self {
        SevenZError::IoError(err.to_string())
    }
}

impl From<TTZipStatus> for SevenZError {
    fn from(st: TTZipStatus) -> Self {
        match st {
            TTZipStatus::ErrCorruptHeader => SevenZError::CorruptHeader("corrupt 7z header"),
            TTZipStatus::ErrInvalidPassword => SevenZError::BadPassword,
            TTZipStatus::ErrInvalidOffset => SevenZError::InvalidStreamIndex {
                stream_index: 0,
                max_streams: 0,
            },
            TTZipStatus::ErrOutOfMemory => SevenZError::OutOfMemory,
            TTZipStatus::ErrSecurityViolation => {
                SevenZError::InsecurePath("security violation".to_string())
            }
            TTZipStatus::ErrUnsupportedFeature => SevenZError::UnsupportedCodec(0),
            TTZipStatus::ErrExtractionFailed => {
                SevenZError::DecompressionFailed("extraction failed".to_string())
            }
            TTZipStatus::ErrOpenFailed => SevenZError::Io("open failed".to_string()),
            TTZipStatus::ErrInvalidParam => {
                SevenZError::InvalidParameter("invalid parameter".to_string())
            }
            TTZipStatus::ErrCompressionFailed => {
                SevenZError::CompressionError("compression failed".to_string())
            }
            _ => SevenZError::CorruptHeader("unexpected error status"),
        }
    }
}
