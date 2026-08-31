// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! TTZip Multi-Threaded Streaming Pipeline (`TTZipMT`).
//!
//! Inspired by Facebook ZSTDMT, provides:
//! 1. `BufferPool`: Recycled, low-contention chunk memory allocator;
//! 2. `JobScheduler`: Lock-free chunk job partitioner and Rayon task dispatcher;
//! 3. `OrderedDrainer`: Strictly monotonic ordered sink drainer for out-of-order worker outputs;
//! 4. `TTZipMtEngine`: High-level multi-core parallel streaming compression/decompression pipeline.

use crate::archive::zero_vtable_dispatch::ArchiveEngineStrategy;
use crate::types::{TTZipCompressionLevel, TTZipStatus};
use parking_lot::Mutex;
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

// MARK: - Constants

/// Default buffer pool chunk size: 256 KB.
pub const DEFAULT_CHUNK_SIZE: usize = 256 * 1024;

/// Default maximum buffer pool cache capacity.
pub const DEFAULT_MAX_POOL_BUFFERS: usize = 64;

/// Default maximum in-flight pipeline jobs to throttle resident memory (<= 64MB).
pub const DEFAULT_MAX_IN_FLIGHT_JOBS: usize = 32;

// MARK: - Buffer Pool & RAII Guard

/// Low-contention chunk memory pool.
#[derive(Debug)]
pub struct BufferPool {
    chunk_size: usize,
    max_buffers: usize,
    free_buffers: Mutex<Vec<Vec<u8>>>,
    allocated_count: AtomicUsize,
}

impl BufferPool {
    /// Constructs a new chunk memory pool.
    pub fn new(chunk_size: usize, max_buffers: usize) -> Self {
        Self {
            chunk_size: chunk_size.max(4096),
            max_buffers: max_buffers.max(4),
            free_buffers: Mutex::new(Vec::with_capacity(max_buffers)),
            allocated_count: AtomicUsize::new(0),
        }
    }

    /// Acquires a pooled buffer with pre-allocated capacity.
    pub fn acquire(self: &Arc<Self>) -> PooledBuffer {
        let buf = {
            let mut guard = self.free_buffers.lock();
            guard.pop().unwrap_or_else(|| {
                self.allocated_count.fetch_add(1, Ordering::Relaxed);
                vec![0u8; self.chunk_size]
            })
        };

        PooledBuffer {
            buffer: buf,
            pool: Some(Arc::clone(self)),
        }
    }

    /// Acquires a standalone raw buffer without RAII pool return.
    pub fn acquire_raw(&self) -> Vec<u8> {
        let mut guard = self.free_buffers.lock();
        guard.pop().unwrap_or_else(|| {
            self.allocated_count.fetch_add(1, Ordering::Relaxed);
            vec![0u8; self.chunk_size]
        })
    }

    /// Releases a buffer back to the pool.
    pub fn release(&self, mut buf: Vec<u8>) {
        if buf.capacity() >= self.chunk_size {
            buf.resize(self.chunk_size, 0);
            let mut guard = self.free_buffers.lock();
            if guard.len() < self.max_buffers {
                guard.push(buf);
            }
        }
    }

    /// Returns total buffers currently created.
    #[inline]
    pub fn total_allocated(&self) -> usize {
        self.allocated_count.load(Ordering::Relaxed)
    }

    /// Returns currently cached free buffers.
    #[inline]
    pub fn available_count(&self) -> usize {
        self.free_buffers.lock().len()
    }
}

/// RAII wrapper returning underlying buffer to `BufferPool` upon drop.
pub struct PooledBuffer {
    buffer: Vec<u8>,
    pool: Option<Arc<BufferPool>>,
}

impl Deref for PooledBuffer {
    type Target = [u8];
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl DerefMut for PooledBuffer {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        if let Some(pool) = self.pool.take() {
            let buf = std::mem::take(&mut self.buffer);
            pool.release(buf);
        }
    }
}

impl PooledBuffer {
    /// Creates a standalone pooled buffer.
    pub fn standalone(buf: Vec<u8>) -> Self {
        Self {
            buffer: buf,
            pool: None,
        }
    }

    /// Extracts inner vector and detaches from pool.
    pub fn into_inner(mut self) -> Vec<u8> {
        self.pool = None;
        std::mem::take(&mut self.buffer)
    }

    /// Returns underlying slice.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.buffer
    }

    /// Returns mutable underlying slice.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.buffer
    }
}

// MARK: - Pipeline Data Structures

