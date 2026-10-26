// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! File system utilities, safe extraction pipeline, and APFS optimizations.

pub mod abort_guard;
pub mod apfs;
pub mod deferred_fixup;
pub mod dir_jail;
pub mod filter;
pub mod natural_sort;
pub mod safe_extract;
pub mod scanner;
pub mod sparse;
pub mod staged_metadata;
pub mod transactional_writer;
pub mod vfs;

pub use abort_guard::*;
pub use apfs::*;
pub use deferred_fixup::*;
pub use dir_jail::*;
pub use filter::*;
pub use natural_sort::*;
pub use safe_extract::*;
pub use scanner::*;
pub use sparse::*;
pub use staged_metadata::*;
pub use transactional_writer::*;
pub use vfs::*;

pub mod collision;
#[cfg(test)]
mod tests_safe_extract;


