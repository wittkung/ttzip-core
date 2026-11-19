// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Parallel RFC 1952 GZIP target stream writer backed by native C/SIMD `libdeflate`.
//!
//! Splits incoming streams into bounded independent chunks (`DEFAULT_GZIP_CHUNK_SIZE`) and
//! compresses batches in parallel via Rayon using thread-local pooled `libdeflate` compressors,
//! emitting valid single-member (`<= 1 MiB`) or concatenated multi-member (RFC 1952 §2.2) gzip
//! streams in strict input order.

use std::io::Write;

use rayon::prelude::*;

use super::CreateTransactionContext;
use crate::types::{TTZipCompressionLevel, TTZipCreateOptions, TTZipStatus};

/// Independent gzip member chunk size (1 MiB).
pub(crate) const DEFAULT_GZIP_CHUNK_SIZE: usize = 1024 * 1024;

/// Maximum number of uncompressed chunks buffered in a single parallel compression batch.
const MAX_PARALLEL_GZIP_BATCH_CHUNKS: usize = 16;

/// Resolved compression parameters for `GzipTargetStreamWriter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GzipStreamConfig {
    pub(crate) level: i32,
    pub(crate) thread_budget: usize,
    pub(crate) chunk_size: usize,
    pub(crate) batch_chunks: usize,
}

#[inline]
pub(super) fn map_gzip_compression_level(level: TTZipCompressionLevel) -> i32 {
    match level {
        TTZipCompressionLevel::Store => 0,
        TTZipCompressionLevel::Fastest => 1,
        TTZipCompressionLevel::Fast => 3,
        TTZipCompressionLevel::Normal => 6,
        TTZipCompressionLevel::Maximum => 9,
        TTZipCompressionLevel::Ultra => 12,
    }
}

pub(crate) fn build_gzip_stream_config(options: &TTZipCreateOptions) -> GzipStreamConfig {
    let ext = crate::types::current_extended_create_options();
    let level = ext
        .raw_level
        .map(|raw| i32::from(raw).clamp(0, 12))
        .unwrap_or_else(|| map_gzip_compression_level(options.level));
    let thread_budget = ttzip_types::resolve_thread_budget(options.thread_budget);
    let batch_chunks = if thread_budget <= 1 {
        1
    } else {
        thread_budget.min(MAX_PARALLEL_GZIP_BATCH_CHUNKS)
    };
    GzipStreamConfig {
        level,
        thread_budget,
        chunk_size: DEFAULT_GZIP_CHUNK_SIZE,
        batch_chunks,
    }
}

fn compress_gzip_member(src: &[u8], level: i32) -> Result<Vec<u8>, TTZipStatus> {
    let bound = crate::codecs::libdeflate::gzip_compress_bound(src.len(), level).max(32);
    let mut out = vec![0u8; bound];
    let written = crate::codecs::libdeflate::gzip_compress(src, &mut out, level)?;
    if written < 18 {
        return Err(TTZipStatus::ErrCompressionFailed);
    }
    out.truncate(written);
    Ok(out)
}

/// Streaming writer that compresses incoming bytes into ordered RFC 1952 gzip members.
pub(crate) struct GzipTargetStreamWriter<W: Write> {
    writer: Option<W>,
    config: GzipStreamConfig,
    pending_chunks: Vec<Vec<u8>>,
    current_chunk: Vec<u8>,
    total_input_bytes: u64,
    finished: bool,
}

impl<W: Write> GzipTargetStreamWriter<W> {
    pub(crate) fn new(writer: W, config: GzipStreamConfig) -> Self {
        Self {
            writer: Some(writer),
            config,
            pending_chunks: Vec::with_capacity(config.batch_chunks),
            current_chunk: Vec::with_capacity(config.chunk_size.min(64 * 1024)),
            total_input_bytes: 0,
            finished: false,
        }
    }

    fn flush_pending_batch(&mut self) -> Result<(), TTZipStatus> {
        if self.pending_chunks.is_empty() {
            return Ok(());
        }
        let batch = std::mem::take(&mut self.pending_chunks);
        let level = self.config.level;
        let compressed_members: Vec<Vec<u8>> = if batch.len() == 1 || self.config.thread_budget <= 1
        {
            batch
                .iter()
                .map(|chunk| compress_gzip_member(chunk, level))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            let pool_threads = self.config.thread_budget.min(self.config.batch_chunks).max(1);
            let pool = crate::archive::shared_compute_pool(pool_threads);
            pool.install(|| {
                batch
                    .into_par_iter()
                    .map(|chunk| compress_gzip_member(&chunk, level))
                    .collect::<Result<Vec<_>, _>>()
            })?
        };
        let writer = self.writer.as_mut().ok_or(TTZipStatus::ErrInvalidParam)?;
        for member in &compressed_members {
            writer
                .write_all(member)
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        }
        Ok(())
    }

