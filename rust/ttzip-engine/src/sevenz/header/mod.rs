// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 7-Zip Header and EncodedHeader Zero-Copy Metadata Parser and Seek Indexing.

pub mod metadata;
pub mod models;
pub mod seek;
pub mod stream;

pub use metadata::parse_7z_metadata;
pub use models::{SevenZCoder, SevenZFileMeta, SevenZFolder, SevenZHeaderInfo};
pub use seek::{SevenZEntryLocation, SevenZSeekIndex};
pub use stream::parse_7z_header_stream;
