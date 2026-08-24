// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

//! ZIP Archive Parallel Compression and Writing Engine.
//!
//! Supports Store, Deflate (Levels 1..12 via `libdeflate`), WinZip AES-256 hardware encryption,
//! and automatic Zip64 extension promotion for large files (>4GB) and large catalogs (>65535 files).

pub mod assemble;
pub mod parallel;
pub mod store_stream;
pub mod streaming_parallel;
pub mod types;

pub use assemble::*;
pub use parallel::*;
pub use store_stream::*;
pub use streaming_parallel::*;
pub use types::*;

use crate::types::{TTZipCreateOptions, TTZipStatus};
use std::path::{Path, PathBuf};

/// Creates a ZIP archive file directly from input source paths.
pub fn create_zip_archive(
    dest_path: &Path,
    source_paths: &[PathBuf],
    options: &TTZipCreateOptions,
) -> Result<ZipCreateReport, TTZipStatus> {
    create_zip_streaming_parallel(dest_path, source_paths, options)
}
