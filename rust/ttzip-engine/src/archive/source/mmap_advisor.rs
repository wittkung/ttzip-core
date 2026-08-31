// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Lifecycle-aware memory mapping advisor and dynamic POSIX madvise scheduler.

use super::mmap::{get_system_page_size, MmapSource};
use crate::types::TTZipStatus;

/// Kernel virtual memory paging hints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmapAdvice {
    /// Default kernel page caching behavior.
    Normal,
    /// Expect page references in sequential order (aggressive read-ahead).
    Sequential,
    /// Expect page references in random order (disable aggressive read-ahead).
    Random,
    /// Expect access in the near future (pre-populate page table entries).
    WillNeed,
    /// Do not expect access in the near future (free page cache pages).
    DontNeed,
    /// Memory can be discarded immediately if memory pressure occurs.
    Free,
}

impl MmapAdvice {
    /// Converts high-level advice into the platform-specific `libc::c_int` flag.
    #[inline]
    #[must_use]
    pub const fn to_libc_advice(self) -> libc::c_int {
        match self {
            Self::Normal => libc::MADV_NORMAL,
            Self::Sequential => libc::MADV_SEQUENTIAL,
            Self::Random => libc::MADV_RANDOM,
            Self::WillNeed => libc::MADV_WILLNEED,
            Self::DontNeed => libc::MADV_DONTNEED,
            #[cfg(target_os = "macos")]
            Self::Free => libc::MADV_FREE,
            #[cfg(not(target_os = "macos"))]
            Self::Free => libc::MADV_DONTNEED,
        }
    }
}

/// Archive execution lifecycle phases used for proactive I/O paging optimization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveLifecyclePhase {
    /// Initial magic byte, central directory, and table-of-contents discovery.
    HeaderProbing,
    /// Continuous sequential streaming decompression from start to finish.
    SequentialDecompression,
    /// Non-contiguous entry seeks, selective extraction, or random access.
    RandomSeeking,
    /// Solid block decoding where a large contiguous range is actively decoded.
    SolidBlockExtraction,
    /// Archive source is idle or completed, allowing page cache eviction.
    Idle,
}

/// High-performance memory-mapping advisor and kernel paging scheduler.
pub struct MmapAdvisor;

impl MmapAdvisor {
    /// Resolves recommended `MmapAdvice` for a given archive execution lifecycle phase.
    #[inline]
    #[must_use]
    pub const fn recommended_advice(phase: ArchiveLifecyclePhase) -> MmapAdvice {
        match phase {
            ArchiveLifecyclePhase::HeaderProbing => MmapAdvice::WillNeed,
            ArchiveLifecyclePhase::SequentialDecompression => MmapAdvice::Sequential,
            ArchiveLifecyclePhase::RandomSeeking => MmapAdvice::Random,
            ArchiveLifecyclePhase::SolidBlockExtraction => MmapAdvice::WillNeed,
            ArchiveLifecyclePhase::Idle => MmapAdvice::Free,
        }
    }

