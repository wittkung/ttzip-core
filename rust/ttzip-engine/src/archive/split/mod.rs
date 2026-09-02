// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Multi-volume split container and virtual continuous reader module.

mod reader;
mod writer;

pub use reader::{detect_volume_chain, VirtualMultiVolumeReader, VolumeSegment};
pub use writer::{compute_volume_path, SplitVolumeWriter};

/// Naming convention for multi-volume archive segments.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum VolumeNamingScheme {
    /// Numbered extensions: archive.7z.001, archive.zip.001, archive.tar.001
    NumberedExtension = 0,
    /// PKZIP spanned standard: archive.z01, archive.z02, ... archive.zip
    PkzipSpanned = 1,
    /// Raw split: archive.001, archive.002
    RawSplit = 2,
}

impl From<i32> for VolumeNamingScheme {
    fn from(val: i32) -> Self {
        match val {
            1 => VolumeNamingScheme::PkzipSpanned,
            2 => VolumeNamingScheme::RawSplit,
            _ => VolumeNamingScheme::NumberedExtension,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn test_split_writer_boundary_rollover_and_precise_counts() {
        let dir = tempdir().unwrap();
        let base_path = dir.path().join("archive.7z");
        let volume_size = 1000; // 1000 bytes per volume

        let mut writer = SplitVolumeWriter::new(
            &base_path,
            volume_size,
            VolumeNamingScheme::NumberedExtension,
        )
        .unwrap();

        // 2500 bytes payload -> 3 volumes: 1000, 1000, 500
        let payload = vec![0xABu8; 2500];
        writer.write_all(&payload).unwrap();
        let volumes = writer.close().unwrap();

        assert_eq!(volumes.len(), 3);
        assert_eq!(writer.total_bytes(), 2500);

        assert_eq!(fs::metadata(&volumes[0]).unwrap().len(), 1000);
        assert_eq!(fs::metadata(&volumes[1]).unwrap().len(), 1000);
        assert_eq!(fs::metadata(&volumes[2]).unwrap().len(), 500);
    }

    #[test]
    fn test_split_writer_pkzip_spanned_naming() {
        let dir = tempdir().unwrap();
        let base_path = dir.path().join("archive.zip");
        let volume_size = 1000;

        let mut writer =
            SplitVolumeWriter::new(&base_path, volume_size, VolumeNamingScheme::PkzipSpanned)
                .unwrap();

        let payload = vec![0x33u8; 2400]; // 1000 (.z01), 1000 (.z02), 400 (.zip)
        writer.write_all(&payload).unwrap();
        let volumes = writer.close().unwrap();

        assert_eq!(volumes.len(), 3);
        assert!(volumes[0].to_string_lossy().ends_with(".z01"));
        assert!(volumes[1].to_string_lossy().ends_with(".z02"));
        assert!(volumes[2].to_string_lossy().ends_with(".zip"));

        assert_eq!(fs::metadata(&volumes[0]).unwrap().len(), 1000);
        assert_eq!(fs::metadata(&volumes[1]).unwrap().len(), 1000);
        assert_eq!(fs::metadata(&volumes[2]).unwrap().len(), 400);
    }

    #[test]
    fn test_virtual_multi_volume_reader_topology_detection_and_seeking() {
        let dir = tempdir().unwrap();
        let base_path = dir.path().join("data.tar");
        let volume_size = 512;

        let mut writer =
            SplitVolumeWriter::new(&base_path, volume_size, VolumeNamingScheme::NumberedExtension)
                .unwrap();

        let mut sample_data = Vec::with_capacity(1500);
        for i in 0..1500 {
            sample_data.push((i % 251) as u8);
        }
        writer.write_all(&sample_data).unwrap();
        let volumes = writer.close().unwrap();
        assert_eq!(volumes.len(), 3); // 512, 512, 476

        // Test opening from the middle volume (.002)
        let middle_vol = &volumes[1];
        let mut reader = VirtualMultiVolumeReader::open_from_any_volume(middle_vol).unwrap();

        assert_eq!(reader.total_size(), 1500);
        assert_eq!(reader.volume_paths().len(), 3);

        // Read all sequentially
        let mut read_buf = Vec::new();
        reader.read_to_end(&mut read_buf).unwrap();
        assert_eq!(read_buf, sample_data);

        // Test seeking across volume boundaries
        reader.seek(SeekFrom::Start(600)).unwrap(); // in volume 2
        let mut small_buf = [0u8; 100];
        reader.read_exact(&mut small_buf).unwrap();
        assert_eq!(&small_buf[..], &sample_data[600..700]);

        // Seek to volume 3
        reader.seek(SeekFrom::Start(1100)).unwrap();
        let mut end_buf = [0u8; 400];
        reader.read_exact(&mut end_buf).unwrap();
        assert_eq!(&end_buf[..], &sample_data[1100..1500]);
    }

    #[test]
    fn test_detect_volume_chain_part_n_rar_format() {
        let dir = tempdir().unwrap();
        let part1 = dir.path().join("backup.part1.rar");
        let part2 = dir.path().join("backup.part2.rar");
        let part3 = dir.path().join("backup.part3.rar");

        fs::write(&part1, b"PART1_BYTES").unwrap();
        fs::write(&part2, b"PART2_BYTES").unwrap();
        fs::write(&part3, b"PART3_BYTES").unwrap();

        // Detect from part 2 seed
        let chain = detect_volume_chain(&part2).unwrap();
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0], part1);
        assert_eq!(chain[1], part2);
        assert_eq!(chain[2], part3);

        // Virtual reader stitches all 3 parts
        let mut vreader = VirtualMultiVolumeReader::from_volumes(chain).unwrap();
        let mut combined = String::new();
        vreader.read_to_string(&mut combined).unwrap();
        assert_eq!(combined, "PART1_BYTESPART2_BYTESPART3_BYTES");
    }

