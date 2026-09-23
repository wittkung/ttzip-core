// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! TTZip High-Throughput Archiving Containers, Virtual File System, and State Machines.

pub mod archive;
pub mod charset;
pub mod fs;
pub mod platform;
pub mod security;
pub mod sevenz;
pub mod standards;
pub mod sys;
pub mod tar;
pub mod vfs;
pub mod xz;
pub mod zip;

// Alias sys as ffi internally so crate::ffi::archive_ffi resolves
pub use sys as ffi;

// Re-export foundational micro-crates for internal crate ergonomics
pub use ttzip_codecs as codecs;
pub use ttzip_crypto as crypto;
pub use ttzip_types as types;

