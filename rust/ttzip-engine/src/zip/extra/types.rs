// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Strongly-typed definitions and constants for the 7 major ZIP Extra Field families.

use crate::crypto::crc32::crc32_fast;

pub const TAG_ZIP64: u16 = 0x0001;
pub const TAG_NTFS: u16 = 0x000a;
pub const TAG_EXT_TIMESTAMP: u16 = 0x5455;
pub const TAG_UNICODE_COMMENT: u16 = 0x6375;
pub const TAG_UNICODE_PATH: u16 = 0x7075;
pub const TAG_ASI_UNIX: u16 = 0x756e;
pub const TAG_INFOZIP_UNIX_NEW: u16 = 0x7875;
pub const TAG_INFOZIP_UNIX: u16 = 0x7875; // Backward-compatible alias
pub const TAG_WINZIP_AES: u16 = 0x9901;
pub const TAG_DATA_STREAM_ALIGNMENT: u16 = 0xa11e;

pub const EXT_TIME_FLAG_MTIME: u8 = 0x01;
pub const EXT_TIME_FLAG_ATIME: u8 = 0x02;
pub const EXT_TIME_FLAG_CTIME: u8 = 0x04;

pub const NTFS_TAG1_TIMESTAMPS: u16 = 0x0001;
pub const WINDOWS_TICK: u64 = 10_000_000;
pub const WINDOWS_EPOCH_DIFF_TICKS: u64 = 116_444_736_000_000_000;

pub const WINZIP_AES_VENDOR_ID: u16 = 0x4541; // "AE"
pub const WINZIP_AES_VERSION_AE1: u16 = 0x0001;
pub const WINZIP_AES_VERSION_AE2: u16 = 0x0002;
pub const WINZIP_AES_STRENGTH_128: u8 = 0x01;
pub const WINZIP_AES_STRENGTH_192: u8 = 0x02;
pub const WINZIP_AES_STRENGTH_256: u8 = 0x03;

pub const ASI_FILE_TYPE_MASK: u16 = 0o170000;
pub const ASI_FILE_TYPE_SOCKET: u16 = 0o140000;
pub const ASI_FILE_TYPE_SYMLINK: u16 = 0o120000;
pub const ASI_FILE_TYPE_REGULAR: u16 = 0o100000;
pub const ASI_FILE_TYPE_DIR: u16 = 0o040000;
pub const ASI_PERM_MASK: u16 = 0o7777;

/// Zip64 Extended Information Extra Field (`0x0001`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Zip64Extra {
    pub uncompressed_size: Option<u64>,
    pub compressed_size: Option<u64>,
    pub local_header_offset: Option<u64>,
    pub disk_start_number: Option<u32>,
}

impl Zip64Extra {
    pub fn parse(
        payload: &[u8],
        is_cdfh: bool,
        uncomp_placeholder: bool,
        comp_placeholder: bool,
        offset_placeholder: bool,
    ) -> Self {
        let mut z64 = Self::default();
        let mut cursor = 0;

        if is_cdfh {
            if uncomp_placeholder && cursor + 8 <= payload.len() {
                z64.uncompressed_size = Some(u64::from_le_bytes(
                    payload[cursor..cursor + 8].try_into().unwrap(),
                ));
                cursor += 8;
            }
            if comp_placeholder && cursor + 8 <= payload.len() {
                z64.compressed_size = Some(u64::from_le_bytes(
                    payload[cursor..cursor + 8].try_into().unwrap(),
                ));
                cursor += 8;
            }
            if offset_placeholder && cursor + 8 <= payload.len() {
                z64.local_header_offset = Some(u64::from_le_bytes(
                    payload[cursor..cursor + 8].try_into().unwrap(),
                ));
                cursor += 8;
            }
            if cursor + 4 <= payload.len() {
                z64.disk_start_number = Some(u32::from_le_bytes(
                    payload[cursor..cursor + 4].try_into().unwrap(),
                ));
            }
        } else {
            if cursor + 8 <= payload.len() {
                z64.uncompressed_size = Some(u64::from_le_bytes(
                    payload[cursor..cursor + 8].try_into().unwrap(),
                ));
                cursor += 8;
            }
            if cursor + 8 <= payload.len() {
                z64.compressed_size = Some(u64::from_le_bytes(
                    payload[cursor..cursor + 8].try_into().unwrap(),
                ));
            }
        }

        z64
    }

