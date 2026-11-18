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
use ttzip_engine::archive::unified::inspect::inspect_archive;
use ttzip_engine::codecs::brotli::brotli_decompress_to_vec;
use ttzip_engine::codecs::snappy::snappy_frame_decode_to_vec;
use ttzip_engine::crypto::crc32::crc32_fast;
use ttzip_engine::sevenz::SevenZArchive;
use ttzip_engine::standards::detect_format_buffer;
use ttzip_engine::standards::signatures::{CompoundFormat, DetectedFormat};
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

impl ArchiveEntryInfo {
    fn from_path(
        relative_path: String,
        is_directory: bool,
        uncompressed_size: u64,
        compressed_size: u64,
        crc32: u32,
        is_encrypted: bool,
    ) -> Self {
        let name = Path::new(&relative_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| relative_path.clone());
        Self {
            name,
            relative_path,
            is_directory,
            uncompressed_size,
            compressed_size,
            crc32,
            is_encrypted,
        }
    }
}

/// Detected archive container format.
#[repr(usize)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ContainerFormat {
    Zip = 0,
    SevenZip = 1,
    Tar = 2,
    Snappy = 3,
    Brotli = 4,
    TarBrotli = 5,
    Gzip = 6,
    Bzip2 = 7,
    Xz = 8,
    Zstd = 9,
    Lz4 = 10,
    Lzip = 11,
    Lrzip = 12,
    Iso = 13,
    Cab = 14,
    Wim = 15,
    Dmg = 16,
    Aar = 17,
    Cpio = 18,
    Ar = 19,
    Deb = 20,
    Rpm = 21,
    Xar = 22,
    Rar = 23,
    Squashfs = 24,
    Lzfse = 25,
    Lzh = 26,
    Unknown = 27,
}

const CONTAINER_FORMAT_NAMES: [&str; 28] = [
    "ZIP", "7Z", "TAR", "SNAPPY", "BROTLI", "TAR.BR", "GZIP", "BZIP2", "XZ", "ZSTD", "LZ4",
    "LZIP", "LRZIP", "ISO", "CAB", "WIM", "DMG", "AAR", "CPIO", "AR", "DEB", "RPM", "XAR", "RAR",
    "SQUASHFS", "LZFSE", "LZH", "UNKNOWN",
];

#[allow(
    non_upper_case_globals,
    reason = "Compound tar format aliases matching enum variant naming convention"
)]
impl ContainerFormat {
    pub const TarGz: Self = Self::Gzip;
    pub const TarBz2: Self = Self::Bzip2;
    pub const TarXz: Self = Self::Xz;
    pub const TarZstd: Self = Self::Zstd;
    pub const TarZst: Self = Self::Zstd;
    pub const TarLz4: Self = Self::Lz4;
    pub const TarLzip: Self = Self::Lzip;
    pub const TarLrzip: Self = Self::Lrzip;
    pub const TarSnappy: Self = Self::Snappy;

