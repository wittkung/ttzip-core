// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Multi-volume split archive writer with byte-level accuracy, volume rotation,
//! and explicit transaction-bound descriptor finish/cleanup semantics.

use super::VolumeNamingScheme;
use crate::archive::unified::create_transaction::{
    CloseScopePrecondition, CreateFileAccessMode, CreateTransactionContext,
    FileFinishSyncPolicy, ResidualPathRole,
};
use crate::types::TTZipStatus;
use std::fs::{self, File};
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Computes volume file path for the given 1-based index according to the naming scheme.
pub fn compute_volume_path(
    base_path: &Path,
    index: usize,
    naming_scheme: VolumeNamingScheme,
) -> PathBuf {
    match naming_scheme {
        VolumeNamingScheme::NumberedExtension | VolumeNamingScheme::RawSplit => {
            PathBuf::from(format!("{}.{:03}", base_path.display(), index))
        }
        VolumeNamingScheme::PkzipSpanned => {
            let parent = base_path.parent().unwrap_or_else(|| Path::new(""));
            let stem = base_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("archive");
            parent.join(format!("{}.z{:02}", stem, index))
        }
    }
}

struct OpenVolumeSlot {
    path: PathBuf,
    file: File,
    guard: crate::fs::FileAbortGuard,
}

struct ClosedVolumeSlot {
    path: PathBuf,
    guard: crate::fs::FileAbortGuard,
}

fn close_volume_slot_for_rotation(
    slot: OpenVolumeSlot,
    prior_res: Result<(), (TTZipStatus, String)>,
) -> Result<ClosedVolumeSlot, io::Error> {
    let expected_id = slot.guard.expected_file_id().unwrap_or((0, 0));
    let scope = CloseScopePrecondition::for_current_target_unique_regular_file();
    match crate::fs::finish_io::finish_and_close_file_for_proof(
        slot.file,
        expected_id,
        prior_res,
        None,
        FileFinishSyncPolicy::None,
        scope,
    ) {
        Ok(_proof) => Ok(ClosedVolumeSlot {
            path: slot.path,
            guard: slot.guard,
        }),
        // A primary cancellation/encoder failure does not imply that close failed.
        // Keep the guard owned so the abort caller can explicitly check unlink.
        Err(err) if !err.has_any_io_finish_error() && err.is_release_confirmed() => {
            Ok(ClosedVolumeSlot {
                path: slot.path,
                guard: slot.guard,
            })
        }
        Err(err) => {
            let mut guard = slot.guard;
            guard.settle_failed_file_finish(&err);
            Err(err
                .close_error
                .unwrap_or_else(|| io::Error::other("split volume close failed")))
        }
    }
}

/// Zero-copy multi-volume split archive writer sink.
pub struct SplitVolumeWriter {
    base_path: PathBuf,
    volume_size_bytes: u64,
    naming_scheme: VolumeNamingScheme,
    clean_on_failure: bool,
    current_volume_index: usize,
    bytes_written_in_current: u64,
    total_bytes_written: u64,
    active_slot: Option<Box<OpenVolumeSlot>>,
    retained_first_slot: Option<Box<OpenVolumeSlot>>,
    closed_slots: Vec<ClosedVolumeSlot>,
    generated_volume_paths: Vec<PathBuf>,
    txn_ctx: Option<CreateTransactionContext>,
    is_closed: bool,
}

impl SplitVolumeWriter {
    /// Creates a new `SplitVolumeWriter` targeting `base_path`.
    pub fn new(
        base_path: impl AsRef<Path>,
        volume_size_bytes: u64,
        naming_scheme: VolumeNamingScheme,
    ) -> io::Result<Self> {
        Self::new_in_txn(base_path, volume_size_bytes, naming_scheme, None)
    }

