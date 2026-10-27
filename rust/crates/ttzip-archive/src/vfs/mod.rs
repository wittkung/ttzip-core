// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Universal Virtual Filesystem (VFS) abstraction, CoW cloning, and sparse file drivers.

pub mod cache_pool;
pub mod cow;
pub mod sparse;
pub mod traits;

pub use cache_pool::*;
pub use cow::*;
pub use sparse::*;
pub use traits::*;
pub use crate::fs::vfs::*;