    #[inline]
    pub fn name(&self) -> &'static str {
        CONTAINER_FORMAT_NAMES[*self as usize]
    }

    #[inline]
    pub fn as_str(&self) -> &'static str {
        self.name()
    }

    #[inline]
    pub fn detect(path: &Path, data: &[u8]) -> Self {
        detect_archive_format(path, data)
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

const PREFIX_MAGIC_TABLE: &[(&[u8], ContainerFormat)] = &[
    (b"7z\xBC\xAF\x27\x1C", ContainerFormat::SevenZip),
    (b"\xFF\x06\x00\x00sNaPpY", ContainerFormat::Snappy),
    (b"PK\x03\x04", ContainerFormat::Zip), (b"PK\x05\x06", ContainerFormat::Zip),
    (b"PK\x07\x08", ContainerFormat::Zip), (b"MSWIM\0\0\0", ContainerFormat::Wim),
    (b"WLPWM\0\0\0", ContainerFormat::Wim), (b"pbze", ContainerFormat::Aar),
    (b"AA01", ContainerFormat::Aar), (b"YAA1", ContainerFormat::Aar),
    (b"AEA1", ContainerFormat::Aar), (b"bvx2", ContainerFormat::Lzfse),
    (b"bvx-", ContainerFormat::Lzfse), (b"bvx1", ContainerFormat::Lzfse),
    (b"bvxn", ContainerFormat::Lzfse),
];

const DETECTED_TO_CONTAINER: &[(DetectedFormat, ContainerFormat)] = &[
    (DetectedFormat::Zip, ContainerFormat::Zip), (DetectedFormat::Zip64, ContainerFormat::Zip),
    (DetectedFormat::SevenZip, ContainerFormat::SevenZip), (DetectedFormat::Tar, ContainerFormat::Tar),
    (DetectedFormat::Snappy, ContainerFormat::Snappy), (DetectedFormat::Brotli, ContainerFormat::Brotli),
    (DetectedFormat::Gzip, ContainerFormat::Gzip), (DetectedFormat::Bzip2, ContainerFormat::Bzip2),
    (DetectedFormat::Xz, ContainerFormat::Xz), (DetectedFormat::Zstd, ContainerFormat::Zstd),
    (DetectedFormat::Lz4, ContainerFormat::Lz4), (DetectedFormat::Lzip, ContainerFormat::Lzip),
    (DetectedFormat::Lrzip, ContainerFormat::Lrzip), (DetectedFormat::Iso, ContainerFormat::Iso),
    (DetectedFormat::Cab, ContainerFormat::Cab), (DetectedFormat::Wim, ContainerFormat::Wim),
    (DetectedFormat::Dmg, ContainerFormat::Dmg), (DetectedFormat::Aar, ContainerFormat::Aar),
    (DetectedFormat::Cpio, ContainerFormat::Cpio), (DetectedFormat::Ar, ContainerFormat::Ar),
    (DetectedFormat::Deb, ContainerFormat::Deb), (DetectedFormat::Rpm, ContainerFormat::Rpm),
    (DetectedFormat::Xar, ContainerFormat::Xar), (DetectedFormat::Rar, ContainerFormat::Rar),
    (DetectedFormat::Squashfs, ContainerFormat::Squashfs), (DetectedFormat::Lzfse, ContainerFormat::Lzfse),
    (DetectedFormat::Lzh, ContainerFormat::Lzh),
];

fn detect_offset_magic(data: &[u8]) -> Option<ContainerFormat> {
    if data.len() >= 262 && data[257..262] == *b"ustar" {
        return Some(ContainerFormat::Tar);
    }
    if data.len() >= 7 && data[6] == b'-' && matches!(&data[2..4], b"-l" | b"-p") {
        return Some(ContainerFormat::Lzh);
    }
    if data.len() >= 512 && &data[data.len() - 512..data.len() - 508] == b"koly" {
        return Some(ContainerFormat::Dmg);
    }
    None
}

fn detect_special_magic(path: &Path, data: &[u8]) -> Option<ContainerFormat> {
    if data.starts_with(b"!<arch>\n") {
        let is_deb = path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.to_ascii_lowercase().ends_with(".deb"));
        return Some(if is_deb { ContainerFormat::Deb } else { ContainerFormat::Ar });
    }
    for (magic, fmt) in PREFIX_MAGIC_TABLE {
        if data.starts_with(magic) {
            return Some(*fmt);
        }
    }
    detect_offset_magic(data)
}

fn detect_format_by_magic(path: &Path, data: &[u8]) -> Option<ContainerFormat> {
    if data.is_empty() {
        return None;
    }
    if let Some(fmt) = detect_special_magic(path, data) {
        return Some(fmt);
    }
    let hint = path.file_name().and_then(|s| s.to_str());
    let sniff = detect_format_buffer(data, hint);
    if sniff.compound_format == Some(CompoundFormat::TarBrotli) {
        return Some(ContainerFormat::TarBrotli);
    }
    DETECTED_TO_CONTAINER
        .iter()
        .find_map(|&(det, fmt)| (det == sniff.format).then_some(fmt))
}

