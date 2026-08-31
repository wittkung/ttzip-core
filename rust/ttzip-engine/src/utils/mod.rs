// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-branch bitstream accumulators, SWAR match extension, and guarded memory test utilities.

pub mod bitstream;
pub mod guarded_memory;
pub mod hardware_portability;
pub mod lz_extend;

pub use bitstream::*;
pub use guarded_memory::*;
pub use hardware_portability::*;
pub use lz_extend::*;
