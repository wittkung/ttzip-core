// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Dual-path streaming pipeline and multi-layer filter chain architecture.
//!
//! Provides zero-copy slice processing for small files (<1MB), bounded chunked
//! streaming with backpressure for medium files (1MB..=64MB), and memory-mapped
//! zero-copy execution for large files (>64MB).

pub mod filter;
pub mod filter_chain;
pub mod strategy;

pub use filter::{ByteMapFilter, IdentityFilter, StreamFilter};
pub use filter_chain::{FilterChain, InlineFilterChain, INLINE_FILTER_CAPACITY};
pub use strategy::MemoryStrategy;
