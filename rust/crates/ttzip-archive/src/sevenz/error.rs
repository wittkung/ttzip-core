// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Format-specific 7z error models and domain failure definitions.

use thiserror::Error;
use ttzip_types::status::TTZipStatus;
use ttzip_crypto::sevenz_kdf::SevenZKdfError;
pub use ttzip_types::ErrorChain;

/// Errors that can occur during 7z Coder DAG construction, stream extraction, and archive processing.
#[derive(Debug, Error)]
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
    Io(#[from] std::io::Error),

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

impl SevenZError {
    /// Returns a zero-allocation iterator over the causal chain of errors.
    #[inline]
    #[must_use]
    pub fn chain(&self) -> ErrorChain<'_> {
        ErrorChain::new(self)
    }

    /// Downcasts by reference to a specific error type along the causal source chain.
    #[inline]
    #[must_use]
    pub fn downcast_ref<T: std::error::Error + 'static>(&self) -> Option<&T> {
        self.chain().find_map(|err| err.downcast_ref::<T>())
    }

    /// Returns true if this error or any error in its causal chain matches the target type `T`.
    #[inline]
    #[must_use]
    pub fn is<T: std::error::Error + 'static>(&self) -> bool {
        self.downcast_ref::<T>().is_some()
    }

    /// Navigates the causal chain to locate the bottom-most root cause.
    #[inline]
    #[must_use]
    pub fn root_cause(&self) -> &(dyn std::error::Error + 'static) {
        let mut current: &(dyn std::error::Error + 'static) = self;
        while let Some(cause) = current.source() {
            current = cause;
        }
        current
    }

    /// Borrows the underlying standard I/O error if this failure was caused by I/O.
    #[inline]
    #[must_use]
    pub fn as_io_error(&self) -> Option<&std::io::Error> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }

    /// Mutably borrows the underlying standard I/O error if this failure was caused by I/O.
    #[inline]
    #[must_use]
    pub fn as_io_error_mut(&mut self) -> Option<&mut std::io::Error> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }

    /// Extracts the underlying standard I/O error by value if this failure was caused by I/O.
    #[inline]
    #[must_use]
    pub fn into_io_error(self) -> Option<std::io::Error> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }

    /// Returns true if this error was caused by an underlying I/O failure.
    #[inline]
    #[must_use]
    pub fn is_io(&self) -> bool {
        self.as_io_error().is_some()
    }
}

impl Clone for SevenZError {
    fn clone(&self) -> Self {
        match self {
            Self::CyclicBindPairs => Self::CyclicBindPairs,
            Self::SelfLoop { coder_index } => Self::SelfLoop { coder_index: *coder_index },
            Self::InvalidCoderIndex { index, num_coders } => Self::InvalidCoderIndex { index: *index, num_coders: *num_coders },
            Self::InvalidStreamIndex { stream_index, max_streams } => Self::InvalidStreamIndex { stream_index: *stream_index, max_streams: *max_streams },
            Self::InvalidSubstreamIndex { index, total } => Self::InvalidSubstreamIndex { index: *index, total: *total },
            Self::StreamAlreadyBound { stream_index } => Self::StreamAlreadyBound { stream_index: *stream_index },
            Self::CorruptHeader(s) => Self::CorruptHeader(s),
            Self::CrcMismatch { expected, computed } => Self::CrcMismatch { expected: *expected, computed: *computed },
            Self::UnexpectedEof { required, actual } => Self::UnexpectedEof { required: *required, actual: *actual },
            Self::NoMainCoder => Self::NoMainCoder,
            Self::EmptyFolder => Self::EmptyFolder,
            Self::CountLimitExceeded { field_name, value, limit } => Self::CountLimitExceeded { field_name, value: *value, limit: *limit },
            Self::InsecurePath(p) => Self::InsecurePath(p.clone()),
            Self::CryptoExhaustion => Self::CryptoExhaustion,
            Self::CompressionError(s) => Self::CompressionError(s.clone()),
            Self::Io(err) => Self::Io(std::io::Error::new(err.kind(), err.to_string())),
            Self::InvalidParameter(s) => Self::InvalidParameter(s.clone()),
            Self::BadPassword => Self::BadPassword,
            Self::MaybeBadPassword => Self::MaybeBadPassword,
            Self::UnsupportedCodec(id) => Self::UnsupportedCodec(*id),
            Self::DecompressionFailed(s) => Self::DecompressionFailed(s.clone()),
            Self::OutOfMemory => Self::OutOfMemory,
        }
    }
}

