// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

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

        // 1. UUEncode check: "begin " at start or after leading whitespaces/newlines
        if is_uuencode_header(buffer) {
            return Some(Self::Uuencode);
        }

        // 2. RPM check: \xED\xAB\xEE\xDB (RPM Lead)
        if buffer.len() >= 4 && buffer[..4] == [0xED, 0xAB, 0xEE, 0xDB] {
            return Some(Self::Rpm);
        }

        // 3. XZ check: \xFD7zXZ\x00
        if buffer.len() >= 6 && buffer[..6] == [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00] {
            return Some(Self::Xz);
        }

        // 4. Zstd check: \x28\xB5\x2F\xFD
        if buffer.len() >= 4 && buffer[..4] == [0x28, 0xB5, 0x2F, 0xFD] {
            return Some(Self::Zstd);
        }

        // 5. Gzip check: \x1F\x8B
        if buffer.len() >= 2 && buffer[..2] == [0x1F, 0x8B] {
            return Some(Self::Gzip);
        }

        // 6. Unix compress check: \x1F\x9D
        if buffer.len() >= 2 && buffer[..2] == [0x1F, 0x9D] {
            return Some(Self::Compress);
        }

        // 7. Bzip2 check: BZh
        if buffer.len() >= 3 && &buffer[..3] == b"BZh" {
            return Some(Self::Bzip2);
        }

        // 8. Lzop check: \x89LZO\0\r\n\x1a\n (9 bytes)
        if buffer.len() >= 4 && buffer[..4] == [0x89, 0x4C, 0x5A, 0x4F] {
            return Some(Self::Lzop);
        }

        // 9. Lzip check: LZIP
        if buffer.len() >= 4 && &buffer[..4] == b"LZIP" {
            return Some(Self::Lzip);
        }

        // 10. LZ4 Frame check
        if buffer.len() >= 4
            && (buffer[..4] == [0x04, 0x22, 0x4D, 0x18] || buffer[..4] == [0x02, 0x21, 0x4C, 0x18])
        {
            return Some(Self::Lz4);
        }

        // 11. Snappy check: \xFF\x06\x00\x00sNaPpY
        if buffer.len() >= 10 && &buffer[..10] == b"\xFF\x06\x00\x00sNaPpY" {
            return Some(Self::Snappy);
        }

        // 12. LZFSE check: bvx- or bvx1/2/n
        if buffer.len() >= 4
            && (&buffer[..4] == b"bvx-"
                || &buffer[..4] == b"bvx1"
                || &buffer[..4] == b"bvx2"
                || &buffer[..4] == b"bvxn")
        {
            return Some(Self::Lzfse);
        }

        // 13. Brotli stream signature
        if buffer.len() >= 4 && buffer[..4] == [0xCE, 0xB2, 0xCF, 0x81] {
            return Some(Self::Brotli);
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
