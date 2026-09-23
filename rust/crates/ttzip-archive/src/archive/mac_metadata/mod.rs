// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! macOS AppleDouble format parser, serializer, and extended attributes subsystem.

pub mod appledouble;
pub mod finder_info;
pub mod types;
pub mod xattr;

pub use appledouble::*;
pub use finder_info::*;
pub use types::*;
pub use xattr::*;
