// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! TTZip High-Throughput Archiving Containers, Virtual File System, and State Machines.

pub mod archive;
pub mod charset;
pub mod fs;
pub mod pipeline;
pub mod platform;
pub mod recovery;
pub mod security;
pub mod sevenz;
pub mod standards;
pub mod sys;
pub mod tar;
pub mod vfs;
pub mod xz;
pub mod zip;
pub mod rayon_pool;
pub mod mmap_guard;

// Alias sys as ffi internally so crate::ffi::archive_ffi resolves
pub use sys as ffi;

// Re-export foundational micro-crates for internal crate ergonomics
pub use ttzip_codecs as codecs;
pub use ttzip_crypto as crypto;
pub use ttzip_types as types;
pub use pipeline::{FilterChain, InlineFilterChain, MemoryStrategy, StreamFilter};
pub use rayon_pool::*;

// Compile-time static concurrency contract assertions ensuring Tier-1 thread safety & L1 cache line compactness.
const fn assert_send_sync_static<T: Send + Sync + 'static>() {}
const _: () = {
    assert_send_sync_static::<ttzip_types::ArchiveFormat>();
    assert_send_sync_static::<archive::ArchiveEntryInfo>();
    assert_send_sync_static::<archive::ArchiveEntry>();
    assert_send_sync_static::<archive::ArchiveReader>();
    assert_send_sync_static::<xz::XzDecodeError>();
    assert_send_sync_static::<xz::XzError>();
    assert_send_sync_static::<xz::XzBlockError>();
    assert_send_sync_static::<xz::XzVliError>();
    assert_send_sync_static::<xz::XzChecksumError>();
    assert_send_sync_static::<zip::ZipEngineError>();
    assert_send_sync_static::<tar::gnu::TarGnuError>();
    assert_send_sync_static::<tar::sparse::TarSparseError>();
    assert!(std::mem::size_of::<xz::XzDecodeError>() <= 64);
    assert!(std::mem::size_of::<xz::XzError>() <= 64);
    assert!(std::mem::size_of::<xz::XzBlockError>() <= 64);
    assert!(std::mem::size_of::<xz::XzVliError>() <= 64);
    assert!(std::mem::size_of::<xz::XzChecksumError>() <= 64);
    assert!(std::mem::size_of::<zip::ZipEngineError>() <= 64);
    assert!(std::mem::size_of::<tar::gnu::TarGnuError>() <= 64);
    assert!(std::mem::size_of::<tar::sparse::TarSparseError>() <= 64);
};