    /// Applies kernel paging advice across a raw memory buffer with page alignment safety.
    pub fn apply(ptr: *const u8, len: usize, advice: MmapAdvice) -> Result<(), TTZipStatus> {
        if ptr.is_null() {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        if len == 0 {
            return Ok(());
        }

        let page_size = get_system_page_size();
        let raw_addr = ptr as usize;
        let aligned_addr = (raw_addr / page_size) * page_size;
        let page_diff = raw_addr - aligned_addr;

        let aligned_len = page_diff
            .checked_add(len)
            .ok_or(TTZipStatus::ErrInvalidParam)?;

        let res = unsafe {
            libc::madvise(
                aligned_addr as *mut libc::c_void,
                aligned_len,
                advice.to_libc_advice(),
            )
        };

        if res != 0 {
            Err(TTZipStatus::ErrInvalidParam)
        } else {
            Ok(())
        }
    }

    /// Applies kernel paging advice to a sub-range within a raw memory buffer.
    pub fn apply_range(
        ptr: *const u8,
        total_len: usize,
        offset: usize,
        range_len: usize,
        advice: MmapAdvice,
    ) -> Result<(), TTZipStatus> {
        if ptr.is_null() {
            return Err(TTZipStatus::ErrInvalidParam);
        }
        if offset.checked_add(range_len).is_none_or(|end| end > total_len) {
            return Err(TTZipStatus::ErrInvalidOffset);
        }
        if range_len == 0 {
            return Ok(());
        }

        let target_ptr = unsafe { ptr.add(offset) };
        Self::apply(target_ptr, range_len, advice)
    }

    /// Schedules advice on an `MmapSource` according to lifecycle phase.
    #[inline]
    pub fn schedule_phase(
        source: &MmapSource,
        phase: ArchiveLifecyclePhase,
    ) -> Result<(), TTZipStatus> {
        let advice = Self::recommended_advice(phase);
        source.advise(advice)
    }

    /// Schedules advice on a sub-range of an `MmapSource` according to lifecycle phase.
    #[inline]
    pub fn schedule_range_phase(
        source: &MmapSource,
        offset: u64,
        len: u64,
        phase: ArchiveLifecyclePhase,
    ) -> Result<(), TTZipStatus> {
        let advice = Self::recommended_advice(phase);
        source.advise_range(offset, len, advice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::source::StorageMedium;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_recommended_advice_mappings() {
        assert_eq!(
            MmapAdvisor::recommended_advice(ArchiveLifecyclePhase::HeaderProbing),
            MmapAdvice::WillNeed
        );
        assert_eq!(
            MmapAdvisor::recommended_advice(ArchiveLifecyclePhase::SequentialDecompression),
            MmapAdvice::Sequential
        );
        assert_eq!(
            MmapAdvisor::recommended_advice(ArchiveLifecyclePhase::RandomSeeking),
            MmapAdvice::Random
        );
        assert_eq!(
            MmapAdvisor::recommended_advice(ArchiveLifecyclePhase::SolidBlockExtraction),
            MmapAdvice::WillNeed
        );
        assert_eq!(
            MmapAdvisor::recommended_advice(ArchiveLifecyclePhase::Idle),
            MmapAdvice::Free
        );
    }

    #[test]
    fn test_null_pointer_rejection() {
        assert_eq!(
            MmapAdvisor::apply(std::ptr::null(), 1024, MmapAdvice::Sequential).err(),
            Some(TTZipStatus::ErrInvalidParam)
        );
        assert_eq!(
            MmapAdvisor::apply_range(std::ptr::null(), 2048, 0, 1024, MmapAdvice::Sequential).err(),
            Some(TTZipStatus::ErrInvalidParam)
        );
    }

    #[test]
    fn test_apply_range_out_of_bounds() {
        let dummy = [0u8; 64];
        assert_eq!(
            MmapAdvisor::apply_range(dummy.as_ptr(), 64, 50, 20, MmapAdvice::Sequential).err(),
            Some(TTZipStatus::ErrInvalidOffset)
        );
    }

    #[test]
    fn test_zero_length_handling() {
        let dummy = [0u8; 64];
        assert_eq!(MmapAdvisor::apply(dummy.as_ptr(), 0, MmapAdvice::Sequential), Ok(()));
        assert_eq!(
            MmapAdvisor::apply_range(dummy.as_ptr(), 64, 10, 0, MmapAdvice::Sequential),
            Ok(())
        );
    }

    #[test]
    fn test_lifecycle_phase_scheduling() {
        let mut temp = NamedTempFile::new().unwrap();
        let payload = vec![0x5A; 32768];
        temp.write_all(&payload).unwrap();
        temp.flush().unwrap();

        let source = MmapSource::open(temp.path(), StorageMedium::LocalFastApfs).unwrap();
        assert_eq!(
            MmapAdvisor::schedule_phase(&source, ArchiveLifecyclePhase::HeaderProbing),
            Ok(())
        );
        assert_eq!(
            MmapAdvisor::schedule_phase(&source, ArchiveLifecyclePhase::SequentialDecompression),
            Ok(())
        );
        assert_eq!(
            MmapAdvisor::schedule_phase(&source, ArchiveLifecyclePhase::RandomSeeking),
            Ok(())
        );
        assert_eq!(
            MmapAdvisor::schedule_phase(&source, ArchiveLifecyclePhase::SolidBlockExtraction),
            Ok(())
        );
        assert_eq!(
            MmapAdvisor::schedule_phase(&source, ArchiveLifecyclePhase::Idle),
            Ok(())
        );
        assert_eq!(
            MmapAdvisor::schedule_range_phase(
                &source,
                0,
                4096,
                ArchiveLifecyclePhase::HeaderProbing
            ),
            Ok(())
        );
    }
}
