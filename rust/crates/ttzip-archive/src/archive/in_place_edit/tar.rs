// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! POSIX TAR in-place atomic archive mutation engine with 512-byte aligned block overwrites and appends.

use super::{InPlaceAction, PartitionedActions};
use crate::archive::tar::header::TAR_BLOCK_SIZE;
use crate::archive::tar::scanner::{TarEntry, TarSeekScanner};
use crate::archive::tar::writer::TarWriter;
use crate::types::{record_execution_provenance, TTZipEngineTag, TTZipExecutionProvenance, TTZipStatus};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

struct CountingWriter<W> {
    inner: W,
    bytes_written: u64,
}

impl<W: Write> Write for CountingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.bytes_written += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Modifies a POSIX TAR archive in-place with 512-byte aligned block overwrites and appends.
pub fn in_place_edit_tar(
    archive_path: &Path,
    shadow_path: &Path,
    actions: &[InPlaceAction],
) -> Result<(), TTZipStatus> {
    let source = crate::archive::source::open_archive_source(archive_path)?;
    let mapped = source.as_slice().ok_or(TTZipStatus::ErrOpenFailed)?;

    if let Some(parent) = shadow_path.parent().filter(|p| !p.exists()) {
        let _ = fs::create_dir_all(parent);
    }
    let shadow_file = File::create(shadow_path).map_err(|_| TTZipStatus::ErrOpenFailed)?;
    let mut writer = BufWriter::with_capacity(64 * 1024, shadow_file);
    let total_len = mutate_tar_to_writer(mapped, actions, &mut writer)?;
    writer.flush().map_err(|_| TTZipStatus::ErrOpenFailed)?;

    let prov = TTZipExecutionProvenance {
        engine_tag: TTZipEngineTag::RustTarStreamEngine,
        compressed_bytes: total_len,
        is_fallback: false,
        ..Default::default()
    };
    record_execution_provenance(prov);

    Ok(())
}

pub(crate) fn mutate_tar_to_writer<W: Write>(
    mapped: &[u8],
    actions: &[InPlaceAction],
    out: &mut W,
) -> Result<u64, TTZipStatus> {
    let mut scanner = TarSeekScanner::new(mapped);
    let entries = scanner.scan_all()?;
    let plan = PartitionedActions::from_slice(actions);

    let mut cw = CountingWriter {
        inner: out,
        bytes_written: 0,
    };
    let mut processed_replaced: HashSet<&str> = HashSet::new();

    for entry in &entries {
        let key = entry.path.trim_start_matches('/');
        if plan.deleted.contains(key) {
            continue;
        }

        if let Some((plan_key, src_path)) = plan.replaced.get_key_value(key) {
            processed_replaced.insert(plan_key.as_str());
            write_tar_entry_from_file(&mut cw, &entry.path, src_path)?;
        } else {
            copy_raw_tar_entry(&mut cw, mapped, entry)?;
        }
    }

    for (key, src_path) in &plan.replaced {
        if !processed_replaced.contains(key.as_str()) {
            write_tar_entry_from_file(&mut cw, key, src_path)?;
        }
    }

    for (rel_path, src_path) in &plan.appended {
        write_tar_entry_from_file(&mut cw, rel_path, src_path)?;
    }

    cw.write_all(&[0u8; 1024])
        .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
    Ok(cw.bytes_written)
}

fn copy_raw_tar_entry<W: Write>(
    out: &mut W,
    mapped: &[u8],
    entry: &TarEntry<'_>,
) -> Result<(), TTZipStatus> {
    let block_start = entry.header_offset;
    let payload_size = entry.size as usize;
    let payload_blocks = payload_size.div_ceil(TAR_BLOCK_SIZE) * TAR_BLOCK_SIZE;
    let block_end = entry.data_offset + payload_blocks;
    if block_end <= mapped.len() && block_start <= block_end {
        for chunk in mapped[block_start..block_end].chunks(64 * 1024) {
            out.write_all(chunk)
                .map_err(|_| TTZipStatus::ErrCompressionFailed)?;
        }
    }
    Ok(())
}

pub(crate) fn write_tar_entry_from_file<W: Write>(
    out: &mut W,
    rel_path: &str,
    src_path: &Path,
) -> Result<(), TTZipStatus> {
    let meta = fs::symlink_metadata(src_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
    let is_dir = meta.is_dir();
    let mode = meta.permissions().mode();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let mut writer = TarWriter::new(out);
    if is_dir {
        writer.append_dir(rel_path, mode, mtime)?;
    } else {
        let mut file = File::open(src_path).map_err(|_| TTZipStatus::ErrFileNotFound)?;
        let file_len = file
            .metadata()
            .map_err(|_| TTZipStatus::ErrFileNotFound)?
            .len();
        writer.append_file_stream(rel_path, &mut file, file_len, mode, mtime)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::tar::reader::TarArchive;

    #[test]
    fn test_in_place_edit_tar_streaming_large_entries() {
        let dir = tempfile::tempdir().expect("tempdir");
        let f1 = dir.path().join("keep.bin");
        let f2 = dir.path().join("replace.bin");
        let payload1 = vec![0x41u8; 150 * 1024];
        let payload2 = vec![0x42u8; 150 * 1024];
        fs::write(&f1, &payload1).expect("write f1");
        fs::write(&f2, b"old").expect("write f2");

        let tar_path = dir.path().join("base.tar");
        let shadow_path = dir.path().join("shadow.tar");
        let mut init_bytes = Vec::new();
        write_tar_entry_from_file(&mut init_bytes, "keep.bin", &f1).expect("write keep");
        write_tar_entry_from_file(&mut init_bytes, "replace.bin", &f2).expect("write replace");
        init_bytes.extend_from_slice(&[0u8; 1024]);
        fs::write(&tar_path, &init_bytes).expect("write tar");

        fs::write(&f2, &payload2).expect("write new f2");
        let actions = vec![InPlaceAction::Replace {
            entry_path: "/replace.bin".to_string(),
            source_path: f2,
        }];
        in_place_edit_tar(&tar_path, &shadow_path, &actions).expect("in_place_edit_tar");

        let out_bytes = fs::read(&shadow_path).expect("read shadow");
        let archive = TarArchive::open_slice(&out_bytes).expect("open tar");
        assert_eq!(archive.len(), 2);
        assert_eq!(archive.extract_entry_bytes(0).expect("e0"), payload1);
        assert_eq!(archive.extract_entry_bytes(1).expect("e1"), payload2);
    }

    #[cfg(unix)]
    #[test]
    fn test_write_tar_entry_from_symlink_uses_target_file_length() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("t.bin");
        let payload = vec![0x7Au8; 4096];
        fs::write(&target, &payload).expect("write target");

        let link = dir.path().join("symlink_entry.bin");
        std::os::unix::fs::symlink(&target, &link).expect("create symlink");

        let mut tar_bytes = Vec::new();
        write_tar_entry_from_file(&mut tar_bytes, "symlink_entry.bin", &link)
            .expect("write_tar_entry_from_file should succeed for symlink to file");
        tar_bytes.extend_from_slice(&[0u8; 1024]);

        let archive = TarArchive::open_slice(&tar_bytes).expect("valid tar archive");
        assert_eq!(archive.len(), 1);
        assert_eq!(
            archive.extract_entry_bytes(0).expect("extract entry"),
            payload
        );
    }
}