const SUFFIX_FORMATS: &[(&[&str], ContainerFormat)] = &[
    (&[".tar.br", ".tbr"], ContainerFormat::TarBrotli),
    (&[".tar.gz", ".tgz", ".taz", ".gz", ".z"], ContainerFormat::Gzip),
    (&[".tar.bz2", ".tbz2", ".tbz", ".tb2", ".bz2"], ContainerFormat::Bzip2),
    (&[".tar.xz", ".txz", ".xz", ".lzma"], ContainerFormat::Xz),
    (&[".tar.zst", ".tzst", ".zst"], ContainerFormat::Zstd),
    (&[".tar.lz4", ".tlz4", ".lz4"], ContainerFormat::Lz4),
    (&[".tar.lz", ".tlz", ".lz", ".lzip"], ContainerFormat::Lzip),
    (&[".tar.lrz", ".tlrz", ".lrz", ".lrzip"], ContainerFormat::Lrzip),
    (&[".tar.sz", ".tar.snappy", ".sz", ".snappy"], ContainerFormat::Snappy),
    (&[".br"], ContainerFormat::Brotli),
    (&[".tar", ".cbt"], ContainerFormat::Tar),
    (&[".7z", ".cb7"], ContainerFormat::SevenZip),
    (&[".zip", ".jar", ".apk", ".cbz", ".epub", ".z01", ".z02"], ContainerFormat::Zip),
    (&[".iso", ".img"], ContainerFormat::Iso),
    (&[".cab"], ContainerFormat::Cab),
    (&[".wim", ".swm", ".esd"], ContainerFormat::Wim),
    (&[".dmg"], ContainerFormat::Dmg),
    (&[".aar", ".aea"], ContainerFormat::Aar),
    (&[".cpio"], ContainerFormat::Cpio),
    (&[".deb"], ContainerFormat::Deb),
    (&[".ar", ".a"], ContainerFormat::Ar),
    (&[".rpm"], ContainerFormat::Rpm),
    (&[".xar", ".pkg"], ContainerFormat::Xar),
    (&[".rar", ".cbr"], ContainerFormat::Rar),
    (&[".squashfs", ".sqsh"], ContainerFormat::Squashfs),
    (&[".lzfse"], ContainerFormat::Lzfse),
    (&[".lzh", ".lha"], ContainerFormat::Lzh),
];

fn strip_split_volume_suffix(name_lower: &str) -> &str {
    if let Some((stem, ext)) = name_lower.rsplit_once('.') {
        let is_num = ext.len() >= 3 && ext.bytes().all(|b| b.is_ascii_digit());
        let is_z = ext.starts_with('z')
            && ext.len() >= 3
            && ext.as_bytes()[1..].iter().all(|b| b.is_ascii_digit());
        if is_num || is_z {
            return stem;
        }
    }
    name_lower
}

/// Detects archive container format from file extension and magic signature bytes.
pub fn detect_archive_format(path: &Path, data: &[u8]) -> ContainerFormat {
    if let Some(fmt) = detect_format_by_magic(path, data) {
        return fmt;
    }

    let raw_lower = path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    let filename_lower = strip_split_volume_suffix(&raw_lower);

    for (suffixes, fmt) in SUFFIX_FORMATS {
        if suffixes.iter().any(|suf| filename_lower.ends_with(suf)) {
            return *fmt;
        }
    }
    if filename_lower.contains(".tar.") {
        return ContainerFormat::Tar;
    }
    if filename_lower.contains(".7z.") {
        return ContainerFormat::SevenZip;
    }
    if filename_lower.contains(".zip.") {
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
            ArchiveEntryInfo::from_path(
                entry.path.to_string(),
                entry.is_directory,
                entry.size,
                entry.size,
                0,
                false,
            )
        })
        .collect()
}

