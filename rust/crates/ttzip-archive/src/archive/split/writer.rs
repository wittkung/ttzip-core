// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Multi-volume split archive writer with byte-level accuracy and volume rotation.

use super::VolumeNamingScheme;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Computes volume file path for the given 1-based index according to the naming scheme.
pub fn compute_volume_path(
    base_path: &Path,
    index: usize,
    naming_scheme: VolumeNamingScheme,
) -> PathBuf {
    match naming_scheme {
        VolumeNamingScheme::NumberedExtension => {
            // Standard: archive.7z.001, archive.zip.001, archive.tar.001
            PathBuf::from(format!("{}.{:03}", base_path.display(), index))
        }
        VolumeNamingScheme::PkzipSpanned => {
            // PKZIP standard: archive.z01, archive.z02, ... archive.zip (final part renamed on close)
            let parent = base_path.parent().unwrap_or_else(|| Path::new(""));
            let stem = base_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("archive");
            parent.join(format!("{}.z{:02}", stem, index))
        }
        VolumeNamingScheme::RawSplit => {
            // Raw split: archive.001, archive.002
            PathBuf::from(format!("{}.{:03}", base_path.display(), index))
        }
    }
}

/// Zero-copy multi-volume split archive writer sink.
///
/// Intercepts archive byte streams in real time at byte-level accuracy, seamlessly closing
/// the active volume and opening subsequent volume files without intermediate memory copies.
pub struct SplitVolumeWriter {
    base_path: PathBuf,
    volume_size_bytes: u64,
    naming_scheme: VolumeNamingScheme,
    clean_on_failure: bool,
    current_volume_index: usize,
    bytes_written_in_current: u64,
    total_bytes_written: u64,
    active_file: Option<File>,
    generated_volume_paths: Vec<PathBuf>,
    is_closed: bool,
}

impl SplitVolumeWriter {
    /// Creates a new `SplitVolumeWriter` targeting `base_path`.
    pub fn new(
        base_path: impl AsRef<Path>,
        volume_size_bytes: u64,
        naming_scheme: VolumeNamingScheme,
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
            active_file: None,
            generated_volume_paths: Vec::new(),
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
        if path.exists() {
            let _ = fs::remove_file(&path);
        }

        let file = File::create(&path)?;
        self.active_file = Some(file);
        self.current_volume_index = index;
        self.bytes_written_in_current = 0;
        self.generated_volume_paths.push(path);
        Ok(())
    }

    /// Rotates from the active volume to the next volume index.
    fn rotate_to_next_volume(&mut self) -> io::Result<()> {
        if let Some(mut file) = self.active_file.take() {
            file.flush()?;
        }
        self.open_volume(self.current_volume_index + 1)
    }

    /// Patches the header bytes at offset 0 of the first volume (`.001`).
    pub fn write_header_at_start(&mut self, header: &[u8]) -> io::Result<()> {
        if self.current_volume_index == 1 {
            let file = self.active_file.as_mut().ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "No active volume file")
            })?;
            file.flush()?;
            file.seek(SeekFrom::Start(0))?;
            file.write_all(header)?;
            file.seek(SeekFrom::Start(self.bytes_written_in_current))?;
            file.flush()
        } else {
            if let Some(file) = self.active_file.as_mut() {
                file.flush()?;
            }
            let first_path = self.generated_volume_paths.first().ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "Missing first volume path")
            })?;
            let mut first_file = OpenOptions::new().write(true).open(first_path)?;
            first_file.seek(SeekFrom::Start(0))?;
            first_file.write_all(header)?;
            first_file.flush()
        }
    }

    /// Flushes and closes all volume handles, applying final volume naming conventions.
    pub fn close(&mut self) -> io::Result<Vec<PathBuf>> {
        if self.is_closed {
            return Ok(self.generated_volume_paths.clone());
        }
        self.is_closed = true;

        if let Some(mut file) = self.active_file.take() {
            file.flush()?;
        }

        // For PKZIP spanned format, rename the last volume to base_path (e.g. archive.zip)
        if self.naming_scheme == VolumeNamingScheme::PkzipSpanned
            && !self.generated_volume_paths.is_empty()
        {
            let last_path = self.generated_volume_paths.pop().unwrap();
            let final_zip_path = self.base_path.clone();
            if last_path != final_zip_path {
                if final_zip_path.exists() {
                    let _ = fs::remove_file(&final_zip_path);
                }
                fs::rename(&last_path, &final_zip_path)?;
            }
            self.generated_volume_paths.push(final_zip_path);
        }

        Ok(self.generated_volume_paths.clone())
    }

    /// Purges all generated volumes in the event of an archive failure.
    pub fn cancel_and_cleanup(&mut self) {
        self.is_closed = true;
        self.active_file = None;

        if self.clean_on_failure {
            for path in &self.generated_volume_paths {
                let _ = fs::remove_file(path);
            }
        }
        self.generated_volume_paths.clear();
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
                let file = self.active_file.as_mut().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, "No active volume file")
                })?;
                file.write_all(&buf[current_offset..current_offset + to_write])?;
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
        if let Some(file) = self.active_file.as_mut() {
            file.flush()
        } else {
            Ok(())
        }
    }
}

