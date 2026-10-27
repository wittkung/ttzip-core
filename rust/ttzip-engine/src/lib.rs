// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! TTZip Pure Safe Rust High-Performance Archiving & Microkernel Engine.
//!
//! Provides hardware-accelerated crypto/checksum routines, safe codec wrappers,
//! unified archive streaming, ZIP/7z archive engines, and VFS processing.

#![allow(non_camel_case_types)]

uniffi::setup_scaffolding!();

pub mod analytics;
pub mod api;
pub use ttzip_archive::archive;
pub mod audio;
pub mod benchmark;
pub use benchmark as bench;
pub mod bench_telemetry;
pub use ttzip_archive::charset;
pub mod checksum;
pub use ttzip_codecs as codecs;
pub use ttzip_crypto as crypto;
pub mod ebook;
pub mod ffi;
pub use ttzip_archive::fs;
pub mod i18n;
pub mod memory;
pub mod platform;
pub mod recovery;
pub mod runtime;
pub mod security;
pub use ttzip_archive::sevenz;
pub mod standards;
pub mod system;
pub use ttzip_archive::tar;
pub mod testing;
pub use ttzip_types as types;
pub mod uniffi_api;
pub mod utils;
pub use ttzip_archive::vfs;
pub use ttzip_archive::xz;
pub use ttzip_archive::zip;
pub use ttzip_archive::to_and_from_le;

pub use api::*;
pub use archive::{
    compute_volume_path, detect_volume_chain, open_virtual_file_stream, ArchiveBuilder,
    ArchiveEntry, ArchiveEntryInfo, ArchiveReader, ExtractBuilder, SplitVolumeWriter,
};
pub use sevenz::{
    create_7z_archive, decode_7z_solid_payload, parse_7z_metadata, SevenZArchive, SevenZFileMeta,
    SevenZHeaderInfo,
};
pub use ttzip_types::*;
pub use uniffi_api::{
    archive::*, audio::*, disk_scanner::*, extraction::*, integrity::*,
    sanitize_archive_path,
    ttzip_i18n_format_bytes, ttzip_i18n_format_throughput, ttzip_i18n_get_string,
    ttzip_i18n_localize_error, vault::*, vfs::*,
};
pub use zip::{
    create_zip_archive, CentralDirectoryEndInfo, EocdScanner, ZipArchive, ZipEngineError, ZipEntry,
    ZipInputItem,
};

// Compile-time static concurrency contract assertions ensuring Tier-1 thread safety & L1 cache line compactness.
const fn assert_send_sync_static<T: Send + Sync + 'static>() {}
const _: () = {
    assert_send_sync_static::<ArchiveFormat>();
    assert_send_sync_static::<ArchiveEntryInfo>();
    assert_send_sync_static::<ArchiveEntry>();
    assert_send_sync_static::<ArchiveReader>();
    assert_send_sync_static::<security::SystemDefenseError>();
    assert_send_sync_static::<uniffi_api::UniFFIError>();
    assert_send_sync_static::<uniffi_api::TTZipError>();
    assert!(std::mem::size_of::<security::SystemDefenseError>() <= 64);
    assert!(std::mem::size_of::<uniffi_api::UniFFIError>() <= 64);
};
