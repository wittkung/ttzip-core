// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Pure Safe Rust incremental delta patching and bsdiff/bspatch microkernel.

pub mod archive;
pub mod bsdiff;
pub mod bspatch;
pub mod engine;
pub mod types;

#[cfg(test)]
mod tests;

pub use archive::*;
pub use bsdiff::*;
pub use bspatch::*;
pub use engine::*;
pub use types::*;
