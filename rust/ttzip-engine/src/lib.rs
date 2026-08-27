// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TTZip Pure Safe Rust High-Performance Archiving & Microkernel Engine.
//!
//! Provides hardware-accelerated crypto/checksum routines, safe codec wrappers,
//! unified archive streaming, ZIP/7z archive engines, and VFS processing.

#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

uniffi::setup_scaffolding!();

pub mod analytics;
pub mod archive;
pub mod audio;
pub mod benchmark;
pub use benchmark as bench;
pub mod charset;
pub mod checksum;
pub mod codecs;
pub mod crypto;
pub mod ffi;
pub mod fs;
pub mod i18n;
pub mod media;
pub mod platform;
pub mod runtime;
pub mod security;
pub mod sevenz;
pub mod standards;
pub mod testing;
pub mod types;
pub mod uniffi_api;
pub mod utils;
pub mod vfs;
pub mod zip;

pub use analytics::*;
pub use checksum::{Adler32Hasher, Crc32Hasher};
pub use utils::*;
pub use archive::{
    compute_volume_path, detect_volume_chain, drill_down_nested_archive, find_next_pk_signature,
    open_virtual_file_stream, repair_damaged_tar, repair_damaged_zip, ArchiveBuilder,
    ArchiveEntryInfo, ArchiveReader, ExtractBuilder, SplitVolumeWriter, VirtualFileStream,
    VirtualMultiVolumeReader, VolumeNamingScheme, VolumeSegment,
};
pub use audio::*;
pub use benchmark::*;
pub use charset::*;
pub use codecs::*;
pub use crypto::*;
pub use fs::*;
pub use i18n::*;
pub use media::*;
pub use platform::*;
pub use runtime::*;
pub use security::*;
pub use sevenz::{
    create_7z_archive, decode_7z_solid_payload, parse_7z_metadata, SevenZArchive, SevenZFileMeta,
    SevenZHeaderInfo,
};
pub use standards::*;
pub use types::*;
pub use uniffi_api::{
    archive::*, audio::*, disk_scanner::*, extraction::*, integrity::*, media::*,
    ttzip_i18n_format_bytes, ttzip_i18n_format_throughput, ttzip_i18n_get_string,
    ttzip_i18n_localize_error, vault::*, vfs::*,
};
pub use vfs::*;
pub use zip::{create_zip_archive, ZipArchive, ZipEntry, ZipInputItem};

