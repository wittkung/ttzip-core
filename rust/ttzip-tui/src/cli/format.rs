// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive container format detection, unified entry info, byte size formatting,
//! Snappy/Brotli codec parsing, and transparent multi-volume chain detection.

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use ttzip_engine::archive::source::mmap::MmapSource;
use ttzip_engine::archive::source::StorageMedium;
use ttzip_engine::archive::split::{detect_volume_chain as glue_detect_volume_chain, VirtualMultiVolumeReader};
use ttzip_engine::archive::tar::TarArchive;
use ttzip_engine::codecs::brotli::brotli_decompress_to_vec;
use ttzip_engine::codecs::snappy::snappy_frame_decode_to_vec;
use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::sevenz::SevenZArchive;
use ttzip_engine::zip::ZipArchive;

/// Segmented seek view over a multi-volume archive chain without reading all volumes into heap.
pub struct VirtualMultiVolumeSeekView {
    reader: Mutex<VirtualMultiVolumeReader>,
    total_size: u64,
    preview: Vec<u8>,
}

impl VirtualMultiVolumeSeekView {
    /// Creates a segmented view over the multi-volume reader.
    pub fn new(mut reader: VirtualMultiVolumeReader) -> std::io::Result<Self> {
        let total_size = reader.total_size();
        let preview_len = (total_size.min(65536)) as usize;
        let mut preview = vec![0u8; preview_len];
        let n = reader.read(&mut preview)?;
        preview.truncate(n);
        let _ = reader.seek(SeekFrom::Start(0));

        Ok(Self {
            reader: Mutex::new(reader),
            total_size,
            preview,
        })
    }

    #[inline]
    pub fn total_size(&self) -> u64 {
        self.total_size
    }

    #[inline]
    pub fn preview(&self) -> &[u8] {
        &self.preview
    }

    /// Obtains a new reader handle positioned at virtual offset 0.
    pub fn open_reader(&self) -> std::io::Result<VirtualMultiVolumeReader> {
        let guard = self.reader.lock().map_err(|_| {
            std::io::Error::other("VirtualMultiVolumeReader mutex poisoned")
        })?;
        VirtualMultiVolumeReader::from_volumes(guard.volume_paths())
    }
}

/// Memory-mapped or allocated buffer backing archive data with zero-copy slice access.
pub enum ArchiveBuffer {
    Mmap(MmapSource),
    Heap(Vec<u8>),
    Virtual(VirtualMultiVolumeSeekView),
}

impl Deref for ArchiveBuffer {
    type Target = [u8];
    #[inline]
    fn deref(&self) -> &[u8] {
        match self {
            ArchiveBuffer::Mmap(m) => m.as_slice().unwrap_or(&[]),
            ArchiveBuffer::Heap(v) => v.as_slice(),
            ArchiveBuffer::Virtual(v) => v.preview(),
        }
    }
}

impl AsRef<[u8]> for ArchiveBuffer {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.deref()
    }
}

/// Unified entry metadata extracted from any supported archive format.
#[derive(Debug, Clone)]
pub struct ArchiveEntryInfo {
    pub name: String,
    pub relative_path: String,
    pub is_directory: bool,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub is_encrypted: bool,
}

/// Detected archive container format.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ContainerFormat {
    Zip,
    SevenZip,
    Tar,
    Snappy,
    Brotli,
    TarBrotli,
    Unknown,
}

impl ContainerFormat {
    pub fn name(&self) -> &'static str {
        match self {
            ContainerFormat::Zip => "ZIP",
            ContainerFormat::SevenZip => "7Z",
            ContainerFormat::Tar => "TAR",
            ContainerFormat::Snappy => "SNAPPY",
            ContainerFormat::Brotli => "BROTLI",
            ContainerFormat::TarBrotli => "TAR.BR",
            ContainerFormat::Unknown => "UNKNOWN",
        }
    }

    pub fn as_str(&self) -> &'static str {
        self.name()
    }
}

/// Automatically detects multi-volume chain segments from any seed volume path.
pub fn detect_volume_chain(path: &Path) -> std::io::Result<Vec<PathBuf>> {
    glue_detect_volume_chain(path)
}

