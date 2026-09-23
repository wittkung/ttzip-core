// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

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
pub use ttzip_archive::charset;
pub mod checksum;
pub use ttzip_codecs as codecs;
pub use ttzip_crypto as crypto;
pub mod ebook;
pub mod ffi;
pub use ttzip_archive::fs;
pub mod i18n;
pub mod memory;
pub mod office;
pub mod platform;
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

pub use analytics::*;
pub use api::*;
pub use audio::*;
pub use checksum::{Adler32Hasher, Crc32Hasher};
pub use utils::*;
pub use archive::{
    compute_volume_path, detect_volume_chain, drill_down_nested_archive, find_next_pk_signature,
    open_virtual_file_stream, repair_damaged_tar, repair_damaged_zip, ArchiveBuilder,
    ArchiveEntryInfo, ArchiveReader, ExtractBuilder, SplitVolumeWriter, VirtualFileStream,
    VirtualMultiVolumeReader, VolumeNamingScheme, VolumeSegment,
};
pub use benchmark::*;
pub use charset::*;
pub use codecs::*;
pub use crypto::*;
pub use ebook::{
    decompress_palmdoc_record, EbookError, EbookFormat, EbookMetadata, EbookMobiDecoder,
    EbookNavigationExtractor, EbookResource, EbookResourceExtractor, EbookResult, EbookTocNode,
    MobiExthRecord, MobiHeaderInfo, PalmDocHeader, SpineItem, TTZipEbookParser,
};
pub use i18n::*;
pub use memory::*;
pub use office::*;
pub use platform::*;
pub use runtime::*;
pub use fs::*;
pub use security::*;
pub use sevenz::{
    create_7z_archive, decode_7z_solid_payload, parse_7z_metadata, SevenZArchive, SevenZFileMeta,
    SevenZHeaderInfo,
};
pub use standards::*;
pub use system::{
    BsDiffControl, BsDiffPatch, DeltaCommand, DeltaError, DeltaFormat, DeltaPatchHeader,
    DeltaPatchResult, DeltaResult, TTZipBsDiff, TTZipBsPatch, TTZipDeltaArchive, TTZipDeltaEngine,
    DELTA_TREE_HASH_SEED,
};
pub use ttzip_types::*;
pub use uniffi_api::{
    archive::*, audio::*, disk_scanner::*, extraction::*, integrity::*,
    ttzip_i18n_format_bytes, ttzip_i18n_format_throughput, ttzip_i18n_get_string,
    ttzip_i18n_localize_error, vault::*, vfs::*, zopfli_opt::*,
};
pub use tar::{
    alignment::*, checksum::*, codec::*, gnu::*, header::*, pax::*, sparse::*,
    xattr::*,
};
pub use xz::{
    apply_filters_decode, arm_decode, arm_thumb_decode, decompress_block_payload,
    decompress_lzma2_payload, delta_decode, ia64_decode, lzma2_dict_size_from_prop, powerpc_decode,
    sparc_decode, xz_compress, xz_decompress, XzBcjType, XzBlockEncoder, XzBlockError,
    XzBlockHeader, XzBlockRecord, XzCheckType, XzChecksumEngine, XzChecksumError, XzChecksumType,
    XzCrc64, XzDecodeError, XzDecoderState, XzEncoderOptions, XzError, XzFilterConfig,
    XzParallelStreamWriter, XzRecord, XzSeekableReader, XzStreamDecoder, XzStreamFlags,
    XzStreamFooter, XzStreamHeader, XzStreamIndex, XzStreamWriter, XzVliError,
};

pub use zip::{
    create_zip_archive, CentralDirectoryEndInfo, EocdScanner, ZipArchive, ZipEngineError, ZipEntry,
    ZipInputItem,
};