fn parse_zip_entries_slice(data: &[u8]) -> Result<Vec<ArchiveEntryInfo>, String> {
    let archive = ZipArchive::open_slice(data)
        .map_err(|e| format!("Failed to parse ZIP archive: {:?}", e))?;
    Ok(archive
        .entries()
        .iter()
        .map(|entry| {
            ArchiveEntryInfo::from_path(
                entry.rel_path.clone(),
                entry.is_directory,
                entry.uncompressed_size,
                entry.compressed_size,
                entry.crc32,
                entry.is_encrypted,
            )
        })
        .collect())
}

fn parse_sevenz_entries_slice(
    data: &[u8],
    password: Option<&str>,
) -> Result<Vec<ArchiveEntryInfo>, String> {
    let archive = SevenZArchive::open_slice_with_password(data, password)
        .map_err(|e| format!("Failed to parse 7z archive: {:?}", e))?;
    let mut entries = Vec::with_capacity(archive.len());
    let info = archive.info();
    let is_archive_enc = info.is_encrypted;
    let mut stream_idx = 0usize;
    let stream_count = info.stream_sizes.len().max(1) as u64;

    for file in &info.files {
        let (u_sz, crc) = if !file.is_directory && !file.is_empty_stream {
            let sz = info.stream_sizes.get(stream_idx).copied().unwrap_or(0);
            let c = info.stream_crcs.get(stream_idx).copied().unwrap_or(0);
            stream_idx += 1;
            (sz, c)
        } else {
            (0, 0)
        };
        let comp_sz = if u_sz > 0 { info.payload_len as u64 / stream_count } else { 0 };
        entries.push(ArchiveEntryInfo::from_path(
            file.rel_path.clone(),
            file.is_directory,
            u_sz,
            comp_sz,
            crc,
            is_archive_enc,
        ));
    }
    Ok(entries)
}

fn build_stream_entries(
    path: &Path,
    decompressed: &[u8],
    compressed_len: u64,
    is_tar: bool,
    suffixes: &[&str],
) -> Vec<ArchiveEntryInfo> {
    if is_tar {
        if let Ok(tar) = TarArchive::open_slice(decompressed) {
            if !tar.is_empty() {
                return extract_tar_entries(&tar);
            }
        }
    }

    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("decompressed.bin");
    let mut inner_name = file_name;
    for suf in suffixes {
        if let Some(stripped) = file_name.strip_suffix(suf) {
            inner_name = stripped;
            break;
        }
    }

    vec![ArchiveEntryInfo::from_path(
        inner_name.to_string(),
        false,
        decompressed.len() as u64,
        compressed_len,
        crc32_fast(0, decompressed),
        false,
    )]
}

fn parse_snappy_entries(path: &Path, data: &[u8]) -> Result<Vec<ArchiveEntryInfo>, String> {
    let decompressed = snappy_frame_decode_to_vec(data, 1024 * 1024 * 512)
        .map_err(|e| format!("Failed to decompress Snappy stream: {:?}", e))?;
    let fname_lower = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    Ok(build_stream_entries(
        path,
        &decompressed,
        data.len() as u64,
        fname_lower.contains(".tar."),
        &[".sz", ".snappy"],
    ))
}

fn parse_brotli_entries(
    path: &Path,
    data: &[u8],
    format: ContainerFormat,
) -> Result<Vec<ArchiveEntryInfo>, String> {
    let decompressed = brotli_decompress_to_vec(data, 1024 * 1024 * 512)
        .map_err(|e| format!("Failed to decompress Brotli stream: {:?}", e))?;
    let fname_lower = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let is_tar = format == ContainerFormat::TarBrotli
        || fname_lower.contains(".tar.")
        || fname_lower.ends_with(".tbr");
    Ok(build_stream_entries(
        path,
        &decompressed,
        data.len() as u64,
        is_tar,
        &[".br"],
    ))
}