/// Reads archive data with zero-copy memory mapping, transparently concatenating multi-volume streams if part of a chain.
pub fn read_archive_data_auto(path: &Path) -> Result<(Vec<PathBuf>, ArchiveBuffer), String> {
    let chain = detect_volume_chain(path).unwrap_or_else(|_| vec![path.to_path_buf()]);
    if chain.len() > 1 {
        let reader = VirtualMultiVolumeReader::from_volumes(chain.clone())
            .map_err(|e| format!("Failed to open multi-volume chain: {}", e))?;
        let view = VirtualMultiVolumeSeekView::new(reader)
            .map_err(|e| format!("Failed to create multi-volume seek view: {}", e))?;
        Ok((chain, ArchiveBuffer::Virtual(view)))
    } else {
        match MmapSource::open(path, StorageMedium::LocalFastApfs) {
            Ok(mmap) => Ok((chain, ArchiveBuffer::Mmap(mmap))),
            Err(_) => {
                let data = fs::read(path).map_err(|e| format!("Failed to read archive {}: {}", path.display(), e))?;
                Ok((chain, ArchiveBuffer::Heap(data)))
            }
        }
    }
}

/// Detects archive container format from file extension and magic signature bytes.
pub fn detect_archive_format(path: &Path, data: &[u8]) -> ContainerFormat {
    if data.len() >= 6 && &data[0..6] == b"7z\xBC\xAF\x27\x1C" {
        return ContainerFormat::SevenZip;
    }
    if data.len() >= 10 && &data[0..10] == b"\xFF\x06\x00\x00sNaPpY" {
        return ContainerFormat::Snappy;
    }
    if data.len() >= 265
        && (&data[257..262] == b"ustar"
            || &data[257..265] == b"ustar  \0"
            || &data[257..263] == b"ustar\0")
    {
        return ContainerFormat::Tar;
    }
    if data.len() >= 4
        && (&data[0..4] == b"PK\x03\x04"
            || &data[0..4] == b"PK\x05\x06"
            || &data[0..4] == b"PK\x07\x08")
    {
        return ContainerFormat::Zip;
    }

    let filename_lower = path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if filename_lower.ends_with(".tar.br") || filename_lower.ends_with(".tbr") {
        return ContainerFormat::TarBrotli;
    }
    if filename_lower.ends_with(".br") {
        return ContainerFormat::Brotli;
    }
    if filename_lower.ends_with(".sz") || filename_lower.ends_with(".snappy") {
        return ContainerFormat::Snappy;
    }
    if filename_lower.ends_with(".tar") || filename_lower.contains(".tar.") {
        return ContainerFormat::Tar;
    }
    if filename_lower.ends_with(".7z") || filename_lower.ends_with(".cb7") || filename_lower.contains(".7z.") {
        return ContainerFormat::SevenZip;
    }
    if filename_lower.ends_with(".zip")
        || filename_lower.ends_with(".jar")
        || filename_lower.ends_with(".apk")
        || filename_lower.ends_with(".cbz")
        || filename_lower.contains(".zip.")
        || filename_lower.ends_with(".z01")
        || filename_lower.ends_with(".z02")
    {
        return ContainerFormat::Zip;
    }

    ContainerFormat::Unknown
}

/// Formats byte sizes into human-readable strings.
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes < KB {
        format!("{} B", bytes)
    } else if bytes < MB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    }
}

fn extract_tar_entries(tar: &TarArchive) -> Vec<ArchiveEntryInfo> {
    tar.entries()
        .iter()
        .map(|entry| {
            let name = Path::new(entry.path.as_ref())
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| entry.path.to_string());
            ArchiveEntryInfo {
                name,
                relative_path: entry.path.to_string(),
                is_directory: entry.is_directory,
                uncompressed_size: entry.size,
                compressed_size: entry.size,
                crc32: 0,
                is_encrypted: false,
            }
        })
        .collect()
}

