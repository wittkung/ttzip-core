// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Memory management, dual-ended bump workspace arenas, and zero-allocation scratchpads.

pub mod bump_workspace;

pub use bump_workspace::{BumpWorkspace, WorkspaceError, CACHE_LINE_ALIGNMENT};
