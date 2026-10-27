// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Zero-branch bitstream accumulators, SWAR match extension, and guarded memory test utilities.

pub mod bitstream;
pub mod guarded_memory;
pub mod hardware_portability;
pub mod lz_extend;

pub use bitstream::*;
pub use guarded_memory::*;
pub use hardware_portability::*;
pub use lz_extend::*;