/// Parses ZIP Central Directory entries from a multi-volume chain via segmented Seek without heap exhaustion.
fn parse_multi_volume_zip_entries(chain: &[PathBuf]) -> Result<Vec<ArchiveEntryInfo>, String> {
    let mut reader = VirtualMultiVolumeReader::from_volumes(chain.to_vec())
        .map_err(|e| format!("Failed to open multi-volume chain: {}", e))?;
    let total_size = reader.total_size();
    if total_size < 22 {
        return Ok(Vec::new());
    }

    let search_len = (total_size.min(65557)) as usize;
    let search_start = total_size - search_len as u64;
    reader
        .seek(SeekFrom::Start(search_start))
        .map_err(|e| format!("Seek failed: {}", e))?;
    let mut tail = vec![0u8; search_len];
    reader
        .read_exact(&mut tail)
        .map_err(|e| format!("Read tail failed: {}", e))?;

    // Search backwards for EOCD signature (0x06054B50)
    let mut eocd_offset = None;
    for pos in (0..=search_len.saturating_sub(22)).rev() {
        if tail[pos..pos + 4] == [0x50, 0x4B, 0x05, 0x06] {
            let comment_len = u16::from_le_bytes([tail[pos + 20], tail[pos + 21]]) as usize;
            if pos + 22 + comment_len == search_len {
                eocd_offset = Some(pos);
                break;
            }
        }
    }

    let eocd_pos = match eocd_offset {
        Some(p) => p,
        None => {
            tail.windows(4)
                .rposition(|w| w == [0x50, 0x4B, 0x05, 0x06])
                .ok_or_else(|| "Failed to find ZIP EOCD record in multi-volume chain".to_string())?
        }
    };

    let total_entries = u16::from_le_bytes([tail[eocd_pos + 10], tail[eocd_pos + 11]]) as usize;
    let cd_size = u32::from_le_bytes([
        tail[eocd_pos + 12],
        tail[eocd_pos + 13],
        tail[eocd_pos + 14],
        tail[eocd_pos + 15],
    ]) as usize;
    let cd_offset = u32::from_le_bytes([
        tail[eocd_pos + 16],
        tail[eocd_pos + 17],
        tail[eocd_pos + 18],
        tail[eocd_pos + 19],
    ]) as u64;

    reader
        .seek(SeekFrom::Start(cd_offset))
        .map_err(|e| format!("Seek to central directory failed: {}", e))?;
    let mut cd_bytes = vec![0u8; cd_size];
    reader
        .read_exact(&mut cd_bytes)
        .map_err(|e| format!("Read central directory failed: {}", e))?;

    let mut entries = Vec::with_capacity(total_entries.min(65536));
    let mut cursor = 0;
    while cursor + 46 <= cd_bytes.len() {
        if cd_bytes[cursor..cursor + 4] != [0x50, 0x4B, 0x01, 0x02] {
            break;
        }

        let flag = u16::from_le_bytes([cd_bytes[cursor + 8], cd_bytes[cursor + 9]]);
        let crc32 = u32::from_le_bytes([
            cd_bytes[cursor + 16],
            cd_bytes[cursor + 17],
            cd_bytes[cursor + 18],
            cd_bytes[cursor + 19],
        ]);
        let comp_size = u32::from_le_bytes([
            cd_bytes[cursor + 20],
            cd_bytes[cursor + 21],
            cd_bytes[cursor + 22],
            cd_bytes[cursor + 23],
        ]) as u64;
        let uncomp_size = u32::from_le_bytes([
            cd_bytes[cursor + 24],
            cd_bytes[cursor + 25],
            cd_bytes[cursor + 26],
            cd_bytes[cursor + 27],
        ]) as u64;
        let fn_len = u16::from_le_bytes([cd_bytes[cursor + 28], cd_bytes[cursor + 29]]) as usize;
        let extra_len = u16::from_le_bytes([cd_bytes[cursor + 30], cd_bytes[cursor + 31]]) as usize;
        let comment_len = u16::from_le_bytes([cd_bytes[cursor + 32], cd_bytes[cursor + 33]]) as usize;

        let name_start = cursor + 46;
        let name_end = (name_start + fn_len).min(cd_bytes.len());
        let rel_path = String::from_utf8_lossy(&cd_bytes[name_start..name_end]).to_string();
        let name = Path::new(&rel_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| rel_path.clone());
        let is_directory = rel_path.ends_with('/');
        let is_encrypted = (flag & 1) != 0;

        entries.push(ArchiveEntryInfo {
            name,
            relative_path: rel_path,
            is_directory,
            uncompressed_size: uncomp_size,
            compressed_size: comp_size,
            crc32,
            is_encrypted,
        });

        cursor += 46 + fn_len + extra_len + comment_len;
    }

    Ok(entries)
}