    /// Creates a new `SplitVolumeWriter` bound to an optional `CreateTransactionContext`.
    pub fn new_in_txn(
        base_path: impl AsRef<Path>,
        volume_size_bytes: u64,
        naming_scheme: VolumeNamingScheme,
        txn_ctx: Option<&CreateTransactionContext>,
    ) -> io::Result<Self> {
        if volume_size_bytes == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Volume size must be greater than zero",
            ));
        }

        let base_path_buf = base_path.as_ref().to_path_buf();
        if let Some(parent) = base_path_buf.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        let mut writer = Self {
            base_path: base_path_buf,
            volume_size_bytes,
            naming_scheme,
            clean_on_failure: true,
            current_volume_index: 1,
            bytes_written_in_current: 0,
            total_bytes_written: 0,
            active_slot: None,
            retained_first_slot: None,
            closed_slots: Vec::new(),
            generated_volume_paths: Vec::new(),
            txn_ctx: Some(txn_ctx.cloned().unwrap_or_else(|| {
                let parent = base_path.as_ref().parent().filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or_else(|| Path::new("."));
                CreateTransactionContext::new(parent.to_path_buf())
            })),
            is_closed: false,
        };

        writer.open_volume(1)?;
        Ok(writer)
    }

    /// Configures whether generated volumes should be purged on cancellation/failure.
    pub fn with_clean_on_failure(mut self, clean: bool) -> Self {
        self.clean_on_failure = clean;
        self
    }

    /// Total bytes written across all volumes so far.
    #[inline]
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes_written
    }

    /// Returns list of all generated volume paths.
    #[inline]
    pub fn generated_volumes(&self) -> &[PathBuf] {
        &self.generated_volume_paths
    }

    /// Whether the writer has been closed.
    #[inline]
    pub fn is_closed(&self) -> bool {
        self.is_closed
    }

    /// Opens a volume file for writing at `index` (1-based).
    fn open_volume(&mut self, index: usize) -> io::Result<()> {
        let path = compute_volume_path(&self.base_path, index, self.naming_scheme);
        let (file, guard) = crate::fs::FileAbortGuard::create_staged_file_in_txn(
            &path,
            CreateFileAccessMode::WriteOnly,
            self.txn_ctx.as_ref(),
            ResidualPathRole::StagedArtifact,
        )?;
        self.active_slot = Some(Box::new(OpenVolumeSlot {
            path: path.clone(),
            file,
            guard,
        }));
        self.current_volume_index = index;
        self.bytes_written_in_current = 0;
        self.generated_volume_paths.push(path);
        Ok(())
    }

    /// Rotates from the active volume to the next volume index.
    fn rotate_to_next_volume(&mut self) -> io::Result<()> {
        if let Some(slot) = self.active_slot.take() {
            if self.current_volume_index == 1 && self.retained_first_slot.is_none() {
                self.retained_first_slot = Some(slot);
            } else {
                let closed = close_volume_slot_for_rotation(*slot, Ok(()))?;
                self.closed_slots.push(closed);
            }
        }
        self.open_volume(self.current_volume_index + 1)
    }

    /// Patches the header bytes at offset 0 of the first volume (`.001`) using the retained
    /// volume-1 file handle without re-opening by path.
    pub fn write_header_at_start(&mut self, header: &[u8]) -> io::Result<()> {
        if self.current_volume_index == 1 {
            let slot = self.active_slot.as_mut().ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "No active volume file")
            })?;
            slot.file.seek(SeekFrom::Start(0))?;
            slot.file.write_all(header)?;
            slot.file
                .seek(SeekFrom::Start(self.bytes_written_in_current))?;
            Ok(())
        } else {
            let first_slot = self.retained_first_slot.as_mut().ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "Missing retained first volume handle")
            })?;
            first_slot.file.seek(SeekFrom::Start(0))?;
            first_slot.file.write_all(header)?;
            Ok(())
        }
    }

    fn abort_all_closed_slots(&mut self, status: TTZipStatus) {
        for mut closed in self.closed_slots.drain(..) {
            let _ = closed.guard.abort_checked_detailed(status);
        }
        self.generated_volume_paths.clear();
    }

    fn finalize_pkzip_spanned_last_volume(&mut self) -> io::Result<()> {
        if self.naming_scheme != VolumeNamingScheme::PkzipSpanned
            || self.generated_volume_paths.is_empty()
        {
            return Ok(());
        }
        let last_path = self.generated_volume_paths.pop().unwrap();
        let final_zip_path = self.base_path.clone();
        if last_path != final_zip_path {
            if let Err(e) = fs::rename(&last_path, &final_zip_path) {
                self.abort_all_closed_slots(TTZipStatus::ErrCompressionFailed);
                return Err(e);
            }
        }
        self.generated_volume_paths.push(final_zip_path);
        Ok(())
    }

    /// Closes all open volume handles once with B1 release verification and commits guards.
    pub fn close_in_txn(&mut self) -> io::Result<Vec<PathBuf>> {
        if self.is_closed {
            return Ok(self.generated_volume_paths.clone());
        }
        self.is_closed = true;

        let mut close_err: Option<io::Error> = None;
        if let Some(first_slot) = self.retained_first_slot.take() {
            match close_volume_slot_for_rotation(*first_slot, Ok(())) {
                Ok(closed) => self.closed_slots.push(closed),
                Err(e) => close_err = Some(e),
            }
        }
        if let Some(active_slot) = self.active_slot.take() {
            let prior = if close_err.is_some() {
                Err((
                    TTZipStatus::ErrCompressionFailed,
                    "earlier split volume close failed".to_string(),
                ))
            } else {
                Ok(())
            };
            match close_volume_slot_for_rotation(*active_slot, prior) {
                Ok(closed) => self.closed_slots.push(closed),
                Err(e) if close_err.is_none() => close_err = Some(e),
                Err(_) => {}
            }
        }
        if let Some(err) = close_err {
            self.abort_all_closed_slots(TTZipStatus::ErrCompressionFailed);
            return Err(err);
        }

        self.finalize_pkzip_spanned_last_volume()?;

        for closed in self.closed_slots.drain(..) {
            let _ = closed.path;
            closed.guard.commit();
        }

        Ok(self.generated_volume_paths.clone())
    }

    /// Flushes and closes all volume handles, applying final volume naming conventions.
    pub fn close(&mut self) -> io::Result<Vec<PathBuf>> {
        self.close_in_txn()
    }

    /// Purges all generated volumes in the event of an archive failure with explicit single-close
    /// verification on any open descriptors before unlinking.
    pub fn cancel_and_cleanup_in_txn(&mut self, primary_status: TTZipStatus) -> io::Result<()> {
        self.is_closed = true;
        let mut first_err: Option<io::Error> = None;

        for open_slot in [self.active_slot.take(), self.retained_first_slot.take()]
            .into_iter()
            .flatten()
        {
            match close_volume_slot_for_rotation(
                *open_slot,
                Err((
                    primary_status,
                    format!("split volume aborted with status {:?}", primary_status),
                )),
            ) {
                Ok(closed) => self.closed_slots.push(closed),
                Err(e) if first_err.is_none() => first_err = Some(e),
                Err(_) => {}
            }
        }

        self.generated_volume_paths.clear();
        if self.clean_on_failure {
            for mut closed in self.closed_slots.drain(..).rev() {
                if let Err(e) = closed.guard.abort_checked_detailed(primary_status) {
                    if first_err.is_none() {
                        first_err = Some(io::Error::other(e.to_string()));
                    }
                }
            }
        } else {
            for closed in self.closed_slots.drain(..) {
                closed.guard.commit();
            }
        }

        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Purges all generated volumes in the event of an archive failure, returning the first
    /// non-`NotFound` removal error if any volume could not be deleted.
    pub fn cancel_and_cleanup_checked(&mut self) -> io::Result<()> {
        self.cancel_and_cleanup_in_txn(TTZipStatus::ErrCompressionFailed)
    }

    /// Purges all generated volumes in the event of an archive failure.
    pub fn cancel_and_cleanup(&mut self) {
        let _ = self.cancel_and_cleanup_checked();
    }
}