    #[test]
    fn test_detect_volume_chain_base_zero_and_pkzip_chain() {
        let dir = tempdir().unwrap();
        // Base-0 numbered extension: data.7z.000, data.7z.001, data.7z.002
        let z0 = dir.path().join("data.7z.000");
        let z1 = dir.path().join("data.7z.001");
        let z2 = dir.path().join("data.7z.002");

        fs::write(&z0, b"ZERO").unwrap();
        fs::write(&z1, b"ONE_").unwrap();
        fs::write(&z2, b"TWO_").unwrap();

        let chain = detect_volume_chain(&z1).unwrap();
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0], z0);
        assert_eq!(chain[1], z1);
        assert_eq!(chain[2], z2);

        // PKZIP spanned: pack.z01, pack.z02, pack.zip
        let p_z01 = dir.path().join("pack.z01");
        let p_z02 = dir.path().join("pack.z02");
        let p_zip = dir.path().join("pack.zip");

        fs::write(&p_z01, b"SEG1_").unwrap();
        fs::write(&p_z02, b"SEG2_").unwrap();
        fs::write(&p_zip, b"FINAL").unwrap();

        let pkzip_chain = detect_volume_chain(&p_zip).unwrap();
        assert_eq!(pkzip_chain.len(), 3);
        assert_eq!(pkzip_chain[0], p_z01);
        assert_eq!(pkzip_chain[1], p_z02);
        assert_eq!(pkzip_chain[2], p_zip);

        let mut pk_reader = VirtualMultiVolumeReader::from_volumes(pkzip_chain).unwrap();
        let mut pk_combined = String::new();
        pk_reader.read_to_string(&mut pk_combined).unwrap();
        assert_eq!(pk_combined, "SEG1_SEG2_FINAL");
    }

    #[test]
    fn test_detect_volume_chain_non_existent_seed_returns_error() {
        let non_existent = Path::new("/tmp/definitely_not_existing_volume_file.7z.001");
        let res = detect_volume_chain(non_existent);
        assert!(res.is_err());
    }

    #[test]
    fn test_virtual_multi_volume_reader_seek_from_current_and_end() {
        let dir = tempdir().unwrap();
        let p1 = dir.path().join("stream.001");
        let p2 = dir.path().join("stream.002");
        let p3 = dir.path().join("stream.003");

        fs::write(&p1, b"0123456789").unwrap(); // 10 bytes
        fs::write(&p2, b"ABCDEFGHIJ").unwrap(); // 10 bytes
        fs::write(&p3, b"KLMNOPQRST").unwrap(); // 10 bytes

        let mut reader = VirtualMultiVolumeReader::from_volumes(vec![p1, p2, p3]).unwrap();
        assert_eq!(reader.total_size(), 30);

        // SeekFrom::End(-5) -> offset 25
        reader.seek(SeekFrom::End(-5)).unwrap();
        let mut buf = [0u8; 5];
        reader.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"PQRST");

        // SeekFrom::Current(-15) from offset 30 -> offset 15 (in volume 2)
        reader.seek(SeekFrom::Current(-15)).unwrap();
        let mut buf2 = [0u8; 5];
        reader.read_exact(&mut buf2).unwrap();
        assert_eq!(&buf2, b"FGHIJ");

        // Seeking to negative offset should error
        let err = reader.seek(SeekFrom::Current(-50));
        assert!(err.is_err());
    }
}

