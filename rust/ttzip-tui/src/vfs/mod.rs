// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Archive Virtual File System (VFS) Tree and Instant Fuzzy Search Matcher.
//!
//! Conforms to `specs/170-rust-interactive-tui-engine/contracts/tui_vfs_tree_contract.json`
//! and `data-model.md`.

pub mod meta;
pub mod node;
pub mod search;
pub mod tree;
pub mod view;

pub use meta::*;
pub use node::*;
pub use search::*;
pub use tree::*;
pub use view::*;

#[cfg(test)]
mod tests;