impl Write for SplitVolumeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.is_closed {
            return Err(io::Error::other(
                "Cannot write to a closed SplitVolumeWriter",
            ));
        }
        if buf.is_empty() {
            return Ok(0);
        }

        let mut bytes_remaining = buf.len();
        let mut current_offset = 0;

        while bytes_remaining > 0 {
            let space_in_current = self
                .volume_size_bytes
                .saturating_sub(self.bytes_written_in_current);
            let to_write = (bytes_remaining as u64).min(space_in_current) as usize;

            if to_write > 0 {
                let slot = self.active_slot.as_mut().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, "No active volume file")
                })?;
                slot.file
                    .write_all(&buf[current_offset..current_offset + to_write])?;
                self.bytes_written_in_current += to_write as u64;
                self.total_bytes_written += to_write as u64;
                current_offset += to_write;
                bytes_remaining -= to_write;
            }

            if self.bytes_written_in_current >= self.volume_size_bytes && bytes_remaining > 0 {
                self.rotate_to_next_volume()?;
            }
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(slot) = self.active_slot.as_mut() {
            slot.file.flush()
        } else {
            Ok(())
        }
    }
}

impl Drop for SplitVolumeWriter {
    fn drop(&mut self) {
        if !self.is_closed {
            let _ = self.cancel_and_cleanup_in_txn(TTZipStatus::ErrCompressionFailed);
        }
    }
}

