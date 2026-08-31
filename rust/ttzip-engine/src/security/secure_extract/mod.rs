// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Secure Path Extractor and Sandboxed Path Traversal Defense Subsystem.
//!
//! Inspired by libarchive `archive_write_disk_posix.c`:
//! - Fine-grained `SecurityFlags` controlling path traversal, symlink resolution, and permission restoration.
//! - Strict Zip-Slip and dot-dot traversal rejection across POSIX and Windows path representations.
//! - Intermediate path validation preventing symlink escape to outside the sandbox.
//! - TOCTOU (Time-of-Check to Time-of-Use) immunity via POSIX file descriptor pinning (`openat`, `fstat`, `fchmod`, `unlinkat`).
//! - Two-stage bottom-up metadata and permission application preventing privilege escalation and lockout.

pub mod deferred;
pub mod extractor;
pub mod flags;

pub use deferred::*;
pub use extractor::*;
pub use flags::*;
