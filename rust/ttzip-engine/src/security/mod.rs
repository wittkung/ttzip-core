// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Security, sandbox isolation, path defense, and threat scanning modules.

pub mod license;
pub mod path_sanitizer;

#[cfg(test)]
mod tests;

pub use license::*;
pub use path_sanitizer::*;