    pub fn build_local(&self) -> Vec<u8> {
        let mut payload = Vec::with_capacity(16);
        if let Some(u_sz) = self.uncompressed_size {
            payload.extend_from_slice(&u_sz.to_le_bytes());
        }
        if let Some(c_sz) = self.compressed_size {
            payload.extend_from_slice(&c_sz.to_le_bytes());
        }
        if payload.is_empty() {
            return Vec::new();
        }

        let mut out = Vec::with_capacity(4 + payload.len());
        out.extend_from_slice(&TAG_ZIP64.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        out.extend_from_slice(&payload);
        out
    }

    pub fn build_central(&self) -> Vec<u8> {
        let mut payload = Vec::with_capacity(28);
        if let Some(u_sz) = self.uncompressed_size {
            payload.extend_from_slice(&u_sz.to_le_bytes());
        }
        if let Some(c_sz) = self.compressed_size {
            payload.extend_from_slice(&c_sz.to_le_bytes());
        }
        if let Some(offset) = self.local_header_offset {
            payload.extend_from_slice(&offset.to_le_bytes());
        }
        if let Some(disk) = self.disk_start_number {
            payload.extend_from_slice(&disk.to_le_bytes());
        }
        if payload.is_empty() {
            return Vec::new();
        }

        let mut out = Vec::with_capacity(4 + payload.len());
        out.extend_from_slice(&TAG_ZIP64.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        out.extend_from_slice(&payload);
        out
    }
}

/// Extended Timestamp Extra Field (`0x5455`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExtendedTimestampExtra {
    pub flags: u8,
    pub mod_time: Option<u32>,
    pub acc_time: Option<u32>,
    pub create_time: Option<u32>,
}

impl ExtendedTimestampExtra {
    pub fn parse(payload: &[u8]) -> Option<Self> {
        if payload.is_empty() {
            return None;
        }
        let flags = payload[0];
        let mut cursor = 1;
        let mut mod_time = None;
        let mut acc_time = None;
        let mut create_time = None;

        if (flags & EXT_TIME_FLAG_MTIME) != 0 && cursor + 4 <= payload.len() {
            mod_time = Some(u32::from_le_bytes(
                payload[cursor..cursor + 4].try_into().unwrap(),
            ));
            cursor += 4;
        }
        if (flags & EXT_TIME_FLAG_ATIME) != 0 && cursor + 4 <= payload.len() {
            acc_time = Some(u32::from_le_bytes(
                payload[cursor..cursor + 4].try_into().unwrap(),
            ));
            cursor += 4;
        }
        if (flags & EXT_TIME_FLAG_CTIME) != 0 && cursor + 4 <= payload.len() {
            create_time = Some(u32::from_le_bytes(
                payload[cursor..cursor + 4].try_into().unwrap(),
            ));
        }

        Some(Self {
            flags,
            mod_time,
            acc_time,
            create_time,
        })
    }

    pub fn build_local(&self) -> Vec<u8> {
        let mut flags = 0u8;
        let mut count = 0usize;
        if self.mod_time.is_some() {
            flags |= EXT_TIME_FLAG_MTIME;
            count += 1;
        }
        if self.acc_time.is_some() {
            flags |= EXT_TIME_FLAG_ATIME;
            count += 1;
        }
        if self.create_time.is_some() {
            flags |= EXT_TIME_FLAG_CTIME;
            count += 1;
        }

        if flags == 0 {
            return Vec::new();
        }

        let data_size = (1 + count * 4) as u16;
        let mut out = Vec::with_capacity(4 + data_size as usize);
        out.extend_from_slice(&TAG_EXT_TIMESTAMP.to_le_bytes());
        out.extend_from_slice(&data_size.to_le_bytes());
        out.push(flags);
        if let Some(mtime) = self.mod_time {
            out.extend_from_slice(&mtime.to_le_bytes());
        }
        if let Some(atime) = self.acc_time {
            out.extend_from_slice(&atime.to_le_bytes());
        }
        if let Some(ctime) = self.create_time {
            out.extend_from_slice(&ctime.to_le_bytes());
        }
        out
    }

