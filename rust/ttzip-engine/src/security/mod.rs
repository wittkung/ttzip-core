// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Security, sandbox isolation, path defense, and threat scanning modules.

pub mod acl;
pub mod brotli_defense;
pub mod bzip2_defense;
pub mod libdeflate_defense;
pub mod license;
pub mod lzfse_defense;
pub mod path_sanitizer;
pub mod secure_extract;
pub mod snappy_defense;
pub mod tar_defense;
pub mod xz_defense;
pub mod zip_defense;

#[cfg(test)]
mod tests;

pub use acl::*;
pub use brotli_defense::*;
pub use bzip2_defense::*;
pub use libdeflate_defense::*;
pub use license::*;
pub use lzfse_defense::*;
pub use path_sanitizer::*;
pub use secure_extract::*;
pub use snappy_defense::*;
pub use tar_defense::*;
pub use xz_defense::*;
pub use zip_defense::*;


