// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Core types, error codes, options, and shared data structures for TTZip.

pub mod archive_format;
pub mod detected_format;
pub mod dto;
pub mod errors;
pub mod formats;
pub mod options;
pub mod preset;
pub mod provenance;
pub mod status;

pub use archive_format::*;
pub use detected_format::*;
pub use dto::*;
pub use errors::*;
pub use formats::*;
pub use options::*;
pub use preset::*;
pub use provenance::*;
pub use status::*;
