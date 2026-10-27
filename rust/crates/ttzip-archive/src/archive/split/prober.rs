// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! PreFlightVolumeProber: Topology discovery and missing volume hole detection.
//!
//! Scans physical volume naming schemes (Numbered, PKZIP, Part) and computes
//! the missing volume difference set: M = E \ S without entering decompression pipelines.

use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Severity classification for missing split volumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingVolumeSeverity {
    /// Catalog/Central Directory is blocked (cannot list entries without missing volumes).
    CatalogBlocked,
    /// Metadata is readable, but decompression past the hole will fail.
    DataDegraded,
}

/// Metadata description for an individual physical volume segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumePartInfo {
    pub index: u32,
    pub path: String,
    pub file_size: u64,
}

/// Structured descriptor returned when a split volume archive has missing segments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingVolumeDescriptor {
    pub format: String,
    pub base_stem: String,
    pub seed_path: String,
    pub expected_total_volumes: Option<u32>,
    pub present_volumes: Vec<VolumePartInfo>,
    pub missing_indices: Vec<u32>,
    pub missing_filenames: Vec<String>,
    pub severity: MissingVolumeSeverity,
    pub recovery_hint: String,
}

/// Pre-flight physical volume topology prober.
pub struct PreFlightVolumeProber;

impl PreFlightVolumeProber {
    /// Non-blocking probe inspecting directory topology around `seed_path`.
    pub fn probe(seed_path: &Path) -> Option<MissingVolumeDescriptor> {
        if !seed_path.exists() {
            return None;
        }

        let file_name = seed_path.file_name()?.to_str()?;
        let parent_dir = seed_path.parent().unwrap_or_else(|| Path::new("."));

        // Case 1: Numbered extension (.001, .002, etc.)
        if let Some(desc) = Self::probe_numbered_extension(seed_path, file_name, parent_dir) {
            return Some(desc);
        }

        // Case 2: PKZIP spanned (.z01, .z02, ... and .zip)
        if let Some(desc) = Self::probe_pkzip_spanned(seed_path, file_name, parent_dir) {
            return Some(desc);
        }

        // Case 3: Part pattern (.part1.rar, .part01.rar, .part1.7z)
        if let Some(desc) = Self::probe_part_pattern(seed_path, file_name, parent_dir) {
            return Some(desc);
        }

        None
    }

    fn probe_numbered_extension(
        seed_path: &Path,
        file_name: &str,
        parent_dir: &Path,
    ) -> Option<MissingVolumeDescriptor> {
        let dot_pos = file_name.rfind('.')?;
        let ext = &file_name[dot_pos + 1..];
        if ext.len() < 3 || !ext.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }

        let prefix = &file_name[..dot_pos];
        let width = ext.len();

