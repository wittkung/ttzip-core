// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Core types, error codes, options, and shared data structures for TTZip.
//!
//! # Memory Safety & FFI Interoperability Invariants
//! This crate defines cross-boundary C-ABI structures and DTOs consumed by Swift and C runtimes.
//! All raw pointer dereferencing is strictly isolated to explicit FFI bridge methods with verified
//! null-pointer checks, memory bounds enforcement, and zero-copy view guards.

pub mod archive_format;
pub mod detected_format;
pub mod dto;
pub mod error_chain;
pub mod formats;
pub mod options;
pub mod preset;
pub mod provenance;
pub mod status;
pub mod arena;
pub mod builder;
pub mod mach_vm;
pub mod node;
pub mod os_log_bridge;
pub mod ring_buffer;
pub mod signpost_bridge;
pub mod snapshot;
pub mod unified_node;
pub mod vfs_handle;
pub mod cpu_topology;
pub mod memory_budget;
pub mod scheduler;
pub mod search;
pub mod sort;
pub mod catalog;
pub mod simd_dot;
pub mod semantic_search;
pub mod path_security;

pub use archive_format::*;
pub use error_chain::ErrorChain;
pub use memory_budget::*;
pub use cpu_topology::*;
pub use dto::*;
pub use formats::{TTZipCompressionLevel, TTZipEncryptionMethod};
pub use mach_vm::*;
pub use node::*;
pub use options::*;
pub use os_log_bridge::*;
pub use preset::*;
pub use provenance::*;
pub use signpost_bridge::*;
pub use snapshot::*;
pub use status::*;
pub use unified_node::*;
pub use vfs_handle::*;
pub use search::*;
pub use sort::*;
pub use catalog::*;
pub use simd_dot::*;
pub use semantic_search::*;

// Compile-time static concurrency contract assertions ensuring Tier-1 thread safety & L1 cache line compactness.
const fn assert_send_sync_static<T: Send + Sync + 'static>() {}
const _: () = {
    assert_send_sync_static::<ArchiveCatalog>();
    assert_send_sync_static::<ArchiveFormat>();
    assert_send_sync_static::<TTZipEntryDetail>();
    assert_send_sync_static::<TTZipStatus>();
    assert_send_sync_static::<UnifiedEntryNode>();
    assert_send_sync_static::<TTWorkspaceEntryNode>();
    assert_send_sync_static::<TTWorkspaceSectionEntry>();
    assert_send_sync_static::<TTWorkspaceSectionType>();
    assert_send_sync_static::<TTWorkspaceSnapshotHeader>();
    assert_send_sync_static::<TTLogTraceContext>();
    assert_send_sync_static::<MachVmAllocation>();
    assert_send_sync_static::<OsLog>();
    assert_send_sync_static::<StringArena>();
    assert_send_sync_static::<UnifiedVfsTable>();
    assert_send_sync_static::<VfsTableHandle>();
    assert!(std::mem::size_of::<TTZipStatus>() <= 64);
    assert!(std::mem::size_of::<UnifiedEntryNode>() == 48);
    assert!(std::mem::align_of::<UnifiedEntryNode>() == 8);
    assert!(std::mem::size_of::<TTWorkspaceEntryNode>() == 64);
    assert!(std::mem::align_of::<TTWorkspaceEntryNode>() == 64);
    assert!(std::mem::size_of::<TTWorkspaceSectionEntry>() == 32);
    assert!(std::mem::align_of::<TTWorkspaceSectionEntry>() == 8);
    assert!(std::mem::size_of::<TTWorkspaceSectionType>() == 4);
    assert!(std::mem::align_of::<TTWorkspaceSectionType>() == 4);
    assert!(std::mem::size_of::<TTWorkspaceSnapshotHeader>() == 128);
    assert!(std::mem::align_of::<TTWorkspaceSnapshotHeader>() == 64);
    assert!(std::mem::size_of::<TTLogTraceContext>() == 24);
    assert!(std::mem::align_of::<TTLogTraceContext>() == 8);
};

