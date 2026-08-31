// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! PPMd algorithmic variants and memory exhaustion restore methods.

/// PPMd model variant specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpmdVariant {
    /// 7-Zip PPMd7 (Model H) with RestartModel strategy on memory exhaustion.
    Ppmd7,
    /// WinZip / PKWARE PPMd8 (Model I) with CutOff 75% memory pruning strategy.
    Ppmd8,
}

/// Strategy executed when the Sub-Allocator arena is exhausted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpmdRestoreMethod {
    /// Completely clear the arena and restart from order-0 root.
    Restart,
    /// Prune low-frequency and high-order nodes, freeing at least 25% of units.
    CutOff,
}
