// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

//! Runtime infrastructure: cancellation tokens, exception barriers, and structured logging router.

pub mod cancellation;
pub mod logging;
pub mod ring_buffer;

pub use cancellation::*;
pub use logging::*;
pub use ring_buffer::*;