/// Unified single-file or single-pass multi-volume output sink for 7z archives.
pub enum SevenZOutputTarget {
    Single {
        writer: Option<BufWriter<File>>,
        abort_guard: Option<crate::fs::FileAbortGuard>,
        primary_status: TTZipStatus,
    },
    Split {
        writer: SplitVolumeWriter,
        committed: bool,
        primary_status: TTZipStatus,
    },
}

impl SevenZOutputTarget {
    /// Opens either a single buffered file or a single-pass `SplitVolumeWriter`.
    pub fn open(dest_path: &Path, split_volume_size_bytes: u64, buf_cap: usize) -> io::Result<Self> {
        Self::open_in_txn(dest_path, split_volume_size_bytes, buf_cap, None)
    }

    /// Opens a single buffered file or `SplitVolumeWriter` bound to `CreateTransactionContext`.
    pub fn open_in_txn(
        dest_path: &Path,
        split_volume_size_bytes: u64,
        buf_cap: usize,
        txn_ctx: Option<&CreateTransactionContext>,
    ) -> io::Result<Self> {
        if let Some(parent) = dest_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        if split_volume_size_bytes > 0 {
            let base_path = if dest_path.extension().and_then(|e| e.to_str()) == Some("001") {
                dest_path.with_extension("")
            } else {
                dest_path.to_path_buf()
            };
            let writer = SplitVolumeWriter::new_in_txn(
                &base_path,
                split_volume_size_bytes,
                VolumeNamingScheme::NumberedExtension,
                txn_ctx,
            )?;
            Ok(Self::Split {
                writer,
                committed: false,
                primary_status: TTZipStatus::ErrCompressionFailed,
            })
        } else {
            let (raw_file, abort_guard) = crate::fs::FileAbortGuard::create_staged_file_in_txn(
                dest_path,
                CreateFileAccessMode::WriteOnly,
                txn_ctx,
                ResidualPathRole::StagedArtifact,
            )?;
            Ok(Self::Single {
                writer: Some(BufWriter::with_capacity(buf_cap, raw_file)),
                abort_guard: Some(abort_guard),
                primary_status: TTZipStatus::ErrCompressionFailed,
            })
        }
    }

    /// Explicitly aborts and settles the 7z output target with `primary_status`.
    pub fn abort_in_txn(&mut self, primary_status: TTZipStatus) {
        match self {
            Self::Single {
                writer,
                abort_guard,
                primary_status: st,
            } => {
                *st = primary_status;
                if let (Some(buf_writer), Some(guard)) = (writer.take(), abort_guard.take()) {
                    let _ = guard.finish_buf_writer_and_settle_in_txn(
                        buf_writer,
                        Err((
                            primary_status,
                            format!("7z writer aborted with {:?}", primary_status),
                        )),
                        FileFinishSyncPolicy::None,
                    );
                }
            }
            Self::Split {
                writer,
                committed,
                primary_status: st,
            } => {
                *st = primary_status;
                if !*committed {
                    *committed = true;
                    let _ = writer.cancel_and_cleanup_in_txn(primary_status);
                }
            }
        }
    }

