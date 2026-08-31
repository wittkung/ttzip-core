// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Bounded 64MB Chunk Ring Buffer and Virtual File Streaming Engine.

use std::collections::VecDeque;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;
use parking_lot::Mutex;

use crate::uniffi_api::types::TTZipError;

pub const MAX_STREAM_MEMORY: usize = 64 * 1024 * 1024; // 64 MB bounded RAM limit

pub fn calculate_chunk_size(total_size: u64) -> usize {
    if total_size <= 1024 * 1024 {
        64 * 1024
    } else if total_size <= 64 * 1024 * 1024 {
        256 * 1024
    } else if total_size <= 1024 * 1024 * 1024 {
        1024 * 1024
    } else {
        2 * 1024 * 1024
    }
}

struct ChunkSlot {
    index: u64,
    data: Arc<Vec<u8>>,
}

pub type ChunkLoader = Arc<dyn Fn(u64, usize) -> Result<Vec<u8>, TTZipError> + Send + Sync>;

pub struct VirtualChunkedStream {
    pub total_size: u64,
    pub chunk_size: usize,
    max_chunks: usize,
    ring_buffer: Mutex<VecDeque<ChunkSlot>>,
    loader: ChunkLoader,
}

impl VirtualChunkedStream {
    pub fn new(total_size: u64, chunk_size: usize, loader: ChunkLoader) -> Self {
        let max_chunks = (MAX_STREAM_MEMORY / chunk_size.max(1)).max(4);
        Self {
            total_size,
            chunk_size,
            max_chunks,
            ring_buffer: Mutex::new(VecDeque::with_capacity(max_chunks)),
            loader,
        }
    }

    pub fn get_chunk(&self, chunk_idx: u64) -> Result<Arc<Vec<u8>>, TTZipError> {
        let mut ring = self.ring_buffer.lock();
        if let Some(pos) = ring.iter().position(|s| s.index == chunk_idx) {
            let slot = ring.remove(pos).unwrap();
            let data = Arc::clone(&slot.data);
            ring.push_back(slot);
            return Ok(data);
        }
        drop(ring);

        let offset = chunk_idx.saturating_mul(self.chunk_size as u64);
        if offset >= self.total_size {
            return Ok(Arc::new(Vec::new()));
        }
        let len = ((self.total_size - offset) as usize).min(self.chunk_size);
        let data_vec = (self.loader)(offset, len)?;
        let arc_data = Arc::new(data_vec);

        let mut ring = self.ring_buffer.lock();
        if let Some(pos) = ring.iter().position(|s| s.index == chunk_idx) {
            let slot = ring.remove(pos).unwrap();
            let data = Arc::clone(&slot.data);
            ring.push_back(slot);
            return Ok(data);
        }
        if ring.len() >= self.max_chunks {
            ring.pop_front();
        }
        ring.push_back(ChunkSlot { index: chunk_idx, data: Arc::clone(&arc_data) });
        Ok(arc_data)
    }

    pub fn read_exact_at(&self, offset: u64, length: u32) -> Result<Vec<u8>, TTZipError> {
        if offset >= self.total_size || length == 0 {
            return Ok(Vec::new());
        }
        let to_read = (length as u64).min(self.total_size - offset) as usize;
        let mut result = Vec::with_capacity(to_read);
        let mut cur_offset = offset;
        let mut remaining = to_read;

        while remaining > 0 && cur_offset < self.total_size {
            let chunk_idx = cur_offset / (self.chunk_size as u64);
            let in_chunk_off = (cur_offset % (self.chunk_size as u64)) as usize;
            let chunk = self.get_chunk(chunk_idx)?;
            if in_chunk_off >= chunk.len() {
                break;
            }
            let avail = chunk.len() - in_chunk_off;
            let take = avail.min(remaining);
            result.extend_from_slice(&chunk[in_chunk_off..in_chunk_off + take]);
            cur_offset += take as u64;
            remaining -= take;
            if take == 0 {
                break;
            }
        }
        Ok(result)
    }
}

/// Thread-safe bounded in-memory virtual file stream supporting random seeking and chunked streaming.
#[derive(uniffi::Object)]
pub struct VirtualFileStream {
    inner: VirtualChunkedStream,
    position: Mutex<u64>,
}

#[uniffi::export]
impl VirtualFileStream {
    #[uniffi::constructor]
    pub fn new_empty() -> Arc<Self> {
        Arc::new(Self::from_vec(Vec::new()))
    }

    pub fn size(&self) -> u64 {
        self.inner.total_size
    }

    pub fn position(&self) -> u64 {
        *self.position.lock()
    }

    pub fn seek(&self, offset: u64) -> Result<u64, TTZipError> {
        let mut pos = self.position.lock();
        let target = offset.min(self.inner.total_size);
        *pos = target;
        Ok(target)
    }

    pub fn read(&self, max_bytes: u32) -> Result<Vec<u8>, TTZipError> {
        let mut pos = self.position.lock();
        let chunk = self.inner.read_exact_at(*pos, max_bytes)?;
        *pos += chunk.len() as u64;
        Ok(chunk)
    }

    pub fn read_exact_at(&self, offset: u64, length: u32) -> Result<Vec<u8>, TTZipError> {
        self.inner.read_exact_at(offset, length)
    }

    pub fn read_all(&self) -> Result<Vec<u8>, TTZipError> {
        self.inner.read_exact_at(0, self.inner.total_size.min(u32::MAX as u64) as u32)
    }
}

impl VirtualFileStream {
    pub fn new(inner: VirtualChunkedStream) -> Self {
        Self { inner, position: Mutex::new(0) }
    }

    pub fn from_vec(data: Vec<u8>) -> Self {
        let total_size = data.len() as u64;
        let chunk_size = calculate_chunk_size(total_size);
        let arc_data = Arc::new(data);
        let loader = Arc::new(move |offset: u64, len: usize| {
            let off = offset as usize;
            if off >= arc_data.len() {
                return Ok(Vec::new());
            }
            let end = (off + len).min(arc_data.len());
            Ok(arc_data[off..end].to_vec())
        });
        Self::new(VirtualChunkedStream::new(total_size, chunk_size, loader))
    }

    pub fn from_arc(data: Arc<Vec<u8>>) -> Self {
        let total_size = data.len() as u64;
        let chunk_size = calculate_chunk_size(total_size);
        let loader = Arc::new(move |offset: u64, len: usize| {
            let off = offset as usize;
            if off >= data.len() {
                return Ok(Vec::new());
            }
            let end = (off + len).min(data.len());
            Ok(data[off..end].to_vec())
        });
        Self::new(VirtualChunkedStream::new(total_size, chunk_size, loader))
    }
}

impl Read for VirtualFileStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut pos = self.position.lock();
        let bytes = self.inner.read_exact_at(*pos, buf.len() as u32)
            .map_err(|e| std::io::Error::other(format!("{:?}", e)))?;
        let n = bytes.len();
        buf[..n].copy_from_slice(&bytes);
        *pos += n as u64;
        Ok(n)
    }
}

impl Seek for VirtualFileStream {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let mut curr = self.position.lock();
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::Current(offset) => (*curr as i64) + offset,
            SeekFrom::End(offset) => (self.inner.total_size as i64) + offset,
        };
        if new_pos < 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "negative seek"));
        }
        let clamped = (new_pos as u64).min(self.inner.total_size);
        *curr = clamped;
        Ok(clamped)
    }
}
