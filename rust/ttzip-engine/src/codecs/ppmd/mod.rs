// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! PPMd (Prediction by Partial Matching, variant D) prediction engine module.
//!
//! Provides the 12-byte `PpmdContext`, 6-byte `PpmdState`, Secondary Escape Estimation (SEE),
//! and the 12-byte Unit Sub-Allocator Arena (`SubAllocBumpArena`) strictly conforming to
//! 7-Zip PPMd7 (Model H) and PPMd8 (Model I) standards.

mod alone;
pub mod models;
pub mod see;
pub mod suballoc;
pub mod suballoc_model;
pub mod variant;

pub use alone::{
    ppmd_compress, ppmd_compress_to_vec, ppmd_decompress, ppmd_decompress_7z,
    ppmd_decompress_7z_to_vec, ppmd_decompress_to_vec, ppmd_parse_7z_props,
    PpmdModel, PpmdRangeDecoder, PpmdRangeEncoder, PpmdSubAlloc,
    METHOD_PPMD, PPMD_DEFAULT_MEMORY_SIZE, PPMD_DEFAULT_ORDER, PPMD_MAX_MEMORY_SIZE,
    PPMD_MAX_ORDER, PPMD_MIN_MEMORY_SIZE, PPMD_MIN_ORDER,
};
pub use models::{
    PpmdContext, PpmdContext as PpmdUnitContext, PpmdState, PpmdState as PpmdUnitState,
    INIT_BIN_ESC, PPMD_BIN_SCALE, PPMD_DEFAULT_SUBALLOC_SIZE, PPMD_EXP_ESCAPE,
    PPMD_INT_BITS, PPMD_MAX_FREQ, PPMD_MAX_SUBALLOC_SIZE, PPMD_MIN_SUBALLOC_SIZE,
    PPMD_NUM_INDEXES, PPMD_PERIOD_BITS, PPMD_UNIT_SIZE, SEE_NUM_BINS, SEE_NUM_CLASSES,
};
pub use see::{SeeEntry, SeeEstimator};
pub use suballoc::SubAllocBumpArena;
pub use suballoc_model::PpmdSubAllocModel;
pub use variant::{PpmdRestoreMethod, PpmdVariant};
