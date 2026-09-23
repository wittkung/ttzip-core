// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Memory management, dual-ended bump workspace arenas, and zero-allocation scratchpads.

pub mod bump_workspace;

pub use bump_workspace::{BumpWorkspace, WorkspaceError, CACHE_LINE_ALIGNMENT};