/// Parses archive metadata and returns unified entry records.
pub fn parse_archive_entries(
    path: &Path,
    data: &[u8],
) -> Result<(ContainerFormat, Vec<ArchiveEntryInfo>), String> {
    let format = detect_archive_format(path, data);
    let chain = detect_volume_chain(path).unwrap_or_default();
    if chain.len() > 1 && format == ContainerFormat::Zip {
        return parse_multi_volume_zip_entries(&chain).map(|entries| (format, entries));
    }

    match format {
        ContainerFormat::Zip => {
            let archive =
                ZipArchive::open_slice(data).map_err(|e| format!("Failed to parse ZIP archive: {:?}", e))?;
            let mut entries = Vec::with_capacity(archive.len());
            for entry in archive.entries() {
                let name = Path::new(&entry.rel_path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| entry.rel_path.clone());
                entries.push(ArchiveEntryInfo {
                    name,
                    relative_path: entry.rel_path.clone(),
                    is_directory: entry.is_directory,
                    uncompressed_size: entry.uncompressed_size,
                    compressed_size: entry.compressed_size,
                    crc32: entry.crc32,
                    is_encrypted: entry.is_encrypted,
                });
            }
            Ok((format, entries))
        }
        ContainerFormat::SevenZip => {
            let archive =
                SevenZArchive::open_slice(data).map_err(|e| format!("Failed to parse 7z archive: {:?}", e))?;
            let mut entries = Vec::with_capacity(archive.len());
            let info = archive.info();
            let is_archive_enc = info.is_encrypted;
            let mut stream_idx = 0usize;
            for file in &info.files {
                let (u_sz, crc) = if !file.is_directory && !file.is_empty_stream {
                    let sz = info.stream_sizes.get(stream_idx).copied().unwrap_or(0);
                    let c = info.stream_crcs.get(stream_idx).copied().unwrap_or(0);
                    stream_idx += 1;
                    (sz, c)
                } else {
                    (0, 0)
                };

                let name = Path::new(&file.rel_path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| file.rel_path.clone());
                entries.push(ArchiveEntryInfo {
                    name,
                    relative_path: file.rel_path.clone(),
                    is_directory: file.is_directory,
                    uncompressed_size: u_sz,
                    compressed_size: if u_sz > 0 {
                        info.payload_len as u64 / info.stream_sizes.len().max(1) as u64
                    } else {
                        0
                    },
                    crc32: crc,
                    is_encrypted: is_archive_enc,
                });
            }
            Ok((format, entries))
        }
        ContainerFormat::Tar => {
            let archive =
                TarArchive::open_slice(data).map_err(|e| format!("Failed to parse TAR archive: {:?}", e))?;
            Ok((format, extract_tar_entries(&archive)))
        }
        ContainerFormat::Snappy => {
            let decompressed = snappy_frame_decode_to_vec(data, 1024 * 1024 * 512)
                .map_err(|e| format!("Failed to decompress Snappy stream: {:?}", e))?;

            let file_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("decompressed.bin");
            let is_tar = file_name.to_lowercase().contains(".tar.");

            if is_tar {
                if let Ok(tar) = TarArchive::open_slice(&decompressed) {
                    if !tar.is_empty() {
                        return Ok((format, extract_tar_entries(&tar)));
                    }
                }
            }

            let inner_name = file_name
                .strip_suffix(".sz")
                .or_else(|| file_name.strip_suffix(".snappy"))
                .unwrap_or(file_name)
                .to_string();

            let crc = crc32_fast(0, &decompressed);
            let entry = ArchiveEntryInfo {
                name: inner_name.clone(),
                relative_path: inner_name,
                is_directory: false,
                uncompressed_size: decompressed.len() as u64,
                compressed_size: data.len() as u64,
                crc32: crc,
                is_encrypted: false,
            };
            Ok((format, vec![entry]))
        }
        ContainerFormat::Brotli | ContainerFormat::TarBrotli => {
            let decompressed = brotli_decompress_to_vec(data, 1024 * 1024 * 512)
                .map_err(|e| format!("Failed to decompress Brotli stream: {:?}", e))?;

            let file_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("decompressed.bin");
            let is_tar = format == ContainerFormat::TarBrotli
                || file_name.to_lowercase().contains(".tar.")
                || file_name.to_lowercase().ends_with(".tbr");

            if is_tar {
                if let Ok(tar) = TarArchive::open_slice(&decompressed) {
                    if !tar.is_empty() {
                        return Ok((format, extract_tar_entries(&tar)));
                    }
                }
            }

            let inner_name = file_name
                .strip_suffix(".br")
                .unwrap_or(file_name)
                .to_string();

            let crc = crc32_fast(0, &decompressed);
            let entry = ArchiveEntryInfo {
                name: inner_name.clone(),
                relative_path: inner_name,
                is_directory: false,
                uncompressed_size: decompressed.len() as u64,
                compressed_size: data.len() as u64,
                crc32: crc,
                is_encrypted: false,
            };
            Ok((format, vec![entry]))
        }
        ContainerFormat::Unknown => Err(format!(
            "Unsupported or unrecognized archive format for: {}",
            path.display()
        )),
    }
}