    pub(crate) fn finish(mut self) -> Result<W, TTZipStatus> {
        if !self.finished {
            if !self.current_chunk.is_empty() {
                let tail = std::mem::take(&mut self.current_chunk);
                self.pending_chunks.push(tail);
            } else if self.total_input_bytes == 0 && self.pending_chunks.is_empty() {
                self.pending_chunks.push(Vec::new());
            }
            self.flush_pending_batch()?;
            self.finished = true;
            if let Some(w) = &mut self.writer {
                w.flush().map_err(|_| TTZipStatus::ErrCompressionFailed)?;
            }
        }
        self.writer.take().ok_or(TTZipStatus::ErrInvalidParam)
    }
}

impl<W: Write> Write for GzipTargetStreamWriter<W> {
    fn write(&mut self, mut buf: &[u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let total = buf.len();
        self.total_input_bytes = self.total_input_bytes.saturating_add(total as u64);
        let chunk_size = self.config.chunk_size;

        while !buf.is_empty() {
            if self.current_chunk.is_empty() && buf.len() >= chunk_size {
                self.pending_chunks.push(buf[..chunk_size].to_vec());
                buf = &buf[chunk_size..];
            } else {
                let space = chunk_size - self.current_chunk.len();
                let take = space.min(buf.len());
                if self.current_chunk.capacity() < chunk_size && self.current_chunk.len() + take > self.current_chunk.capacity() {
                    self.current_chunk.reserve(chunk_size - self.current_chunk.len());
                }
                self.current_chunk.extend_from_slice(&buf[..take]);
                buf = &buf[take..];
                if self.current_chunk.len() == chunk_size {
                    let full = std::mem::replace(
                        &mut self.current_chunk,
                        Vec::with_capacity(chunk_size),
                    );
                    self.pending_chunks.push(full);
                }
            }

            if self.pending_chunks.len() >= self.config.batch_chunks {
                self.flush_pending_batch()
                    .map_err(|_| std::io::Error::other("gzip parallel batch compression failure"))?;
            }
        }
        Ok(total)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if !self.finished {
            self.flush_pending_batch()
                .map_err(|_| std::io::Error::other("gzip flush compression failure"))?;
            if let Some(w) = &mut self.writer {
                w.flush()?;
            }
        }
        Ok(())
    }
}

impl<W: Write> Drop for GzipTargetStreamWriter<W> {
    fn drop(&mut self) {
        if !self.finished && self.writer.is_some() {
            if !self.current_chunk.is_empty() {
                let tail = std::mem::take(&mut self.current_chunk);
                self.pending_chunks.push(tail);
            } else if self.total_input_bytes == 0 && self.pending_chunks.is_empty() {
                self.pending_chunks.push(Vec::new());
            }
            let _ = self.flush_pending_batch();
        }
    }
}

pub(crate) fn write_gzip_target_in_txn<F>(
    destination_path: &std::path::Path,
    split_volume_size_bytes: u64,
    options: &TTZipCreateOptions,
    raw_source: Option<&std::path::Path>,
    txn_ctx: Option<&CreateTransactionContext>,
    write_tar: F,
) -> Result<(), TTZipStatus>
where
    F: FnOnce(
        GzipTargetStreamWriter<crate::archive::split::StreamOutputTarget>,
    ) -> Result<
        GzipTargetStreamWriter<crate::archive::split::StreamOutputTarget>,
        TTZipStatus,
    >,
{
    let mut raw_in = match raw_source {
        Some(src_file) => {
            Some(std::fs::File::open(src_file).map_err(|_| TTZipStatus::ErrFileNotFound)?)
        }
        None => None,
    };
    let target = crate::archive::split::StreamOutputTarget::open_in_txn(
        destination_path,
        split_volume_size_bytes,
        txn_ctx,
    )
    .map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let config = build_gzip_stream_config(options);
    let mut gzip_writer = GzipTargetStreamWriter::new(target, config);
    if let Some(ref mut in_file) = raw_in {
        std::io::copy(in_file, &mut gzip_writer)
            .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    } else {
        gzip_writer = write_tar(gzip_writer)?;
    }
    gzip_writer
        .finish()?
        .finish_and_settle_in_txn(Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::unified::{create_archive, extract_archive, inspect_archive};
    use crate::fs::collision::staging::StagingRoot;
    use crate::types::{
        with_extended_create_options, TTZipArchiveFormat, TTZipExtendedCreateOptions,
        TTZipExtractOptions,
    };
    use std::io::Read;

    fn decode_multi_gzip(bytes: &[u8]) -> Vec<u8> {
        let mut decoder = flate2::read::MultiGzDecoder::new(bytes);
        let mut out = Vec::new();
        decoder.read_to_end(&mut out).expect("MultiGzDecoder roundtrip");
        out
    }

    fn verify_system_gzip_t(bytes: &[u8]) {
        let tmp = tempfile::NamedTempFile::new().expect("temp file");
        std::fs::write(tmp.path(), bytes).expect("write gzip bytes");
        let out = std::process::Command::new("/usr/bin/gzip")
            .args(["-t", tmp.path().to_str().expect("utf8 path")])
            .output()
            .expect("run /usr/bin/gzip -t");
        assert!(
            out.status.success(),
            "gzip -t rejected output: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn deterministic_payload(len: usize) -> Vec<u8> {
        let mut out = vec![0u8; len];
        let mut state = 0xCAFE_BABE_1234_5678u64;
        for (idx, b) in out.iter_mut().enumerate() {
            if idx % 256 < 192 {
                *b = b'a' + ((idx / 7 + idx % 23) % 26) as u8;
            } else {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *b = (state & 0xFF) as u8;
            }
        }
        out
    }

    #[test]
    fn test_gzip_writer_empty_input_valid_rfc1952() {
        let opts = TTZipCreateOptions {
            level: TTZipCompressionLevel::Normal,
            thread_budget: 4,
            ..Default::default()
        };
        let cfg = build_gzip_stream_config(&opts);
        let writer = GzipTargetStreamWriter::new(Vec::new(), cfg);
        let gz = writer.finish().expect("finish empty gzip");
        assert!(gz.len() >= 20);
        assert_eq!(&gz[..3], &[0x1F, 0x8B, 0x08]);
        assert!(decode_multi_gzip(&gz).is_empty());
        verify_system_gzip_t(&gz);
    }

    #[test]
    fn test_gzip_writer_small_single_member_roundtrip() {
        let payload = deterministic_payload(128 * 1024);
        let opts = TTZipCreateOptions {
            level: TTZipCompressionLevel::Normal,
            thread_budget: 4,
            ..Default::default()
        };
        let mut writer = GzipTargetStreamWriter::new(Vec::new(), build_gzip_stream_config(&opts));
        writer.write_all(&payload).expect("write small payload");
        let gz = writer.finish().expect("finish small gzip");
        assert_eq!(&gz[..3], &[0x1F, 0x8B, 0x08]);
        // Single member must decode cleanly even with single-member GzDecoder.
        let mut single_dec = flate2::read::GzDecoder::new(gz.as_slice());
        let mut decoded = Vec::new();
        single_dec.read_to_end(&mut decoded).expect("single-member decode");
        assert_eq!(decoded, payload);
        verify_system_gzip_t(&gz);
    }

    #[test]
    fn test_gzip_writer_multi_member_parallel_and_single_thread_exact_roundtrip() {
        let payload = deterministic_payload(3 * 1024 * 1024 + 37_123);
        for threads in [1u32, 4u32] {
            let opts = TTZipCreateOptions {
                level: TTZipCompressionLevel::Normal,
                thread_budget: threads,
                ..Default::default()
            };
            let mut writer =
                GzipTargetStreamWriter::new(Vec::new(), build_gzip_stream_config(&opts));
            for chunk in payload.chunks(32 * 1024 + 13) {
                writer.write_all(chunk).expect("write chunk");
            }
            let gz = writer.finish().expect("finish multi-member gzip");
            let decoded = decode_multi_gzip(&gz);
            assert_eq!(decoded, payload, "byte mismatch for threads={threads}");
            verify_system_gzip_t(&gz);
        }
    }

    #[test]
    fn test_gzip_config_honors_levels_and_extended_raw_level() {
        let base_opts = TTZipCreateOptions {
            level: TTZipCompressionLevel::Store,
            thread_budget: 2,
            ..Default::default()
        };
        assert_eq!(build_gzip_stream_config(&base_opts).level, 0);
        let ext = TTZipExtendedCreateOptions {
            dictionary_size_mb: 0,
            enable_ldm: false,
            raw_level: Some(11),
        };
        with_extended_create_options(ext, || {
            let cfg = build_gzip_stream_config(&base_opts);
            assert_eq!(cfg.level, 11);
            assert_eq!(cfg.thread_budget, 2);
        });

        let payload = deterministic_payload(64 * 1024);
        let mut store_w =
            GzipTargetStreamWriter::new(Vec::new(), build_gzip_stream_config(&base_opts));
        store_w.write_all(&payload).expect("write store");
        let store_gz = store_w.finish().expect("finish store");
        assert_eq!(decode_multi_gzip(&store_gz), payload);
        verify_system_gzip_t(&store_gz);
    }

    #[test]
    fn test_tar_gz_parallel_archive_roundtrip_and_system_interop() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let src_dir = tmp.path().join("pkg");
        std::fs::create_dir_all(src_dir.join("nested")).expect("mkdir");
        let p1 = deterministic_payload(1_400_000);
        let p2 = deterministic_payload(1_500_000);
        let p3 = b"tiny file payload\n".to_vec();
        std::fs::write(src_dir.join("a.bin"), &p1).expect("write a");
        std::fs::write(src_dir.join("nested/b.bin"), &p2).expect("write b");
        std::fs::write(src_dir.join("nested/c.txt"), &p3).expect("write c");

        let archive_path = tmp.path().join("out.tar.gz");
        let opts = TTZipCreateOptions {
            format: TTZipArchiveFormat::TarGz,
            level: TTZipCompressionLevel::Normal,
            thread_budget: 4,
            ..Default::default()
        };
        create_archive(std::slice::from_ref(&src_dir), &archive_path, &opts, 0)
            .expect("create parallel tar.gz");

        let gzip_t = std::process::Command::new("/usr/bin/gzip")
            .args(["-t", archive_path.to_str().expect("utf8")])
            .output()
            .expect("gzip -t");
        assert!(gzip_t.status.success(), "gzip -t failed");

        let tar_list = std::process::Command::new("/usr/bin/bsdtar")
            .args(["-tzf", archive_path.to_str().expect("utf8")])
            .output()
            .expect("bsdtar -tzf");
        assert!(tar_list.status.success(), "bsdtar -tzf failed");
        let listed = String::from_utf8_lossy(&tar_list.stdout);
        assert!(listed.contains("pkg/a.bin"));
        assert!(listed.contains("pkg/nested/b.bin"));
        assert!(listed.contains("pkg/nested/c.txt"));

        let mut inspected = Vec::new();
        inspect_archive(&archive_path, None, false, |entry| {
            inspected.push(entry.path.to_string());
            true
        })
        .expect("inspect tar.gz");
        assert!(inspected.iter().any(|p| p.ends_with("pkg/nested/b.bin")));

        let out_dir = tmp.path().join("extracted");
        extract_archive(
            &archive_path,
            &StagingRoot(out_dir.clone()),
            &TTZipExtractOptions::default(),
        )
        .expect("extract tar.gz");
        assert_eq!(std::fs::read(out_dir.join("pkg/a.bin")).unwrap(), p1);
        assert_eq!(std::fs::read(out_dir.join("pkg/nested/b.bin")).unwrap(), p2);
        assert_eq!(std::fs::read(out_dir.join("pkg/nested/c.txt")).unwrap(), p3);
    }

    #[test]
    fn test_shared_compute_pool_honors_budget_and_reuses_workers() {
        use std::collections::HashSet;

        let default_threads = ttzip_types::recommended_compute_threads();
        let above_default = (default_threads + 1).min(64);
        let below_default = if default_threads > 2 { 2 } else { 1 };

        assert_eq!(crate::archive::shared_compute_pool(0).thread_count(), 1);
        for &requested in &[1usize, below_default, 3, above_default] {
            let p1 = crate::archive::shared_compute_pool(requested);
            let p2 = crate::archive::shared_compute_pool(requested);
            assert_eq!(p1.thread_count(), requested);
            assert_eq!(p2.thread_count(), requested);
            assert_eq!(p1.install(rayon::current_num_threads), requested);

            let ids1: HashSet<_> = p1
                .install(|| rayon::broadcast(|_| std::thread::current().id()))
                .into_iter()
                .collect();
            let ids2: HashSet<_> = p2
                .install(|| rayon::broadcast(|_| std::thread::current().id()))
                .into_iter()
                .collect();
            assert_eq!(ids1.len(), requested);
            assert_eq!(
                ids1, ids2,
                "shared_compute_pool({requested}) must reuse the same worker threads without respawning"
            );
        }
    }
}
