// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Explicit ZIP batch compression budgets and ordered scoped execution.

use super::parallel_batch::{compress_single_item, CompressedEntryResult, CompressionPlanItem};
use crate::types::{TTZipArchiveFormat, TTZipEncryptionMethod, TTZipStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use ttzip_types::options::ProgressAtomicSheet;

#[cfg(test)]
mod execution_probe;
#[cfg(test)]
mod tests;

/// Per-call compression context; the worker limit never comes from ambient topology.
#[derive(Clone, Copy)]
pub struct BatchCompressionOptions<'a> {
    pub level: i32,
    pub format: TTZipArchiveFormat,
    pub encryption: TTZipEncryptionMethod,
    pub password: Option<&'a str>,
    pub sheet: Option<&'a ProgressAtomicSheet>,
    /// Zero preserves automatic topology selection; explicit budgets clamp to 1..=64.
    pub thread_budget: u32,
}

type EntryResult = Result<CompressedEntryResult, TTZipStatus>;
type IndexedResults = Vec<(usize, EntryResult)>;

#[derive(Clone)]
struct ExecutionContext<'a> {
    options: BatchCompressionOptions<'a>,
    started: Instant,
    #[cfg(test)]
    probe: Option<std::sync::Arc<execution_probe::Probe>>,
}

impl ExecutionContext<'_> {
    fn compress(&self, item: CompressionPlanItem) -> EntryResult {
        #[cfg(test)]
        let _span = self.probe.as_ref().map(|probe| probe.enter(&item.rel_path));
        if self
            .options
            .sheet
            .is_some_and(ProgressAtomicSheet::is_cancelled)
        {
            return Err(TTZipStatus::Cancelled);
        }
        compress_single_item(
            item,
            self.options.level,
            self.options.format,
            self.options.encryption,
            self.options.password,
            self.options.sheet,
            Some(self.started),
        )
    }
}

/// Executes on the caller for budget one, otherwise joins every bounded scoped worker.
pub fn execute_batch_parallel_with_options(
    batch: Vec<CompressionPlanItem>,
    options: BatchCompressionOptions<'_>,
) -> Vec<EntryResult> {
    if batch.is_empty() {
        return Vec::new();
    }
    if options.sheet.is_some_and(ProgressAtomicSheet::is_cancelled) {
        return (0..batch.len())
            .map(|_| Err(TTZipStatus::Cancelled))
            .collect();
    }
    let workers = resolve_workers(options.thread_budget, batch.len());
    let context = ExecutionContext {
        options,
        started: Instant::now(),
        #[cfg(test)]
        probe: execution_probe::current(),
    };
    if workers == 1 {
        return batch
            .into_iter()
            .map(|item| context.compress(item))
            .collect();
    }
    execute_scoped(batch, workers, context)
}

fn resolve_workers(requested: u32, items: usize) -> usize {
    let budget = if requested == 0 {
        ttzip_types::recommended_compute_threads()
    } else {
        (requested as usize).clamp(1, 64)
    };
    budget.min(items).max(1)
}

fn execute_scoped(
    batch: Vec<CompressionPlanItem>,
    workers: usize,
    context: ExecutionContext<'_>,
) -> Vec<EntryResult> {
    let total_items = batch.len();
    let mut buckets: Vec<Vec<(usize, CompressionPlanItem)>> =
        (0..workers).map(|_| Vec::new()).collect();
    for (index, item) in batch.into_iter().enumerate() {
        buckets[index % workers].push((index, item));
    }
    let mut slots: Vec<Option<EntryResult>> = (0..total_items).map(|_| None).collect();
    std::thread::scope(|scope| {
        let handles: Vec<_> = buckets
            .into_iter()
            .map(|bucket| {
                let context = context.clone();
                scope.spawn(move || compress_bucket(bucket, &context))
            })
            .collect();
        for handle in handles {
            if let Ok(results) = handle.join() {
                for (index, result) in results {
                    slots[index] = Some(result);
                }
            }
        }
    });
    slots
        .into_iter()
        .map(|slot| slot.unwrap_or(Err(TTZipStatus::Cancelled)))
        .collect()
}

fn compress_bucket(
    bucket: Vec<(usize, CompressionPlanItem)>,
    context: &ExecutionContext<'_>,
) -> IndexedResults {
    bucket
        .into_iter()
        .map(|(index, item)| (index, context.compress(item)))
        .collect()
}

/// Compatibility API retaining its historical automatic CPU budget.
/// Production callers with an explicit budget use `execute_batch_parallel_with_options`.
pub fn execute_batch_parallel(
    batch: Vec<CompressionPlanItem>,
    level: i32,
    format: TTZipArchiveFormat,
    encryption: TTZipEncryptionMethod,
    password: Option<&str>,
    sheet: Option<&ProgressAtomicSheet>,
) -> Vec<EntryResult> {
    execute_batch_parallel_with_options(
        batch,
        BatchCompressionOptions {
            level,
            format,
            encryption,
            password,
            sheet,
            thread_budget: 0,
        },
    )
}

/// Compatibility adapter retaining the original raw-flag snapshot semantics.
pub fn execute_batch_parallel_with_flag(
    batch: Vec<CompressionPlanItem>,
    level: i32,
    format: TTZipArchiveFormat,
    encryption: TTZipEncryptionMethod,
    password: Option<&str>,
    is_cancelled: &AtomicBool,
) -> Vec<EntryResult> {
    let local_sheet = ProgressAtomicSheet::new();
    if is_cancelled.load(Ordering::Relaxed) {
        local_sheet.cancel();
    }
    execute_batch_parallel(
        batch,
        level,
        format,
        encryption,
        password,
        Some(&local_sheet),
    )
}
