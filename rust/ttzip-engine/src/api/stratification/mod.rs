// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 4-Tier Progressive API Staircase facade model.
//!
//! Provides stratified API levels for varying consumer requirements:
//! - Layer 1 (Simple API): Stateless one-shot compression and decompression functions.
//! - Layer 2 (Context API): Explicit reusable context handles with zero-allocation resets.
//! - Layer 3 (Streaming API): Structured stream cursors, chunk processing, and I/O pipeline adapters.
//! - Layer 4 (Advanced API): Fine-grained hyperparameter control, custom dictionaries, and SIMD flags.

pub mod advanced;
pub mod context;
pub mod simple;
pub mod streaming;

pub use advanced::*;
pub use context::*;
pub use simple::*;
pub use streaming::*;