/// Sanitizes a string for safe terminal output, defending against ANSI escape injection (CWE-150),
/// terminal control codes, OSC 8 hyperlinks, cursor manipulations, and invisible/bidi characters.
pub fn sanitize_terminal_display(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];
        if ch == '\x1b' {
            // ANSI Escape Sequence start
            i += 1;
            if i >= len {
                break;
            }
            match chars[i] {
                '[' => {
                    // CSI: ESC [ Parameter* Intermediate* Final
                    i += 1;
                    // Parameter bytes: 0x30..=0x3F ('0'..='?' / '0'..='9', ';', etc.)
                    while i < len && matches!(chars[i], '\x30'..='\x3F') {
                        i += 1;
                    }
                    // Intermediate bytes: 0x20..=0x2F (' '..='/')
                    while i < len && matches!(chars[i], '\x20'..='\x2F') {
                        i += 1;
                    }
                    // Final byte: 0x40..=0x7E ('@'..='~')
                    if i < len && matches!(chars[i], '\x40'..='\x7E') {
                        i += 1;
                    }
                }
                ']' => {
                    // OSC: ESC ] ... (terminated by BEL '\x07' or ST '\x1b\\')
                    i += 1;
                    while i < len {
                        if chars[i] == '\x07' {
                            i += 1;
                            break;
                        }
                        if chars[i] == '\x1b' && i + 1 < len && chars[i + 1] == '\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                'P' | '_' | '^' | 'X' => {
                    // DCS / APC / PM / SOS: ESC ... (terminated by BEL or ST)
                    i += 1;
                    while i < len {
                        if chars[i] == '\x07' {
                            i += 1;
                            break;
                        }
                        if chars[i] == '\x1b' && i + 1 < len && chars[i + 1] == '\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                '(' | ')' | '*' | '+' => {
                    // Charset designation: ESC ( C
                    i += 2;
                }
                _ => {
                    // 2-byte escape: skip the byte after ESC
                    i += 1;
                }
            }
        } else if ch == '\x07' {
            // Stray BEL character
            i += 1;
        } else if ch == '\r' || ch == '\n' || ch == '\t' {
            match ch {
                '\r' => out.push_str("\\r"),
                '\n' => out.push_str("\\n"),
                '\t' => out.push_str("\\t"),
                _ => unreachable!(),
            }
            i += 1;
        } else if ch.is_ascii_control() {
            out.push_str(&format!("\\x{:02x}", ch as u32));
            i += 1;
        } else if matches!(ch, '\u{200B}'..='\u{200D}' | '\u{FEFF}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}') {
            // Filter invisible zero-width and Bidi override characters
            i += 1;
        } else {
            out.push(ch);
            i += 1;
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_terminal_display_ansi_csi_and_clear_screen() {
        assert_eq!(sanitize_terminal_display("\x1b[2Jhello"), "hello");
        assert_eq!(sanitize_terminal_display("safe\x1b[31;1m_red\x1b[0m_end"), "safe_red_end");
        assert_eq!(sanitize_terminal_display("\x1b[?1049hsecret"), "secret");
        assert_eq!(sanitize_terminal_display("\x1b[2K\rline"), "\\rline");
    }

    #[test]
    fn test_sanitize_terminal_display_osc8_and_control_chars() {
        // OSC 8 hyperlink with BEL terminator
        let osc_bel = "\x1b]8;;http://evil.com\x07Click\x1b]8;;\x07";
        assert_eq!(sanitize_terminal_display(osc_bel), "Click");

        // OSC 8 hyperlink with ST terminator
        let osc_st = "\x1b]8;;http://evil.com\x1b\\Click\x1b]8;;\x1b\\";
        assert_eq!(sanitize_terminal_display(osc_st), "Click");

        // Control characters and invisible zero-width / bidi characters
        assert_eq!(sanitize_terminal_display("foo\0bar"), "foo\\x00bar");
        assert_eq!(sanitize_terminal_display("a\r\nb\tc"), "a\\r\\nb\\tc");
        assert_eq!(sanitize_terminal_display("bidi\u{202E}txt.exe"), "biditxt.exe");
        assert_eq!(sanitize_terminal_display("zero\u{200B}width"), "zerowidth");
        assert_eq!(sanitize_terminal_display("clean/path/to/file.txt"), "clean/path/to/file.txt");
    }
}