        let mut present: Vec<(u32, PathBuf, u64)> = Vec::new();
        if let Ok(entries) = fs::read_dir(parent_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = match name.to_str() {
                    Some(s) => s,
                    None => continue,
                };
                if let Some(p) = name_str.rfind('.') {
                    let p_prefix = &name_str[..p];
                    let p_ext = &name_str[p + 1..];
                    if p_prefix == prefix
                        && p_ext.len() == width
                        && p_ext.chars().all(|c| c.is_ascii_digit())
                    {
                        if let Ok(idx) = p_ext.parse::<u32>() {
                            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                            present.push((idx, entry.path(), size));
                        }
                    }
                }
            }
        }

        if present.is_empty() {
            return None;
        }

        present.sort_by_key(|p| p.0);
        let min_idx = present.first().map(|p| p.0).unwrap_or(1);
        let max_idx = present.last().map(|p| p.0).unwrap_or(1);

        let start_idx = if min_idx == 0 { 0 } else { 1 };
        let present_set: BTreeSet<u32> = present.iter().map(|p| p.0).collect();
        let mut missing_indices = Vec::new();
        let mut missing_filenames = Vec::new();

        for expected in start_idx..=max_idx {
            if !present_set.contains(&expected) {
                missing_indices.push(expected);
                missing_filenames.push(format!("{}.{:0width$}", prefix, expected, width = width));
            }
        }

        if missing_indices.is_empty() {
            return None;
        }

        let present_volumes = present
            .into_iter()
            .map(|(index, path, file_size)| VolumePartInfo {
                index,
                path: path.to_string_lossy().to_string(),
                file_size,
            })
            .collect();

        Some(MissingVolumeDescriptor {
            format: "NumberedExtension".to_string(),
            base_stem: prefix.to_string(),
            seed_path: seed_path.to_string_lossy().to_string(),
            expected_total_volumes: Some(max_idx - start_idx + 1),
            present_volumes,
            missing_indices,
            missing_filenames,
            severity: MissingVolumeSeverity::CatalogBlocked,
            recovery_hint: "Numbered split volume sequence contains missing segments".to_string(),
        })
    }

    fn probe_pkzip_spanned(
        seed_path: &Path,
        file_name: &str,
        parent_dir: &Path,
    ) -> Option<MissingVolumeDescriptor> {
        let dot_pos = file_name.rfind('.')?;
        let ext = &file_name[dot_pos + 1..];
        let is_z_part = ext.starts_with('z') || ext.starts_with('Z');
        let is_z_numbered =
            is_z_part && ext.len() >= 3 && ext[1..].chars().all(|c| c.is_ascii_digit());
        let is_zip_final = ext.eq_ignore_ascii_case("zip");

        if !is_z_numbered && !is_zip_final {
            return None;
        }

        // Fast-path: If the seed is a .zip file, check its EOCD record first.
        // Standalone (single-volume) ZIP files specify disk 0 and CD disk 0.
        // Bypasses parent directory scanning entirely for standard non-split archives.
        if is_zip_final {
            if let Ok(mut file) = fs::File::open(seed_path) {
                if let Ok(metadata) = file.metadata() {
                    let len = metadata.len();
                    let scan_len = (len as usize).min(1024);
                    if scan_len >= 22 && file.seek(SeekFrom::End(-(scan_len as i64))).is_ok() {
                        let mut buf = vec![0u8; scan_len];
                        if file.read_exact(&mut buf).is_ok() {
                            for i in (0..=(scan_len - 22)).rev() {
                                if &buf[i..i + 4] == b"PK\x05\x06" {
                                    let disk_num = u16::from_le_bytes([buf[i + 4], buf[i + 5]]);
                                    let cd_disk = u16::from_le_bytes([buf[i + 6], buf[i + 7]]);
                                    if disk_num == 0 && cd_disk == 0 {
                                        return None;
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        let stem = &file_name[..dot_pos];
        let mut present_z: Vec<(u32, PathBuf, u64)> = Vec::new();
        let mut zip_path: Option<(PathBuf, u64)> = None;

        if let Ok(entries) = fs::read_dir(parent_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = match name.to_str() {
                    Some(s) => s,
                    None => continue,
                };
                if let Some(p) = name_str.rfind('.') {
                    let p_stem = &name_str[..p];
                    let p_ext = &name_str[p + 1..];
                    if p_stem.eq_ignore_ascii_case(stem) {
                        if p_ext.eq_ignore_ascii_case("zip") {
                            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                            zip_path = Some((entry.path(), size));
                        } else if (p_ext.starts_with('z') || p_ext.starts_with('Z'))
                            && p_ext.len() >= 3
                            && p_ext[1..].chars().all(|c| c.is_ascii_digit())
                        {
                            if let Ok(idx) = p_ext[1..].parse::<u32>() {
                                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                                present_z.push((idx, entry.path(), size));
                            }
                        }
                    }
                }
            }
        }

        if present_z.is_empty() {
            if let Some((ref zp, len)) = zip_path {
                if let Ok(mut file) = fs::File::open(zp) {
                    let scan_len = (len as usize).min(1024);
                    if scan_len >= 22 && file.seek(SeekFrom::End(-(scan_len as i64))).is_ok() {
                        let mut buf = vec![0u8; scan_len];
                        if file.read_exact(&mut buf).is_ok() {
                            for i in 0..=(scan_len - 22) {
                                if &buf[i..i + 4] == b"PK\x05\x06" {
                                    let disk_num = u16::from_le_bytes([buf[i + 4], buf[i + 5]]);
                                    let cd_disk = u16::from_le_bytes([buf[i + 6], buf[i + 7]]);
                                    if (disk_num > 0 || cd_disk > 0)
                                        && disk_num != u16::MAX
                                        && cd_disk != u16::MAX
                                    {
                                        let total_vols = (disk_num as u32) + 1;
                                        let mut missing_indices = Vec::new();
                                        let mut missing_filenames = Vec::new();
                                        for d in 1..=disk_num as u32 {
                                            missing_indices.push(d);
                                            missing_filenames.push(format!("{}.z{:02}", stem, d));
                                        }
                                        return Some(MissingVolumeDescriptor {
                                            format: "PKZIP".to_string(),
                                            base_stem: stem.to_string(),
                                            seed_path: seed_path.to_string_lossy().to_string(),
                                            expected_total_volumes: Some(total_vols),
                                            present_volumes: vec![VolumePartInfo {
                                                index: total_vols,
                                                path: zp.to_string_lossy().to_string(),
                                                file_size: len,
                                            }],
                                            missing_indices,
                                            missing_filenames,
                                            severity: MissingVolumeSeverity::CatalogBlocked,
                                            recovery_hint: format!(
                                                "Archive spans {} disks; prior segments are missing",
                                                total_vols
                                            ),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
            return None;
        }

        present_z.sort_by_key(|p| p.0);
        let max_z = present_z.last().map(|p| p.0).unwrap_or(1);
        let present_set: BTreeSet<u32> = present_z.iter().map(|p| p.0).collect();

        let mut missing_indices = Vec::new();
        let mut missing_filenames = Vec::new();

        for i in 1..=max_z {
            if !present_set.contains(&i) {
                missing_indices.push(i);
                missing_filenames.push(format!("{}.z{:02}", stem, i));
            }
        }

        let final_zip_missing = zip_path.is_none();
        if final_zip_missing {
            missing_indices.push(max_z + 1);
            missing_filenames.push(format!("{}.zip", stem));
        }

        if missing_indices.is_empty() {
            return None;
        }

        let mut present_volumes: Vec<VolumePartInfo> = present_z
            .into_iter()
            .map(|(index, path, file_size)| VolumePartInfo {
                index,
                path: path.to_string_lossy().to_string(),
                file_size,
            })
            .collect();

        if let Some((zp, size)) = zip_path {
            present_volumes.push(VolumePartInfo {
                index: max_z + 1,
                path: zp.to_string_lossy().to_string(),
                file_size: size,
            });
        }

        Some(MissingVolumeDescriptor {
            format: "PKZIP".to_string(),
            base_stem: stem.to_string(),
            seed_path: seed_path.to_string_lossy().to_string(),
            expected_total_volumes: Some(max_z + 1),
            present_volumes,
            missing_indices,
            missing_filenames,
            severity: if final_zip_missing {
                MissingVolumeSeverity::CatalogBlocked
            } else {
                MissingVolumeSeverity::DataDegraded
            },
            recovery_hint: "PKZIP spanned archive missing segments".to_string(),
        })
    }

    fn probe_part_pattern(
        seed_path: &Path,
        file_name: &str,
        parent_dir: &Path,
    ) -> Option<MissingVolumeDescriptor> {
        let lower = file_name.to_lowercase();
        let part_pos = lower.find(".part")?;
        let after_part = &file_name[part_pos + 5..];
        let dot_after = after_part.find('.')?;
        let num_str = &after_part[..dot_after];
        let suffix = &after_part[dot_after..];

        if num_str.is_empty() || !num_str.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }

        let prefix = &file_name[..part_pos];
        let width = num_str.len();

        let mut present: Vec<(u32, PathBuf, u64)> = Vec::new();
        if let Ok(entries) = fs::read_dir(parent_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = match name.to_str() {
                    Some(s) => s,
                    None => continue,
                };
                let n_lower = name_str.to_lowercase();
                if let Some(pos) = n_lower.find(".part") {
                    let p_prefix = &name_str[..pos];
                    if p_prefix.eq_ignore_ascii_case(prefix) {
                        let aft = &name_str[pos + 5..];
                        if let Some(d_aft) = aft.find('.') {
                            let n_num = &aft[..d_aft];
                            let n_suf = &aft[d_aft..];
                            if n_suf.eq_ignore_ascii_case(suffix)
                                && n_num.chars().all(|c| c.is_ascii_digit())
                            {
                                if let Ok(idx) = n_num.parse::<u32>() {
                                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                                    present.push((idx, entry.path(), size));
                                }
                            }
                        }
                    }
                }
            }
        }

        if present.is_empty() {
            return None;
        }

        present.sort_by_key(|p| p.0);
        let max_idx = present.last().map(|p| p.0).unwrap_or(1);
        let present_set: BTreeSet<u32> = present.iter().map(|p| p.0).collect();

        let mut missing_indices = Vec::new();
        let mut missing_filenames = Vec::new();

        for expected in 1..=max_idx {
            if !present_set.contains(&expected) {
                missing_indices.push(expected);
                missing_filenames.push(format!(
                    "{}.part{:0width$}{}",
                    prefix,
                    expected,
                    suffix,
                    width = width
                ));
            }
        }

        if missing_indices.is_empty() {
            return None;
        }

        let present_volumes = present
            .into_iter()
            .map(|(index, path, file_size)| VolumePartInfo {
                index,
                path: path.to_string_lossy().to_string(),
                file_size,
            })
            .collect();

        let missing_count = missing_indices.len();
        Some(MissingVolumeDescriptor {
            format: "PartNumbered".to_string(),
            base_stem: prefix.to_string(),
            seed_path: seed_path.to_string_lossy().to_string(),
            expected_total_volumes: Some(max_idx),
            present_volumes,
            missing_indices,
            missing_filenames,
            severity: MissingVolumeSeverity::CatalogBlocked,
            recovery_hint: format!("Missing {} part(s) in part sequence", missing_count),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_probe_numbered_extension_detects_hole() {
        let dir = tempdir().unwrap();
        let p1 = dir.path().join("archive.7z.001");
        let p3 = dir.path().join("archive.7z.003");
        File::create(&p1).unwrap().write_all(b"part1").unwrap();
        File::create(&p3).unwrap().write_all(b"part3").unwrap();

        let desc = PreFlightVolumeProber::probe(&p1).expect("Should detect missing volume");
        assert_eq!(desc.format, "NumberedExtension");
        assert_eq!(desc.missing_indices, vec![2]);
        assert_eq!(desc.missing_filenames, vec!["archive.7z.002"]);
        assert_eq!(desc.severity, MissingVolumeSeverity::CatalogBlocked);
    }

    #[test]
    fn test_probe_numbered_extension_complete_returns_none() {
        let dir = tempdir().unwrap();
        let p1 = dir.path().join("archive.7z.001");
        let p2 = dir.path().join("archive.7z.002");
        File::create(&p1).unwrap().write_all(b"part1").unwrap();
        File::create(&p2).unwrap().write_all(b"part2").unwrap();

        assert!(PreFlightVolumeProber::probe(&p1).is_none());
    }

    #[test]
    fn test_probe_pkzip_spanned_missing_final_zip() {
        let dir = tempdir().unwrap();
        let z1 = dir.path().join("backup.z01");
        let z2 = dir.path().join("backup.z02");
        File::create(&z1).unwrap().write_all(b"z01").unwrap();
        File::create(&z2).unwrap().write_all(b"z02").unwrap();

        let desc = PreFlightVolumeProber::probe(&z1).expect("Should detect missing backup.zip");
        assert_eq!(desc.format, "PKZIP");
        assert_eq!(desc.missing_filenames, vec!["backup.zip"]);
        assert_eq!(desc.severity, MissingVolumeSeverity::CatalogBlocked);
    }

    #[test]
    fn test_probe_part_numbered_detects_hole() {
        let dir = tempdir().unwrap();
        let r1 = dir.path().join("movie.part01.rar");
        let r3 = dir.path().join("movie.part03.rar");
        File::create(&r1).unwrap().write_all(b"r1").unwrap();
        File::create(&r3).unwrap().write_all(b"r3").unwrap();

        let desc = PreFlightVolumeProber::probe(&r1).expect("Should detect missing part02");
        assert_eq!(desc.format, "PartNumbered");
        assert_eq!(desc.missing_indices, vec![2]);
        assert_eq!(desc.missing_filenames, vec!["movie.part02.rar"]);
    }

    #[test]
    fn test_probe_standalone_archive_returns_none() {
        let dir = tempdir().unwrap();
        let single = dir.path().join("normal.zip");
        File::create(&single).unwrap().write_all(b"PK\x05\x06\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0").unwrap();

        assert!(PreFlightVolumeProber::probe(&single).is_none());
    }
}