impl PartialEq for SevenZError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::CyclicBindPairs, Self::CyclicBindPairs) => true,
            (Self::SelfLoop { coder_index: a }, Self::SelfLoop { coder_index: b }) => a == b,
            (Self::InvalidCoderIndex { index: a1, num_coders: a2 }, Self::InvalidCoderIndex { index: b1, num_coders: b2 }) => a1 == b1 && a2 == b2,
            (Self::InvalidStreamIndex { stream_index: a1, max_streams: a2 }, Self::InvalidStreamIndex { stream_index: b1, max_streams: b2 }) => a1 == b1 && a2 == b2,
            (Self::InvalidSubstreamIndex { index: a1, total: a2 }, Self::InvalidSubstreamIndex { index: b1, total: b2 }) => a1 == b1 && a2 == b2,
            (Self::StreamAlreadyBound { stream_index: a }, Self::StreamAlreadyBound { stream_index: b }) => a == b,
            (Self::CorruptHeader(a), Self::CorruptHeader(b)) => a == b,
            (Self::CrcMismatch { expected: a1, computed: a2 }, Self::CrcMismatch { expected: b1, computed: b2 }) => a1 == b1 && a2 == b2,
            (Self::UnexpectedEof { required: a1, actual: a2 }, Self::UnexpectedEof { required: b1, actual: b2 }) => a1 == b1 && a2 == b2,
            (Self::NoMainCoder, Self::NoMainCoder) => true,
            (Self::EmptyFolder, Self::EmptyFolder) => true,
            (Self::CountLimitExceeded { field_name: a1, value: a2, limit: a3 }, Self::CountLimitExceeded { field_name: b1, value: b2, limit: b3 }) => a1 == b1 && a2 == b2 && a3 == b3,
            (Self::InsecurePath(a), Self::InsecurePath(b)) => a == b,
            (Self::CryptoExhaustion, Self::CryptoExhaustion) => true,
            (Self::CompressionError(a), Self::CompressionError(b)) => a == b,
            (Self::Io(a), Self::Io(b)) => a.kind() == b.kind() && a.to_string() == b.to_string(),
            (Self::InvalidParameter(a), Self::InvalidParameter(b)) => a == b,
            (Self::BadPassword, Self::BadPassword) => true,
            (Self::MaybeBadPassword, Self::MaybeBadPassword) => true,
            (Self::UnsupportedCodec(a), Self::UnsupportedCodec(b)) => a == b,
            (Self::DecompressionFailed(a), Self::DecompressionFailed(b)) => a == b,
            (Self::OutOfMemory, Self::OutOfMemory) => true,
            _ => false,
        }
    }
}

impl Eq for SevenZError {}

impl From<SevenZKdfError> for SevenZError {
    #[inline]
    fn from(err: SevenZKdfError) -> Self {
        match err {
            SevenZKdfError::CryptoExhaustion => SevenZError::CryptoExhaustion,
        }
    }
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
            SevenZError::Io(_) => TTZipStatus::ErrOpenFailed,
            SevenZError::InvalidParameter(_) => TTZipStatus::ErrInvalidParam,
        }
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
            TTZipStatus::ErrOpenFailed => {
                SevenZError::Io(std::io::Error::other("open failed"))
            }
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

impl From<SevenZError> for std::io::Error {
    fn from(err: SevenZError) -> Self {
        match err {
            SevenZError::Io(io_err) => io_err,
            err => std::io::Error::new(std::io::ErrorKind::InvalidData, err),
        }
    }
}

// Compile-time barrier ensuring SevenZError concurrency boundaries and memory bounds
const _: () = {
    const fn assert_send_sync<T: Send + Sync + 'static>() {}
    const fn assert_clone<T: Clone>() {}
    const fn assert_unpin<T: Unpin>() {}
    const fn assert_eq_bounds<T: Eq>() {}
    const fn assert_from<T, U>()
    where
        U: From<T>,
    {
    }

    assert_send_sync::<SevenZError>();
    assert_clone::<SevenZError>();
    assert_unpin::<SevenZError>();
    assert_eq_bounds::<SevenZError>();

    assert_from::<std::io::Error, SevenZError>();
    assert_from::<SevenZError, std::io::Error>();
    assert_from::<SevenZKdfError, SevenZError>();

    assert!(std::mem::size_of::<SevenZError>() <= 64);
    assert!(std::mem::align_of::<SevenZError>() <= 8);
};
