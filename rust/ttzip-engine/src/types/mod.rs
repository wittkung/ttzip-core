// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Core types, error codes, options, and shared data structures for TTZip.

pub mod dto;
pub mod formats;
pub mod options;
pub mod provenance;
pub mod status;

pub use dto::*;
pub use formats::*;
pub use options::*;
pub use provenance::*;
pub use status::*;