    /// Patches the 32-byte 7z signature header at offset 0 and settles all handles via B1 proof.
    pub fn finalize_signature_in_txn(mut self, sig_bytes: &[u8; 32]) -> Result<(), TTZipStatus> {
        match &mut self {
            Self::Single {
                writer,
                abort_guard,
                ..
            } => {
                let mut buf_writer = writer.take().ok_or(TTZipStatus::ErrCompressionFailed)?;
                let guard = abort_guard.take().ok_or(TTZipStatus::ErrCompressionFailed)?;
                let seek_write_res = buf_writer
                    .seek(SeekFrom::Start(0))
                    .and_then(|_| buf_writer.write_all(sig_bytes))
                    .map_err(|e| {
                        (
                            TTZipStatus::ErrCompressionFailed,
                            format!("7z signature seek/write failed: {e}"),
                        )
                    });
                guard
                    .finish_buf_writer_and_settle_in_txn(
                        buf_writer,
                        seek_write_res,
                        FileFinishSyncPolicy::None,
                    )
                    .map(|_| ())
                    .map_err(|e| e.primary_status)
            }
            Self::Split {
                writer, committed, ..
            } => {
                if writer.write_header_at_start(sig_bytes).is_err() {
                    *committed = true;
                    let _ = writer.cancel_and_cleanup_in_txn(TTZipStatus::ErrCompressionFailed);
                    return Err(TTZipStatus::ErrCompressionFailed);
                }
                *committed = true;
                writer
                    .close_in_txn()
                    .map(|_| ())
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)
            }
        }
    }

    /// Patches the 32-byte 7z signature header at offset 0 and commits the output.
    pub fn finalize_signature(self, sig_bytes: &[u8; 32]) -> io::Result<()> {
        self.finalize_signature_in_txn(sig_bytes)
            .map_err(|_| io::Error::other("7z signature finalization failed"))
    }
}

impl Write for SevenZOutputTarget {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Single { writer, .. } => writer
                .as_mut()
                .ok_or_else(|| io::Error::other("7z single writer already closed"))?
                .write(buf),
            Self::Split { writer, .. } => writer.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Single { writer, .. } => writer
                .as_mut()
                .ok_or_else(|| io::Error::other("7z single writer already closed"))?
                .flush(),
            Self::Split { writer, .. } => writer.flush(),
        }
    }
}

impl Drop for SevenZOutputTarget {
    fn drop(&mut self) {
        let status = match self {
            Self::Single { primary_status, .. } | Self::Split { primary_status, .. } => {
                *primary_status
            }
        };
        self.abort_in_txn(status);
    }
}

/// Unified single-file or single-pass multi-volume streaming output sink for sequential archive formats.
pub enum StreamOutputTarget {
    Single {
        file: Option<File>,
        abort_guard: Option<crate::fs::FileAbortGuard>,
        primary_status: TTZipStatus,
    },
    Split {
        writer: SplitVolumeWriter,
        committed: bool,
        primary_status: TTZipStatus,
    },
}

impl StreamOutputTarget {
    /// Opens either a single output file or a single-pass `SplitVolumeWriter` using numbered `.001` volumes.
    pub fn open(dest_path: &Path, split_volume_size_bytes: u64) -> io::Result<Self> {
        Self::open_in_txn(dest_path, split_volume_size_bytes, None)
    }

