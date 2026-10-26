// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use crate::mach_vm::{MachVmAllocation, MachVmError, MachPortGuard};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A dual-buffer RCU publishing pool over Mach VM anonymous memory.
/// Allows background generation of new snapshots while UI readers retain the previous snapshot port.
pub struct SnapshotRingBuffer {
    buffers: [Option<MachVmAllocation>; 2],
    active_index: AtomicUsize,
    generation: u64,
}

impl Default for SnapshotRingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotRingBuffer {
    pub fn new() -> Self {
        Self {
            buffers: [None, None],
            active_index: AtomicUsize::new(0),
            generation: 0,
        }
    }

    /// Acquires the inactive buffer, reallocating if the requested size is larger than current capacity.
    /// Returns a mutable slice to write the new snapshot.
    pub fn acquire_inactive_buffer(&mut self, required_size: usize) -> Result<&mut [u8], MachVmError> {
        let inactive_idx = self.active_index.load(Ordering::Acquire) ^ 1;
        
        let needs_realloc = match &self.buffers[inactive_idx] {
            Some(alloc) => alloc.len() < required_size,
            None => true,
        };

        if needs_realloc {
            let aligned_size = (required_size + 16383) & !16383; // 16KB alignment
            self.buffers[inactive_idx] = Some(MachVmAllocation::allocate(aligned_size)?);
        }

        // We can safely unwrap because we just ensured it exists.
        Ok(self.buffers[inactive_idx].as_mut().unwrap().as_mut_slice())
    }

    /// Publishes the inactive buffer as the new active snapshot and returns a Mach port handle
    /// (memory entry) for the reader to consume (e.g., via XPC).
    pub fn publish_and_export_port(&mut self) -> Result<(u64, MachPortGuard), MachVmError> {
        let inactive_idx = self.active_index.load(Ordering::Acquire) ^ 1;
        
        // Export the port from the newly written inactive buffer (read-only)
        let port = if let Some(alloc) = &self.buffers[inactive_idx] {
            alloc.make_memory_entry(true)?
        } else {
            return Err(MachVmError::InvalidSize);
        };

        // Atomically flip the active index
        self.active_index.store(inactive_idx, Ordering::Release);
        self.generation += 1;

        Ok((self.generation, port))
    }
}
