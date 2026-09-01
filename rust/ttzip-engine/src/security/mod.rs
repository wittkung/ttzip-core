// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Security, sandbox isolation, path defense, and threat scanning modules.

pub mod acl;
pub mod blake3_defense;
pub mod brotli_defense;
pub mod bzip2_defense;
pub mod deflate_ng_defense;
pub mod ed25519_defense;
pub mod libdeflate_defense;
pub mod license;
pub mod lzfse_defense;
pub mod mmap_defense;
pub mod path_sanitizer;
pub mod secure_extract;
pub mod snappy_defense;
pub mod syntax_defense;
pub mod tar_defense;
pub mod text_encoding_defense;
pub mod uniffi_defense;
pub mod xml_defense;
pub mod xz_defense;
pub mod zip_defense;
pub mod zopfli_defense;

#[cfg(test)]
mod tests;

pub use acl::*;
pub use blake3_defense::*;
pub use brotli_defense::*;
pub use bzip2_defense::*;
pub use deflate_ng_defense::*;
pub use ed25519_defense::*;
pub use libdeflate_defense::*;
pub use license::*;
pub use lzfse_defense::*;
pub use mmap_defense::*;
pub use path_sanitizer::*;
pub use secure_extract::*;
pub use snappy_defense::*;
pub use syntax_defense::*;
pub use tar_defense::*;
pub use text_encoding_defense::*;
pub use uniffi_defense::*;
pub use xml_defense::*;
pub use xz_defense::*;
pub use zip_defense::*;
pub use zopfli_defense::*;
