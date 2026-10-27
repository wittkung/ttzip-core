// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Memory routing strategy based on archive entry payload scale.

/// Adaptive memory execution strategy determined by entry size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryStrategy {
    /// Entries < 1MB: direct inline invocation, zero thread or channel switching overhead.
    DirectCall,
    /// Entries 1MB..=64MB: chunked streaming with bounded backpressure buffering.
    Streaming {
        /// Chunk buffer capacity in bytes (typically 64KB..=256KB).
        chunk_size: usize,
    },
    /// Entries > 64MB: zero-copy memory mapped sliding window.
    ZeroCopy,
}

impl MemoryStrategy {
    /// 1 MB threshold separating direct call from streaming.
    pub const DIRECT_CALL_THRESHOLD: u64 = 1024 * 1024;
    /// 64 MB threshold separating streaming from memory mapping.
    pub const STREAMING_THRESHOLD: u64 = 64 * 1024 * 1024;
    /// Default streaming buffer chunk size (128 KB).
    pub const DEFAULT_CHUNK_SIZE: usize = 128 * 1024;

    /// Selects the optimal execution strategy based on compressed or uncompressed payload size.
    #[inline]
    #[must_use]
    pub const fn select(size_bytes: u64) -> Self {
        if size_bytes < Self::DIRECT_CALL_THRESHOLD {
            Self::DirectCall
        } else if size_bytes <= Self::STREAMING_THRESHOLD {
            Self::Streaming {
                chunk_size: Self::DEFAULT_CHUNK_SIZE,
            }
        } else {
            Self::ZeroCopy
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_strategy_routing_thresholds() {
        assert_eq!(MemoryStrategy::select(0), MemoryStrategy::DirectCall);
        assert_eq!(MemoryStrategy::select(1024), MemoryStrategy::DirectCall);
        assert_eq!(MemoryStrategy::select(1024 * 1024 - 1), MemoryStrategy::DirectCall);

        assert_eq!(
            MemoryStrategy::select(1024 * 1024),
            MemoryStrategy::Streaming {
                chunk_size: MemoryStrategy::DEFAULT_CHUNK_SIZE
            }
        );
        assert_eq!(
            MemoryStrategy::select(32 * 1024 * 1024),
            MemoryStrategy::Streaming {
                chunk_size: MemoryStrategy::DEFAULT_CHUNK_SIZE
            }
        );
        assert_eq!(
            MemoryStrategy::select(64 * 1024 * 1024),
            MemoryStrategy::Streaming {
                chunk_size: MemoryStrategy::DEFAULT_CHUNK_SIZE
            }
        );

        assert_eq!(
            MemoryStrategy::select(64 * 1024 * 1024 + 1),
            MemoryStrategy::ZeroCopy
        );
        assert_eq!(
            MemoryStrategy::select(1024 * 1024 * 1024),
            MemoryStrategy::ZeroCopy
        );
    }
}
