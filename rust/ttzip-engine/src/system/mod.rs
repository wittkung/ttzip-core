// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! System-level services, microkernel utilities, and binary delta patching subsystems.

pub mod delta;

pub use delta::{
    BsDiffControl, BsDiffPatch, DeltaCommand, DeltaError, DeltaFormat, DeltaPatchHeader,
    DeltaPatchResult, DeltaResult, TTZipBsDiff, TTZipBsPatch, TTZipDeltaArchive, TTZipDeltaEngine,
    DELTA_TREE_HASH_SEED,
};
