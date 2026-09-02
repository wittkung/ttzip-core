// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! System-level services, microkernel utilities, and binary delta patching subsystems.

pub mod delta;

pub use delta::{
    BsDiffControl, BsDiffPatch, DeltaCommand, DeltaError, DeltaFormat, DeltaPatchHeader,
    DeltaPatchResult, DeltaResult, TTZipBsDiff, TTZipBsPatch, TTZipDeltaArchive, TTZipDeltaEngine,
    DELTA_TREE_HASH_SEED,
};
