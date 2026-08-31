// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Virtual Filesystem (VFS) decompression caching and buffer management.

pub mod cache_pool;

pub use cache_pool::*;
pub use crate::codecs::zstd_seekable::*;
