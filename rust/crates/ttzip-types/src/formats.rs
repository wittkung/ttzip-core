// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Supported archive formats, compression levels, and encryption schemes.

pub use crate::archive_format::TTZipArchiveFormat;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TTZipCompressionLevel {
    Store = 0,
    Fastest = 1,
    Fast = 3,
    Normal = 6,
    Maximum = 9,
    Ultra = 12,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TTZipEncryptionMethod {
    None = 0,
    ZipCrypto = 1,
    Aes128 = 2,
    Aes192 = 3,
    Aes256 = 4,
}