    pub fn build_central(&self) -> Vec<u8> {
        if let Some(mtime) = self.mod_time {
            let mut out = Vec::with_capacity(9);
            out.extend_from_slice(&TAG_EXT_TIMESTAMP.to_le_bytes());
            out.extend_from_slice(&5u16.to_le_bytes());
            out.push(EXT_TIME_FLAG_MTIME);
            out.extend_from_slice(&mtime.to_le_bytes());
            out
        } else {
            Vec::new()
        }
    }
}

/// Info-ZIP Unix New Extra Field (`0x7875`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct InfoZipUnixNewExtra {
    pub version: u8,
    pub uid: u32,
    pub gid: u32,
}

impl InfoZipUnixNewExtra {
    pub fn parse(payload: &[u8]) -> Option<Self> {
        if payload.is_empty() {
            return Some(Self {
                version: 1,
                uid: 0,
                gid: 0,
            });
        }
        if payload.len() < 4 || payload[0] != 1 {
            return None;
        }

        let version = payload[0];
        let uid_size = payload[1] as usize;
        let mut cursor = 2;
        let mut uid = 0u32;
        let mut gid = 0u32;

        if cursor + uid_size <= payload.len() {
            if uid_size == 2 {
                uid = u16::from_le_bytes([payload[cursor], payload[cursor + 1]]) as u32;
            } else if uid_size == 4 {
                uid = u32::from_le_bytes(payload[cursor..cursor + 4].try_into().unwrap());
            }
            cursor += uid_size;
        }

        if cursor < payload.len() {
            let gid_size = payload[cursor] as usize;
            cursor += 1;
            if cursor + gid_size <= payload.len() {
                if gid_size == 2 {
                    gid = u16::from_le_bytes([payload[cursor], payload[cursor + 1]]) as u32;
                } else if gid_size == 4 {
                    gid = u32::from_le_bytes(payload[cursor..cursor + 4].try_into().unwrap());
                }
            }
        }

        Some(Self { version, uid, gid })
    }

    pub fn build_local(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(15);
        out.extend_from_slice(&TAG_INFOZIP_UNIX_NEW.to_le_bytes());
        out.extend_from_slice(&11u16.to_le_bytes());
        out.push(1); // Version 1
        out.push(4); // UID size = 4
        out.extend_from_slice(&self.uid.to_le_bytes());
        out.push(4); // GID size = 4
        out.extend_from_slice(&self.gid.to_le_bytes());
        out
    }

    pub fn build_central(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4);
        out.extend_from_slice(&TAG_INFOZIP_UNIX_NEW.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }
}

/// Windows NTFS 100ns Timestamps Extra Field (`0x000a`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NtfsExtra {
    pub mtime_ticks: u64,
    pub atime_ticks: u64,
    pub ctime_ticks: u64,
}

impl NtfsExtra {
    #[inline]
    pub fn filetime_to_unix_secs(filetime: u64) -> i64 {
        if filetime < WINDOWS_EPOCH_DIFF_TICKS {
            return -(((WINDOWS_EPOCH_DIFF_TICKS - filetime) / WINDOWS_TICK) as i64);
        }
        ((filetime - WINDOWS_EPOCH_DIFF_TICKS) / WINDOWS_TICK) as i64
    }

    #[inline]
    pub fn filetime_to_unix_nanos(filetime: u64) -> (i64, u32) {
        if filetime < WINDOWS_EPOCH_DIFF_TICKS {
            let diff = WINDOWS_EPOCH_DIFF_TICKS - filetime;
            let secs = -((diff / WINDOWS_TICK) as i64);
            let rem_ticks = (diff % WINDOWS_TICK) as u32;
            let nsec = if rem_ticks > 0 {
                1_000_000_000 - rem_ticks * 100
            } else {
                0
            };
            (secs, nsec)
        } else {
            let diff = filetime - WINDOWS_EPOCH_DIFF_TICKS;
            let secs = (diff / WINDOWS_TICK) as i64;
            let nsec = ((diff % WINDOWS_TICK) * 100) as u32;
            (secs, nsec)
        }
    }