/// Discrete multi-threaded processing unit.
#[derive(Debug)]
pub struct Job {
    pub job_id: u64,
    pub input_data: Vec<u8>,
    pub is_last: bool,
    pub original_size: usize,
}

/// Completed computation result from an out-of-order worker thread.
#[derive(Debug)]
pub struct CompletedJob {
    pub job_id: u64,
    pub is_last: bool,
    pub result: Result<Vec<u8>, TTZipStatus>,
    pub raw_bytes: usize,
    pub processed_bytes: usize,
}

// MARK: - Job Scheduler

/// Lock-free chunk job partitioner and task scheduler.
#[derive(Debug)]
pub struct JobScheduler {
    job_counter: AtomicU64,
    chunk_size: usize,
    max_in_flight: usize,
}

impl JobScheduler {
    /// Creates a new job scheduler.
    pub fn new(chunk_size: usize, max_in_flight: usize) -> Self {
        Self {
            job_counter: AtomicU64::new(0),
            chunk_size: chunk_size.max(4096),
            max_in_flight: max_in_flight.max(1),
        }
    }

    /// Resets the internal monotonic job counter to 0.
    #[inline]
    pub fn reset_job_counter(&self) {
        self.job_counter.store(0, Ordering::Relaxed);
    }

    /// Chunks a `Read` stream into a batch of discrete jobs up to `max_in_flight`.
    pub fn read_batch<R: Read>(
        &self,
        reader: &mut R,
        pool: &Arc<BufferPool>,
        max_batch_size: usize,
    ) -> Result<(Vec<Job>, bool), TTZipStatus> {
        let mut jobs = Vec::with_capacity(max_batch_size.min(self.max_in_flight));
        let mut is_eof = false;

        while jobs.len() < max_batch_size {
            let mut buf = pool.acquire_raw();
            buf.resize(self.chunk_size, 0);

            let mut total_read = 0;
            while total_read < self.chunk_size {
                match reader.read(&mut buf[total_read..]) {
                    Ok(0) => {
                        is_eof = true;
                        break;
                    }
                    Ok(n) => {
                        total_read += n;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => return Err(TTZipStatus::ErrExtractionFailed),
                }
            }

            if total_read == 0 {
                is_eof = true;
                if jobs.is_empty() {
                    // Empty input or immediate EOF
                    let job_id = self.job_counter.fetch_add(1, Ordering::Relaxed);
                    jobs.push(Job {
                        job_id,
                        input_data: Vec::new(),
                        is_last: true,
                        original_size: 0,
                    });
                } else if let Some(last) = jobs.last_mut() {
                    last.is_last = true;
                }
                break;
            }

            buf.truncate(total_read);
            let job_id = self.job_counter.fetch_add(1, Ordering::Relaxed);
            let is_last = is_eof;

            jobs.push(Job {
                job_id,
                input_data: buf,
                is_last,
                original_size: total_read,
            });

            if is_eof {
                break;
            }
        }

        Ok((jobs, is_eof))
    }

    /// Dispatches a batch of jobs across worker threads via Rayon.
    pub fn dispatch_parallel<F>(&self, jobs: Vec<Job>, worker: F) -> Vec<CompletedJob>
    where
        F: Fn(Job) -> CompletedJob + Sync + Send,
    {
        jobs.into_par_iter().map(worker).collect()
    }
}

// MARK: - Ordered Drainer

/// Pipeline throughput and byte transformation statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PipelineMetrics {
    pub total_input_bytes: u64,
    pub total_output_bytes: u64,
    pub total_jobs_processed: u64,
}

/// Strictly monotonic ordered sink drainer.
#[derive(Debug)]
pub struct OrderedDrainer {
    next_drain_id: u64,
    pending_jobs: BTreeMap<u64, CompletedJob>,
    is_finished: bool,
    metrics: PipelineMetrics,
}

impl Default for OrderedDrainer {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderedDrainer {
    /// Constructs a new empty ordered drainer starting at job 0.
    pub fn new() -> Self {
        Self {
            next_drain_id: 0,
            pending_jobs: BTreeMap::new(),
            is_finished: false,
            metrics: PipelineMetrics::default(),
        }
    }

    /// Submits an out-of-order completed job into the reordering queue.
    pub fn submit(&mut self, job: CompletedJob) {
        self.pending_jobs.insert(job.job_id, job);
    }

