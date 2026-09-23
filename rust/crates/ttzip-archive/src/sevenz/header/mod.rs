// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! 7-Zip Header and EncodedHeader Zero-Copy Metadata Parser and Seek Indexing.

pub mod metadata;
pub mod models;
pub mod seek;
pub mod stream;

pub use metadata::parse_7z_metadata;
pub use models::{SevenZCoder, SevenZFileMeta, SevenZFolder, SevenZHeaderInfo};
pub use seek::{SevenZEntryLocation, SevenZSeekIndex};
pub use stream::parse_7z_header_stream;