    #[inline]
    pub fn unix_secs_to_filetime(secs: i64) -> u64 {
        if secs < 0 {
            let neg_ticks = (-secs as u64).saturating_mul(WINDOWS_TICK);
            WINDOWS_EPOCH_DIFF_TICKS.saturating_sub(neg_ticks)
        } else {
            (secs as u64)
                .saturating_mul(WINDOWS_TICK)
                .saturating_add(WINDOWS_EPOCH_DIFF_TICKS)
        }
    }

    #[inline]
    pub fn unix_nanos_to_filetime(secs: i64, nsec: u32) -> u64 {
        let base = Self::unix_secs_to_filetime(secs);
        base.saturating_add((nsec / 100) as u64)
    }

    pub fn from_unix_secs(mtime: i64, atime: i64, ctime: i64) -> Self {
        Self {
            mtime_ticks: Self::unix_secs_to_filetime(mtime),
            atime_ticks: Self::unix_secs_to_filetime(atime),
            ctime_ticks: Self::unix_secs_to_filetime(ctime),
        }
    }

    pub fn mtime_unix_secs(&self) -> i64 {
        Self::filetime_to_unix_secs(self.mtime_ticks)
    }

    pub fn atime_unix_secs(&self) -> i64 {
        Self::filetime_to_unix_secs(self.atime_ticks)
    }

    pub fn ctime_unix_secs(&self) -> i64 {
        Self::filetime_to_unix_secs(self.ctime_ticks)
    }

    pub fn parse(payload: &[u8]) -> Option<Self> {
        if payload.len() < 8 {
            return None;
        }
        let mut cursor = 4; // Skip 4 reserved bytes
        let mut ntfs = Self::default();
        let mut found = false;

        while cursor + 4 <= payload.len() {
            let sub_tag = u16::from_le_bytes([payload[cursor], payload[cursor + 1]]);
            let sub_size =
                u16::from_le_bytes([payload[cursor + 2], payload[cursor + 3]]) as usize;
            let sub_start = cursor + 4;

            if sub_start + sub_size > payload.len() {
                break;
            }

            if sub_tag == NTFS_TAG1_TIMESTAMPS && sub_size >= 24 {
                ntfs.mtime_ticks =
                    u64::from_le_bytes(payload[sub_start..sub_start + 8].try_into().unwrap());
                ntfs.atime_ticks =
                    u64::from_le_bytes(payload[sub_start + 8..sub_start + 16].try_into().unwrap());
                ntfs.ctime_ticks = u64::from_le_bytes(
                    payload[sub_start + 16..sub_start + 24].try_into().unwrap(),
                );
                found = true;
            }
            cursor = sub_start + sub_size;
        }

        if found {
            Some(ntfs)
        } else {
            None
        }
    }

    pub fn build(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(36);
        out.extend_from_slice(&TAG_NTFS.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&NTFS_TAG1_TIMESTAMPS.to_le_bytes());
        out.extend_from_slice(&24u16.to_le_bytes());
        out.extend_from_slice(&self.mtime_ticks.to_le_bytes());
        out.extend_from_slice(&self.atime_ticks.to_le_bytes());
        out.extend_from_slice(&self.ctime_ticks.to_le_bytes());
        out
    }
}

/// Info-ZIP Unicode Path (`0x7075`) or Unicode Comment (`0x6375`) Extra Field.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct UnicodeFieldExtra {
    pub tag: u16,
    pub version: u8,
    pub crc32: u32,
    pub text: String,
}

impl UnicodeFieldExtra {
    pub fn parse(tag: u16, payload: &[u8]) -> Option<Self> {
        if payload.len() < 5 || payload[0] != 1 {
            return None;
        }
        let version = payload[0];
        let crc32 = u32::from_le_bytes(payload[1..5].try_into().unwrap());
        let text = std::str::from_utf8(&payload[5..]).ok()?.to_string();
        Some(Self {
            tag,
            version,
            crc32,
            text,
        })
    }

