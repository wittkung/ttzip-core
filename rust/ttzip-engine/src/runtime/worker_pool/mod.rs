// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

//! Event-driven worker pools with kernel synchronization.

mod pool;

#[cfg(test)]
mod tests;

pub use pool::{EventDrivenWorkerPool, WorkerJob, WorkerPoolState};
