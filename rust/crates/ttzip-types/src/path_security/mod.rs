// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Universal cross-platform system, path, and security primitives for the MetastudyLine monorepo.
//!
//! # Architecture Invariants
//! - **Zero External Dependencies**: Implemented strictly against the Rust standard library (`core`/`std`).
//! - **Zero Heap Allocation**: Critical validators evaluate as pure `const fn` over byte slices with no `String` or `Vec`.
//! - **Deterministic Defense Matrix**: Immunizes against directory traversal (CWE-22), Win32 space/dot normalization (CWE-41),
//!   NTFS 8.3 short-name aliasing, Apple HFS+ Unicode ignorable code points (CVE-2014-9390), and Trojan Source visual spoofing (CVE-2021-42574).

pub mod device;
pub mod error;
pub mod path;

// Re-export primary API surface for ergonomic consumer access
pub use device::{
    is_ntfs_reserved_metadata, is_windows_reserved_device_name,
    is_windows_reserved_device_name_bytes,
};
pub use error::PathSecurityError;
pub use path::{
    is_forbidden_path_char, is_safe_path, is_safe_relative_path, validate_safe_path,
    validate_safe_relative_path,
};
