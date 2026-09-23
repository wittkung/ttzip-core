// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Virtual Filesystem (VFS) decompression caching and buffer management.

pub mod cache_pool;

pub use cache_pool::*;
pub use crate::codecs::zstd_seekable::*;