    /// Drains all currently ready contiguous jobs into the `Write` sink in order.
    pub fn drain_ready<W: Write>(&mut self, writer: &mut W) -> Result<usize, TTZipStatus> {
        let mut bytes_written = 0;

        while let Some(entry) = self.pending_jobs.remove(&self.next_drain_id) {
            let is_last = entry.is_last;
            let raw_len = entry.raw_bytes;
            let output_data = entry.result?;
            let produced_len = output_data.len();

            if !output_data.is_empty() {
                writer
                    .write_all(&output_data)
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
                bytes_written += produced_len;
            }

            self.metrics.total_input_bytes =
                self.metrics.total_input_bytes.saturating_add(raw_len as u64);
            self.metrics.total_output_bytes = self
                .metrics
                .total_output_bytes
                .saturating_add(produced_len as u64);
            self.metrics.total_jobs_processed += 1;
            self.next_drain_id += 1;

            if is_last {
                self.is_finished = true;
                break;
            }
        }

        Ok(bytes_written)
    }

    /// Flushes writer and returns true if the last block has been drained.
    pub fn is_finished(&self) -> bool {
        self.is_finished
    }

    /// Returns accumulated metrics.
    #[inline]
    pub fn metrics(&self) -> PipelineMetrics {
        self.metrics
    }
}

// MARK: - TTZipMT High-Level Multi-Core Engine

/// High-level multi-core streaming compression and decompression engine.
pub struct TTZipMtEngine {
    chunk_size: usize,
    max_in_flight: usize,
    buffer_pool: Arc<BufferPool>,
    scheduler: JobScheduler,
}

impl TTZipMtEngine {
    /// Creates a new `TTZipMtEngine` with specified chunk size and buffer pool limits.
    pub fn new(chunk_size: usize, max_in_flight: usize) -> Self {
        let c_size = chunk_size.max(4096);
        let in_flight = max_in_flight.max(2);
        let pool = Arc::new(BufferPool::new(c_size, in_flight * 2));
        let sched = JobScheduler::new(c_size, in_flight);

        Self {
            chunk_size: c_size,
            max_in_flight: in_flight,
            buffer_pool: pool,
            scheduler: sched,
        }
    }

    /// Returns configured chunk size in bytes.
    #[inline]
    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    /// Returns configured maximum in-flight tasks.
    #[inline]
    pub fn max_in_flight(&self) -> usize {
        self.max_in_flight
    }


    /// Executes parallel multi-threaded stream processing with guaranteed ordered output.
    pub fn process_stream<R: Read, W: Write, F>(
        &self,
        reader: &mut R,
        writer: &mut W,
        worker: F,
    ) -> Result<PipelineMetrics, TTZipStatus>
    where
        F: Fn(&[u8]) -> Result<Vec<u8>, TTZipStatus> + Sync + Send,
    {
        self.scheduler.reset_job_counter();
        let mut drainer = OrderedDrainer::new();

        loop {
            let (jobs, is_eof) =
                self.scheduler
                    .read_batch(reader, &self.buffer_pool, self.max_in_flight)?;
            if jobs.is_empty() {
                break;
            }

            // Parallel execution via Rayon
            let completed_batch = self.scheduler.dispatch_parallel(jobs, |job| {
                let raw_len = job.input_data.len();
                let is_last = job.is_last;
                let job_id = job.job_id;

                let res = worker(&job.input_data);
                let processed_len = res.as_ref().map(|v| v.len()).unwrap_or(0);

                CompletedJob {
                    job_id,
                    is_last,
                    result: res,
                    raw_bytes: raw_len,
                    processed_bytes: processed_len,
                }
            });

            // Reorder and submit to drainer
            for comp in completed_batch {
                drainer.submit(comp);
            }

            drainer.drain_ready(writer)?;

            if is_eof || drainer.is_finished() {
                break;
            }
        }

        writer.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        Ok(drainer.metrics())
    }

    /// Parallel block compression over stream using specified strategy.
    pub fn compress_stream_parallel<R: Read, W: Write>(
        &self,
        reader: &mut R,
        writer: &mut W,
        strategy: ArchiveEngineStrategy,
        level: TTZipCompressionLevel,
    ) -> Result<PipelineMetrics, TTZipStatus> {
        self.process_stream(reader, writer, |chunk| {
            strategy.compress_to_vec(chunk, level)
        })
    }

    /// Parallel block decompression over stream using specified strategy.
    pub fn decompress_stream_parallel<R: Read, W: Write>(
        &self,
        reader: &mut R,
        writer: &mut W,
        strategy: ArchiveEngineStrategy,
    ) -> Result<PipelineMetrics, TTZipStatus> {
        self.process_stream(reader, writer, |chunk| {
            strategy.decompress_to_vec(chunk)
        })
    }
}
