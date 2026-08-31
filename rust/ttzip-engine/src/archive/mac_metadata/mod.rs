// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! macOS AppleDouble format parser, serializer, and extended attributes subsystem.

pub mod appledouble;
pub mod finder_info;
pub mod types;
pub mod xattr;

pub use appledouble::*;
pub use finder_info::*;
pub use types::*;
pub use xattr::*;