fn parse_engine_archive_entries(
    path: &Path,
    data: &[u8],
    password: Option<&str>,
) -> Result<Vec<ArchiveEntryInfo>, String> {
    crate::cli::handlers::check::ensure_utf8_c_locale();
    let mut temp_dir = None;
    let target_path = if path.exists() {
        path.to_path_buf()
    } else {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("ttzip_tui_inspect_{}_{}", std::process::id(), nanos));
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create temporary inspect directory: {}", e))?;
        let fname = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("archive.bin");
        let tmp_file = dir.join(fname);
        fs::write(&tmp_file, data)
            .map_err(|e| format!("Failed to stage archive buffer for inspection: {}", e))?;
        temp_dir = Some(dir);
        tmp_file
    };

    let mut entries = Vec::new();
    let res = inspect_archive(&target_path, password, true, |entry| {
        entries.push(ArchiveEntryInfo::from_path(
            entry.path.to_string(),
            entry.is_directory,
            entry.uncompressed_size,
            entry.compressed_size,
            entry.crc32,
            entry.is_encrypted,
        ));
        true
    });

    if let Some(dir) = temp_dir {
        let _ = fs::remove_dir_all(dir);
    }

    res.map_err(|e| format!("Failed to inspect archive {}: {:?}", path.display(), e))?;
    Ok(entries)
}

/// Parses archive metadata with optional password and returns unified entry records.
pub fn parse_archive_entries_with_password(
    path: &Path,
    data: &[u8],
    password: Option<&str>,
) -> Result<(ContainerFormat, Vec<ArchiveEntryInfo>), String> {
    let format = detect_archive_format(path, data);
    if format == ContainerFormat::Unknown {
        return Err(format!(
            "Unsupported or unrecognized archive format for: {}",
            path.display()
        ));
    }

    let chain = detect_volume_chain(path).unwrap_or_default();
    if chain.len() > 1 {
        return parse_engine_archive_entries(path, data, password).map(|entries| (format, entries));
    }

    let entries = match format {
        ContainerFormat::Zip => parse_zip_entries_slice(data)?,
        ContainerFormat::SevenZip => parse_sevenz_entries_slice(data, password)?,
        ContainerFormat::Tar => {
            let archive = TarArchive::open_slice(data)
                .map_err(|e| format!("Failed to parse TAR archive: {:?}", e))?;
            extract_tar_entries(&archive)
        }
        ContainerFormat::Snappy => parse_snappy_entries(path, data)?,
        ContainerFormat::Brotli | ContainerFormat::TarBrotli => {
            parse_brotli_entries(path, data, format)?
        }
        _ => parse_engine_archive_entries(path, data, password)?,
    };
    Ok((format, entries))
}

/// Parses archive metadata and returns unified entry records.
pub fn parse_archive_entries(
    path: &Path,
    data: &[u8],
) -> Result<(ContainerFormat, Vec<ArchiveEntryInfo>), String> {
    parse_archive_entries_with_password(path, data, None)
}

fn skip_st_terminated_seq(chars: &[char], mut i: usize) -> usize {
    let len = chars.len();
    while i < len {
        if chars[i] == '\x07' {
            return i + 1;
        }
        if chars[i] == '\x1b' && i + 1 < len && chars[i + 1] == '\\' {
            return i + 2;
        }
        i += 1;
    }
    i
}

fn skip_ansi_escape_seq(chars: &[char], mut i: usize) -> usize {
    let len = chars.len();
    if i >= len {
        return i;
    }
    match chars[i] {
        '[' => {
            i += 1;
            while i < len && matches!(chars[i], '\x30'..='\x3F') {
                i += 1;
            }
            while i < len && matches!(chars[i], '\x20'..='\x2F') {
                i += 1;
            }
            if i < len && matches!(chars[i], '\x40'..='\x7E') {
                i += 1;
            }
            i
        }
        ']' | 'P' | '_' | '^' | 'X' => skip_st_terminated_seq(chars, i + 1),
        '(' | ')' | '*' | '+' => i + 2,
        _ => i + 1,
    }
}

