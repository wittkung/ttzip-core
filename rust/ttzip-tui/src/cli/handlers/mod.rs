// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Subcommand execution handlers for all headless CLI commands.

pub mod bench;
pub mod cat;
pub mod check;
pub mod comment;
pub mod convert;
pub mod create;
pub mod delete;
pub mod diff;
pub mod doctor;
pub mod extract;
pub mod hash;
pub mod info;
pub mod list;
pub mod lock;
pub mod recover;
pub mod repair;
pub mod split;
pub mod tree;
pub mod update;
pub mod completions;
pub mod mcp;

pub use bench::*;
pub use cat::*;
pub use check::*;
pub use comment::*;
pub use completions::*;
pub use convert::*;
pub use create::*;
pub use delete::*;
pub use diff::*;
pub use doctor::*;
pub use extract::*;
pub use hash::*;
pub use info::*;
pub use list::*;
pub use lock::*;
pub use mcp::*;
pub use recover::*;
pub use repair::*;
pub use split::*;
pub use tree::*;
pub use update::*;