impl Drop for SplitVolumeWriter {
    fn drop(&mut self) {
        if !self.is_closed {
            let _ = self.close();
        }
    }
}

/// Unified single-file or single-pass multi-volume output sink for 7z archives.
pub enum SevenZOutputTarget {
    Single {
        writer: BufWriter<File>,
        abort_guard: Option<crate::fs::FileAbortGuard>,
    },
    Split {
        writer: SplitVolumeWriter,
        committed: bool,
    },
}

impl SevenZOutputTarget {
    /// Opens either a single buffered file or a single-pass `SplitVolumeWriter`.
    pub fn open(dest_path: &Path, split_volume_size_bytes: u64, buf_cap: usize) -> io::Result<Self> {
        if let Some(parent) = dest_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        if split_volume_size_bytes > 0 {
            let base_path = if dest_path.extension().and_then(|e| e.to_str()) == Some("001") {
                dest_path.with_extension("")
            } else {
                dest_path.to_path_buf()
            };
            let writer = SplitVolumeWriter::new(
                &base_path,
                split_volume_size_bytes,
                VolumeNamingScheme::NumberedExtension,
            )?;
            Ok(Self::Split {
                writer,
                committed: false,
            })
        } else {
            let abort_guard = crate::fs::FileAbortGuard::new(dest_path);
            let raw_file = File::create(dest_path)?;
            Ok(Self::Single {
                writer: BufWriter::with_capacity(buf_cap, raw_file),
                abort_guard: Some(abort_guard),
            })
        }
    }

    /// Patches the 32-byte 7z signature header at offset 0 and commits the output.
    pub fn finalize_signature(mut self, sig_bytes: &[u8; 32]) -> io::Result<()> {
        match &mut self {
            Self::Single { writer, abort_guard } => {
                writer.seek(SeekFrom::Start(0))?;
                writer.write_all(sig_bytes)?;
                writer.flush()?;
                if let Some(guard) = abort_guard.take() {
                    guard.commit();
                }
                Ok(())
            }
            Self::Split { writer, committed } => {
                writer.write_header_at_start(sig_bytes)?;
                writer.close()?;
                *committed = true;
                Ok(())
            }
        }
    }
}

impl Write for SevenZOutputTarget {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Single { writer, .. } => writer.write(buf),
            Self::Split { writer, .. } => writer.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Single { writer, .. } => writer.flush(),
            Self::Split { writer, .. } => writer.flush(),
        }
    }
}

impl Drop for SevenZOutputTarget {
    fn drop(&mut self) {
        if let Self::Split { writer, committed } = self {
            if !*committed {
                writer.cancel_and_cleanup();
            }
        }
    }
}

/// Unified single-file or single-pass multi-volume streaming output sink for sequential archive formats.
pub enum StreamOutputTarget {
    Single {
        file: File,
        abort_guard: Option<crate::fs::FileAbortGuard>,
    },
    Split {
        writer: SplitVolumeWriter,
        committed: bool,
    },
}

impl StreamOutputTarget {
    /// Opens either a single output file or a single-pass `SplitVolumeWriter` using numbered `.001` volumes.
    pub fn open(dest_path: &Path, split_volume_size_bytes: u64) -> io::Result<Self> {
        if let Some(parent) = dest_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        if split_volume_size_bytes > 0 {
            let base_path = if dest_path.extension().and_then(|e| e.to_str()) == Some("001") {
                dest_path.with_extension("")
            } else {
                dest_path.to_path_buf()
            };
            let writer = SplitVolumeWriter::new(
                &base_path,
                split_volume_size_bytes,
                VolumeNamingScheme::NumberedExtension,
            )?;
            Ok(Self::Split {
                writer,
                committed: false,
            })
        } else {
            let abort_guard = crate::fs::FileAbortGuard::new(dest_path);
            let file = File::create(dest_path)?;
            Ok(Self::Single {
                file,
                abort_guard: Some(abort_guard),
            })
        }
    }

    /// Flushes and commits the output target, preventing rollback on drop.
    pub fn commit(mut self) -> io::Result<()> {
        match &mut self {
            Self::Single { file, abort_guard } => {
                file.flush()?;
                if let Some(guard) = abort_guard.take() {
                    guard.commit();
                }
                Ok(())
            }
            Self::Split { writer, committed } => {
                writer.close()?;
                *committed = true;
                Ok(())
            }
        }
    }
}

impl Write for StreamOutputTarget {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Single { file, .. } => file.write(buf),
            Self::Split { writer, .. } => writer.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Single { file, .. } => file.flush(),
            Self::Split { writer, .. } => writer.flush(),
        }
    }
}

impl Drop for StreamOutputTarget {
    fn drop(&mut self) {
        if let Self::Split { writer, committed } = self {
            if !*committed {
                writer.cancel_and_cleanup();
            }
        }
    }
}