fn append_sanitized_char(out: &mut String, ch: char) {
    match ch {
        '\x07' => {}
        '\r' => out.push_str("\\r"),
        '\n' => out.push_str("\\n"),
        '\t' => out.push_str("\\t"),
        c if c.is_ascii_control() => out.push_str(&format!("\\x{:02x}", c as u32)),
        '\u{200B}'..='\u{200D}' | '\u{FEFF}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' => {}
        c => out.push(c),
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
            i = skip_ansi_escape_seq(&chars, i + 1);
        } else {
            append_sanitized_char(&mut out, ch);
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

    #[test]
    fn test_detect_compound_tar_and_engine_archive_formats() {
        let cases: &[(&str, ContainerFormat)] = &[
            ("bundle.tar.gz", ContainerFormat::Gzip),
            ("bundle.tgz", ContainerFormat::Gzip),
            ("bundle.taz", ContainerFormat::Gzip),
            ("bundle.tar.bz2", ContainerFormat::Bzip2),
            ("bundle.tbz2", ContainerFormat::Bzip2),
            ("bundle.tbz", ContainerFormat::Bzip2),
            ("bundle.tb2", ContainerFormat::Bzip2),
            ("bundle.tar.xz", ContainerFormat::Xz),
            ("bundle.txz", ContainerFormat::Xz),
            ("bundle.tar.zst", ContainerFormat::Zstd),
            ("bundle.tzst", ContainerFormat::Zstd),
            ("bundle.tar.lz4", ContainerFormat::Lz4),
            ("bundle.tlz4", ContainerFormat::Lz4),
            ("bundle.tar.lz", ContainerFormat::Lzip),
            ("bundle.tlz", ContainerFormat::Lzip),
            ("bundle.tar.lrz", ContainerFormat::Lrzip),
            ("bundle.tlrz", ContainerFormat::Lrzip),
            ("bundle.tar.br", ContainerFormat::TarBrotli),
            ("bundle.tbr", ContainerFormat::TarBrotli),
            ("bundle.tar.sz", ContainerFormat::Snappy),
            ("stream.gz", ContainerFormat::Gzip),
            ("stream.z", ContainerFormat::Gzip),
            ("stream.bz2", ContainerFormat::Bzip2),
            ("stream.xz", ContainerFormat::Xz),
            ("stream.lzma", ContainerFormat::Xz),
            ("stream.zst", ContainerFormat::Zstd),
            ("stream.lz4", ContainerFormat::Lz4),
            ("stream.lz", ContainerFormat::Lzip),
            ("stream.lrz", ContainerFormat::Lrzip),
            ("stream.br", ContainerFormat::Brotli),
            ("stream.sz", ContainerFormat::Snappy),
            ("stream.snappy", ContainerFormat::Snappy),
            ("image.iso", ContainerFormat::Iso),
            ("setup.cab", ContainerFormat::Cab),
            ("install.wim", ContainerFormat::Wim),
            ("macos.dmg", ContainerFormat::Dmg),
            ("payload.aar", ContainerFormat::Aar),
            ("initramfs.cpio", ContainerFormat::Cpio),
            ("libfoo.ar", ContainerFormat::Ar),
            ("pkg.deb", ContainerFormat::Deb),
            ("pkg.rpm", ContainerFormat::Rpm),
            ("installer.xar", ContainerFormat::Xar),
            ("archive.rar", ContainerFormat::Rar),
            ("rootfs.squashfs", ContainerFormat::Squashfs),
            ("cache.lzfse", ContainerFormat::Lzfse),
            ("legacy.lzh", ContainerFormat::Lzh),
            ("legacy.lha", ContainerFormat::Lzh),
        ];
        for (name, expected) in cases {
            let detected = ContainerFormat::detect(Path::new(name), b"");
            assert_eq!(detected, *expected, "unexpected format for {}", name);
            assert_ne!(detected, ContainerFormat::Unknown, "{} must not be Unknown", name);
        }
    }
}