    #[inline]
    pub fn is_valid_for(&self, standard_bytes: &[u8]) -> bool {
        self.version == 1 && crc32_fast(0, standard_bytes) == self.crc32
    }

    pub fn from_text(tag: u16, text: &str, standard_bytes: &[u8]) -> Self {
        Self {
            tag,
            version: 1,
            crc32: crc32_fast(0, standard_bytes),
            text: text.to_string(),
        }
    }

    pub fn build(&self) -> Vec<u8> {
        let text_bytes = self.text.as_bytes();
        let data_size = (5 + text_bytes.len()) as u16;
        let mut out = Vec::with_capacity(4 + data_size as usize);
        out.extend_from_slice(&self.tag.to_le_bytes());
        out.extend_from_slice(&data_size.to_le_bytes());
        out.push(self.version);
        out.extend_from_slice(&self.crc32.to_le_bytes());
        out.extend_from_slice(text_bytes);
        out
    }
}

/// WinZip AES Encryption Extra Field (`0x9901`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WinZipAesExtra {
    pub version: u16,
    pub vendor_id: u16,
    pub strength: u8,
    pub actual_compression_method: u16,
}

impl WinZipAesExtra {
    pub fn parse(payload: &[u8]) -> Option<Self> {
        if payload.len() < 7 {
            return None;
        }
        let version = u16::from_le_bytes([payload[0], payload[1]]);
        let vendor_id = u16::from_le_bytes([payload[2], payload[3]]);
        let strength = payload[4];
        let actual_compression_method = u16::from_le_bytes([payload[5], payload[6]]);

        Some(Self {
            version,
            vendor_id,
            strength: if (1..=3).contains(&strength) {
                strength
            } else {
                0
            },
            actual_compression_method,
        })
    }

    pub fn new(actual_method: u16, strength: u8) -> Self {
        Self {
            version: WINZIP_AES_VERSION_AE2,
            vendor_id: WINZIP_AES_VENDOR_ID,
            strength,
            actual_compression_method: actual_method,
        }
    }

    pub fn build(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(11);
        out.extend_from_slice(&TAG_WINZIP_AES.to_le_bytes());
        out.extend_from_slice(&7u16.to_le_bytes());
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&self.vendor_id.to_le_bytes());
        out.push(self.strength);
        out.extend_from_slice(&self.actual_compression_method.to_le_bytes());
        out
    }
}

/// ASi Unix Extra Field (`0x756e`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AsiUnixExtra {
    pub crc32: u32,
    pub mode: u16,
    pub sizdev: u32,
    pub uid: u16,
    pub gid: u16,
    pub symlink_target: Option<String>,
}

impl AsiUnixExtra {
    pub fn parse(payload: &[u8]) -> Option<Self> {
        if payload.len() < 14 {
            return None;
        }
        let crc32 = u32::from_le_bytes(payload[0..4].try_into().unwrap());
        let mode = u16::from_le_bytes([payload[4], payload[5]]);
        let sizdev = u32::from_le_bytes(payload[6..10].try_into().unwrap());
        let uid = u16::from_le_bytes([payload[10], payload[11]]);
        let gid = u16::from_le_bytes([payload[12], payload[13]]);

        let actual_crc = crc32_fast(0, &payload[4..]);
        if actual_crc != crc32 {
            return None; // CRC mismatch: corrupt extra field
        }

        let symlink_target = if payload.len() > 14 {
            std::str::from_utf8(&payload[14..]).ok().map(|s| s.to_string())
        } else {
            None
        };

        Some(Self {
            crc32,
            mode,
            sizdev,
            uid,
            gid,
            symlink_target,
        })
    }

    #[inline]
    pub fn is_symlink(&self) -> bool {
        (self.mode & ASI_FILE_TYPE_MASK) == ASI_FILE_TYPE_SYMLINK
    }

    #[inline]
    pub fn is_dir(&self) -> bool {
        (self.mode & ASI_FILE_TYPE_MASK) == ASI_FILE_TYPE_DIR
    }

