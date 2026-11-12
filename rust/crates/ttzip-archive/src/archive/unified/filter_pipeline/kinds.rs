// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! FilterKind enum and outer envelope signature recognition.

/// Supported stream compression and outer shell filter kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilterKind {
    /// Gzip (RFC 1952, `\x1F\x8B`)
    Gzip,
    /// Bzip2 (`BZh`)
    Bzip2,
    /// XZ (`\xFD7zXZ\x00`)
    Xz,
    /// Zstandard (`\x28\xB5\x2F\xFD`)
    Zstd,
    /// LZ4 Frame (`\x04\x22\x4D\x18`) or Legacy Frame (`\x02\x21\x4C\x18`)
    Lz4,
    /// Lzip (`LZIP`)
    Lzip,
    /// Lzop (`\x89LZO\0\r\n\x1a\n`)
    Lzop,
    /// Unix compress (`.Z`, `\x1F\x9D`)
    Compress,
    /// UUEncoded ASCII envelope (`begin `)
    Uuencode,
    /// RedHat Package Manager envelope (`\xED\xAB\xEE\xDB`)
    Rpm,
    /// Brotli (`\xCE\xB2\xCF\x81`)
    Brotli,
    /// Framed Snappy (`\xFF\x06\x00\x00sNaPpY`)
    Snappy,
    /// Apple LZFSE (`bvx-`, `bvx1`, `bvx2`, `bvxn`)
    Lzfse,
}

const MAGIC_PREFIX_FILTERS: &[(&[u8], FilterKind)] = &[
    (&[0xED, 0xAB, 0xEE, 0xDB], FilterKind::Rpm),
    (&[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00], FilterKind::Xz),
    (&[0x28, 0xB5, 0x2F, 0xFD], FilterKind::Zstd),
    (&[0x1F, 0x8B], FilterKind::Gzip),
    (&[0x1F, 0x9D], FilterKind::Compress),
    (b"BZh", FilterKind::Bzip2),
    (&[0x89, 0x4C, 0x5A, 0x4F], FilterKind::Lzop),
    (b"LZIP", FilterKind::Lzip),
    (&[0x04, 0x22, 0x4D, 0x18], FilterKind::Lz4),
    (&[0x02, 0x21, 0x4C, 0x18], FilterKind::Lz4),
    (b"\xFF\x06\x00\x00sNaPpY", FilterKind::Snappy),
    (b"bvx-", FilterKind::Lzfse),
    (b"bvx1", FilterKind::Lzfse),
    (b"bvx2", FilterKind::Lzfse),
    (b"bvxn", FilterKind::Lzfse),
    (&[0xCE, 0xB2, 0xCF, 0x81], FilterKind::Brotli),
];

impl FilterKind {
    /// Returns the canonical lower-case name of the filter.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gzip => "gzip",
            Self::Bzip2 => "bzip2",
            Self::Xz => "xz",
            Self::Zstd => "zstd",
            Self::Lz4 => "lz4",
            Self::Lzip => "lzip",
            Self::Lzop => "lzop",
            Self::Compress => "compress",
            Self::Uuencode => "uuencode",
            Self::Rpm => "rpm",
            Self::Brotli => "brotli",
            Self::Snappy => "snappy",
            Self::Lzfse => "lzfse",
        }
    }

    /// Sniffs input buffer for leading filter signatures.
    #[must_use]
    pub fn sniff(buffer: &[u8]) -> Option<Self> {
        if buffer.is_empty() {
            return None;
        }

        if is_uuencode_header(buffer) {
            return Some(Self::Uuencode);
        }

        for &(magic, kind) in MAGIC_PREFIX_FILTERS {
            if buffer.starts_with(magic) {
                return Some(kind);
            }
        }

        None
    }
}

/// Helper function to check if buffer begins with a UUEncoded header line (`begin <mode> <filename>`).
pub(crate) fn is_uuencode_header(buffer: &[u8]) -> bool {
    let limit = buffer.len().min(512);
    let slice = &buffer[..limit];
    let trimmed = match slice.iter().position(|&b| b != b'\r' && b != b'\n' && b != b' ' && b != b'\t') {
        Some(pos) => &slice[pos..],
        None => return false,
    };
    if trimmed.starts_with(b"begin ") || trimmed.starts_with(b"begin-base64 ") {
        return true;
    }
    false
}
