// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

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