    #[inline]
    pub fn is_regular(&self) -> bool {
        (self.mode & ASI_FILE_TYPE_MASK) == ASI_FILE_TYPE_REGULAR
    }

    #[inline]
    pub fn permissions(&self) -> u16 {
        self.mode & ASI_PERM_MASK
    }

    pub fn new_symlink(mode: u16, uid: u16, gid: u16, target: &str) -> Self {
        let full_mode = ASI_FILE_TYPE_SYMLINK | (mode & ASI_PERM_MASK);
        let target_bytes = target.as_bytes();
        let mut crc_payload = Vec::with_capacity(10 + target_bytes.len());
        crc_payload.extend_from_slice(&full_mode.to_le_bytes());
        crc_payload.extend_from_slice(&0u32.to_le_bytes());
        crc_payload.extend_from_slice(&uid.to_le_bytes());
        crc_payload.extend_from_slice(&gid.to_le_bytes());
        crc_payload.extend_from_slice(target_bytes);
        let crc32 = crc32_fast(0, &crc_payload);

        Self {
            crc32,
            mode: full_mode,
            sizdev: 0,
            uid,
            gid,
            symlink_target: Some(target.to_string()),
        }
    }

    pub fn new_file(mode: u16, size: u32, uid: u16, gid: u16) -> Self {
        let full_mode = ASI_FILE_TYPE_REGULAR | (mode & ASI_PERM_MASK);
        let mut crc_payload = Vec::with_capacity(10);
        crc_payload.extend_from_slice(&full_mode.to_le_bytes());
        crc_payload.extend_from_slice(&size.to_le_bytes());
        crc_payload.extend_from_slice(&uid.to_le_bytes());
        crc_payload.extend_from_slice(&gid.to_le_bytes());
        let crc32 = crc32_fast(0, &crc_payload);

        Self {
            crc32,
            mode: full_mode,
            sizdev: size,
            uid,
            gid,
            symlink_target: None,
        }
    }

    pub fn build(&self) -> Vec<u8> {
        let target_bytes = self.symlink_target.as_deref().unwrap_or("").as_bytes();
        let mut crc_payload = Vec::with_capacity(10 + target_bytes.len());
        crc_payload.extend_from_slice(&self.mode.to_le_bytes());
        crc_payload.extend_from_slice(&self.sizdev.to_le_bytes());
        crc_payload.extend_from_slice(&self.uid.to_le_bytes());
        crc_payload.extend_from_slice(&self.gid.to_le_bytes());
        if !target_bytes.is_empty() {
            crc_payload.extend_from_slice(target_bytes);
        }

        let crc = crc32_fast(0, &crc_payload);
        let data_size = (4 + crc_payload.len()) as u16;

        let mut out = Vec::with_capacity(4 + data_size as usize);
        out.extend_from_slice(&TAG_ASI_UNIX.to_le_bytes());
        out.extend_from_slice(&data_size.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&crc_payload);
        out
    }
}

/// Data Stream Alignment Extra Field (`0xa11e`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DataStreamAlignmentExtra {
    pub alignment: u16,
    pub padding_len: usize,
}

impl DataStreamAlignmentExtra {
    pub fn parse(payload: &[u8], total_field_len: usize) -> Option<Self> {
        if payload.len() < 2 {
            return None;
        }
        let alignment = u16::from_le_bytes([payload[0], payload[1]]);
        Some(Self {
            alignment,
            padding_len: total_field_len,
        })
    }

    pub fn build_local(&self) -> Vec<u8> {
        if self.padding_len < 6 {
            return Vec::new();
        }
        let data_size = (self.padding_len - 4) as u16;
        let mut out = Vec::with_capacity(self.padding_len);
        out.extend_from_slice(&TAG_DATA_STREAM_ALIGNMENT.to_le_bytes());
        out.extend_from_slice(&data_size.to_le_bytes());
        out.extend_from_slice(&self.alignment.to_le_bytes());
        out.resize(self.padding_len, 0u8);
        out
    }

    pub fn build_central(&self) -> Vec<u8> {
        // Strict Invariant: Alignment fields are omitted from Central Directory
        Vec::new()
    }
}
