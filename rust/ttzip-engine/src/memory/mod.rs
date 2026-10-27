// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Memory management, dual-ended bump workspace arenas, and zero-allocation scratchpads.

pub mod bump_workspace;

pub use bump_workspace::{BumpWorkspace, WorkspaceError, CACHE_LINE_ALIGNMENT};