    /// Opens a single output file or `SplitVolumeWriter` bound to `CreateTransactionContext`.
    pub fn open_in_txn(
        dest_path: &Path,
        split_volume_size_bytes: u64,
        txn_ctx: Option<&CreateTransactionContext>,
    ) -> io::Result<Self> {
        if let Some(parent) = dest_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        if split_volume_size_bytes > 0 {
            let base_path = if dest_path.extension().and_then(|e| e.to_str()) == Some("001") {
                dest_path.with_extension("")
            } else {
                dest_path.to_path_buf()
            };
            let writer = SplitVolumeWriter::new_in_txn(
                &base_path,
                split_volume_size_bytes,
                VolumeNamingScheme::NumberedExtension,
                txn_ctx,
            )?;
            Ok(Self::Split {
                writer,
                committed: false,
                primary_status: TTZipStatus::ErrCompressionFailed,
            })
        } else {
            let (file, abort_guard) = crate::fs::FileAbortGuard::create_staged_file_in_txn(
                dest_path,
                CreateFileAccessMode::WriteOnly,
                txn_ctx,
                ResidualPathRole::StagedArtifact,
            )?;
            Ok(Self::Single {
                file: Some(file),
                abort_guard: Some(abort_guard),
                primary_status: TTZipStatus::ErrCompressionFailed,
            })
        }
    }

    /// Explicitly finishes and settles the output target with `prior_write_res`.
    pub fn finish_and_settle_in_txn(
        mut self,
        prior_write_res: Result<(), (TTZipStatus, String)>,
    ) -> Result<(), TTZipStatus> {
        match &mut self {
            Self::Single {
                file, abort_guard, ..
            } => {
                let raw_file = file.take().ok_or(TTZipStatus::ErrCompressionFailed)?;
                let guard = abort_guard.take().ok_or(TTZipStatus::ErrCompressionFailed)?;
                guard
                    .finish_file_and_settle_in_txn(
                        raw_file,
                        prior_write_res,
                        None,
                        FileFinishSyncPolicy::None,
                    )
                    .map(|_| ())
                    .map_err(|e| e.primary_status)
            }
            Self::Split {
                writer, committed, ..
            } => {
                *committed = true;
                if let Err((status, _)) = prior_write_res {
                    let _ = writer.cancel_and_cleanup_in_txn(status);
                    return Err(status);
                }
                writer
                    .close_in_txn()
                    .map(|_| ())
                    .map_err(|_| TTZipStatus::ErrCompressionFailed)
            }
        }
    }

    /// Aborts the stream output target with `primary_status`.
    pub fn abort_in_txn(self, primary_status: TTZipStatus) {
        let _ = self.finish_and_settle_in_txn(Err((
            primary_status,
            format!("stream target aborted with {:?}", primary_status),
        )));
    }

    /// Flushes and commits the output target, preventing rollback on drop.
    pub fn commit(self) -> io::Result<()> {
        self.finish_and_settle_in_txn(Ok(()))
            .map_err(|_| io::Error::other("stream output target commit failed"))
    }
}

impl Write for StreamOutputTarget {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Single { file, .. } => file
                .as_mut()
                .ok_or_else(|| io::Error::other("stream single file already closed"))?
                .write(buf),
            Self::Split { writer, .. } => writer.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Single { file, .. } => file
                .as_mut()
                .ok_or_else(|| io::Error::other("stream single file already closed"))?
                .flush(),
            Self::Split { writer, .. } => writer.flush(),
        }
    }
}

impl Drop for StreamOutputTarget {
    fn drop(&mut self) {
        match self {
            Self::Single {
                file,
                abort_guard,
                primary_status,
            } => {
                if let (Some(raw_file), Some(guard)) = (file.take(), abort_guard.take()) {
                    let _ = guard.finish_file_and_settle_in_txn(
                        raw_file,
                        Err((*primary_status, "stream target dropped before commit".to_string())),
                        None,
                        FileFinishSyncPolicy::None,
                    );
                }
            }
            Self::Split {
                writer,
                committed,
                primary_status,
            } => {
                if !*committed {
                    *committed = true;
                    let _ = writer.cancel_and_cleanup_in_txn(*primary_status);
                }
            }
        }
    }
}
