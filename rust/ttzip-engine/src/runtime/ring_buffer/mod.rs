// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Lock-free high throughput ring buffers.

pub mod mpmc;
pub mod spsc;

#[cfg(test)]
mod tests;

pub use mpmc::MpmcRingBuffer;
pub use spsc::{SpscConsumer, SpscProducer, SpscRingBuffer};
