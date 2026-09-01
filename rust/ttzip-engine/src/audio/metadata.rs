// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Audio container metadata parsing, standard tag normalization, and embedded cover art extraction.
//!
//! Provides zero-unsafe extraction of ID3v1, ID3v2, VorbisComment, MP4 ilst, RIFF INFO,
//! and FLAC metadata blocks, complete with cover art MIME detection and geometry sniffing.

use std::collections::HashMap;
use std::fs::File;
use std::io::Cursor;
use std::path::Path;

use serde::{Deserialize, Serialize};
use symphonia::core::codecs::CODEC_TYPE_NULL;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, StandardTagKey, Tag, Value};
use symphonia::core::probe::Hint;

use super::AudioError;

/// Categorization for embedded audio cover art and visual assets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioPictureType {
    Other,
    FileIcon,
    OtherIcon,
    CoverFront,
    CoverBack,
    LeafletPage,
    Media,
    LeadArtist,
    Artist,
    Conductor,
    Band,
    Composer,
    Lyricist,
    RecordingLocation,
    DuringRecording,
    DuringPerformance,
    ScreenCapture,
    BrightColoredFish,
    Illustration,
    BandLogo,
    PublisherLogo,
}

/// Embedded visual artwork (e.g. album cover, artist photo, back cover).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioCoverArt {
    /// MIME type of the embedded artwork image (e.g. "image/jpeg", "image/png").
    pub mime_type: String,
    /// Semantic picture type categorization.
    pub picture_type: AudioPictureType,
    /// Optional artwork description or caption.
    pub description: Option<String>,
    /// Pixel width if declared in container or sniffed from image header.
    pub width: Option<u32>,
    /// Pixel height if declared in container or sniffed from image header.
    pub height: Option<u32>,
    /// Raw binary image payload bytes.
    pub data: Vec<u8>,
}

/// Normalized audio metadata summary extracted from container tags and streams.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AudioMetadata {
    /// Track title.
    pub title: Option<String>,
    /// Main performing artist.
    pub artist: Option<String>,
    /// Album or release name.
    pub album: Option<String>,
    /// Album primary artist / band.
    pub album_artist: Option<String>,
    /// Track composer.
    pub composer: Option<String>,
    /// Genre classification.
    pub genre: Option<String>,
    /// Release year (e.g. 2024).
    pub year: Option<u32>,
    /// Track number on disc.
    pub track_number: Option<u32>,
    /// Total tracks on disc.
    pub total_tracks: Option<u32>,
    /// Disc number in multi-disc set.
    pub disc_number: Option<u32>,
    /// Total discs in set.
    pub total_discs: Option<u32>,
    /// Freeform comments or remarks.
    pub comment: Option<String>,
    /// Synchronized or unsynchronized lyrics text.
    pub lyrics: Option<String>,
    /// Beats per minute tempo.
    pub bpm: Option<f32>,
    /// Total track duration in seconds.
    pub duration_seconds: Option<f64>,
    /// Bitrate in kbps.
    pub bitrate_kbps: Option<u32>,
    /// Audio sample rate in Hz.
    pub sample_rate: Option<u32>,
    /// Number of audio channels.
    pub channels: Option<u32>,
    /// Bits per sample (e.g. 16, 24, 32).
    pub bits_per_sample: Option<u32>,
    /// List of extracted embedded cover artworks.
    pub covers: Vec<AudioCoverArt>,
    /// Full key-value dictionary of raw native tags.
    pub raw_tags: HashMap<String, String>,
}

/// High-performance metadata and embedded visual extractor.
pub struct AudioMetadataExtractor;

impl AudioMetadataExtractor {
    /// Extracts metadata and cover arts from in-memory audio bytes.
    pub fn extract_from_bytes(data: &[u8]) -> Result<AudioMetadata, AudioError> {
        Self::extract_from_bytes_with_hint(data, None)
    }

    /// Extracts metadata and cover arts from in-memory audio bytes with an optional format hint.
    pub fn extract_from_bytes_with_hint(
        data: &[u8],
        hint_str: Option<&str>,
    ) -> Result<AudioMetadata, AudioError> {
        if data.is_empty() {
            return Err(AudioError::InvalidParameter("Audio byte slice is empty".to_string()));
        }

        let cursor = Cursor::new(data.to_vec());
        let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

        let mut hint = Hint::new();
        if let Some(h) = hint_str {
            if h.contains('/') {
                hint.with_extension(h.split('/').next_back().unwrap_or(h));
            } else {
                hint.with_extension(h);
            }
        }

        Self::extract_from_media_source_stream(mss, &hint)
    }

    /// Extracts metadata and cover arts from a media file on disk.
    pub fn extract_from_file<P: AsRef<Path>>(path: P) -> Result<AudioMetadata, AudioError> {
        let p = path.as_ref();
        let file = File::open(p).map_err(AudioError::Io)?;

        let mut hint = Hint::new();
        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        Self::extract_from_media_source_stream(mss, &hint)
    }

    /// Internal extraction core running against probed media format reader.
    fn extract_from_media_source_stream(
        mss: MediaSourceStream,
        hint: &Hint,
    ) -> Result<AudioMetadata, AudioError> {
        let fmt_opts = FormatOptions::default();
        let meta_opts = MetadataOptions::default();

        let probed_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            symphonia::default::get_probe().format(hint, mss, &fmt_opts, &meta_opts)
        }));

        let mut probed = match probed_res {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => return Err(AudioError::UnsupportedFormat(e.to_string())),
            Err(_) => {
                return Err(AudioError::Format(
                    "Symphonia probe panicked on corrupted audio stream".to_string(),
                ))
            }
        };

        let meta_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut meta = Self::extract_from_format_reader(probed.format.as_mut());

            if let Some(meta_log) = probed.metadata.get() {
                if let Some(rev) = meta_log.current() {
                    Self::parse_tags_into_metadata(rev.tags(), &mut meta);
                    Self::parse_visuals_into_metadata(rev.visuals(), &mut meta);
                }
            }
            meta
        }));

        match meta_res {
            Ok(meta) => Ok(meta),
            Err(_) => Err(AudioError::Format(
                "Symphonia metadata extraction panicked on corrupted tags".to_string(),
            )),
        }
    }

    /// Extracts metadata and visuals from an active Symphonia [`FormatReader`].
    pub fn extract_from_format_reader(format_reader: &mut dyn FormatReader) -> AudioMetadata {
        let mut meta = AudioMetadata::default();

        // 1. Populate track stream properties
        if let Some(track) = format_reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        {
            let params = &track.codec_params;
            meta.sample_rate = params.sample_rate;
            meta.channels = params.channels.map(|c| c.count() as u32);
            meta.bits_per_sample = params.bits_per_sample;

            if let Some(n_frames) = params.n_frames {
                if let Some(tb) = params.time_base {
                    let time = tb.calc_time(n_frames);
                    meta.duration_seconds = Some(time.seconds as f64 + time.frac);
                } else if let Some(sr) = params.sample_rate {
                    if sr > 0 {
                        meta.duration_seconds = Some(n_frames as f64 / sr as f64);
                    }
                }
            }
        }

        // 2. Extract container metadata revisions & tags
        if let Some(metadata_log) = format_reader.metadata().current() {
            Self::parse_tags_into_metadata(metadata_log.tags(), &mut meta);
            Self::parse_visuals_into_metadata(metadata_log.visuals(), &mut meta);
        }

        meta
    }

    /// Parses tag entries into structured metadata fields.
    fn parse_tags_into_metadata(tags: &[Tag], meta: &mut AudioMetadata) {
        for tag in tags {
            let val_str = match &tag.value {
                Value::String(s) => s.trim_matches('\0').trim().to_string(),
                Value::SignedInt(i) => i.to_string(),
                Value::UnsignedInt(u) => u.to_string(),
                Value::Float(f) => f.to_string(),
                Value::Boolean(b) => b.to_string(),
                Value::Flag => "true".to_string(),
                Value::Binary(_) => continue,
            };

            if val_str.is_empty() {
                continue;
            }

            meta.raw_tags.insert(tag.key.clone(), val_str.clone());

            if let Some(std_key) = tag.std_key {
                match std_key {
                    StandardTagKey::TrackTitle if meta.title.is_none() => {
                        meta.title = Some(val_str);
                    }
                    StandardTagKey::Artist if meta.artist.is_none() => {
                        meta.artist = Some(val_str);
                    }
                    StandardTagKey::Album if meta.album.is_none() => {
                        meta.album = Some(val_str);
                    }
                    StandardTagKey::AlbumArtist if meta.album_artist.is_none() => {
                        meta.album_artist = Some(val_str);
                    }
                    StandardTagKey::Composer if meta.composer.is_none() => {
                        meta.composer = Some(val_str);
                    }
                    StandardTagKey::Genre if meta.genre.is_none() => {
                        meta.genre = Some(val_str);
                    }
                    StandardTagKey::Date | StandardTagKey::OriginalDate if meta.year.is_none() => {
                        meta.year = parse_year(&val_str);
                    }
                    StandardTagKey::TrackNumber if meta.track_number.is_none() => {
                        let (num, tot) = parse_number_and_total(&val_str);
                        meta.track_number = num;
                        if meta.total_tracks.is_none() && tot.is_some() {
                            meta.total_tracks = tot;
                        }
                    }
                    StandardTagKey::TrackTotal if meta.total_tracks.is_none() => {
                        meta.total_tracks = val_str.parse().ok();
                    }
                    StandardTagKey::DiscNumber if meta.disc_number.is_none() => {
                        let (num, tot) = parse_number_and_total(&val_str);
                        meta.disc_number = num;
                        if meta.total_discs.is_none() && tot.is_some() {
                            meta.total_discs = tot;
                        }
                    }
                    StandardTagKey::DiscTotal if meta.total_discs.is_none() => {
                        meta.total_discs = val_str.parse().ok();
                    }
                    StandardTagKey::Comment | StandardTagKey::Description if meta.comment.is_none() => {
                        meta.comment = Some(val_str);
                    }
                    StandardTagKey::Lyrics if meta.lyrics.is_none() => {
                        meta.lyrics = Some(val_str);
                    }
                    StandardTagKey::Bpm if meta.bpm.is_none() => {
                        meta.bpm = val_str.parse().ok();
                    }
                    _ => {}
                }
            } else {
                // Fallback: match uppercase raw keys for common tags
                let upper_key = tag.key.to_ascii_uppercase();
                match upper_key.as_str() {
                    "TITLE" | "TIT2" | "INAM" if meta.title.is_none() => {
                        meta.title = Some(val_str);
                    }
                    "ARTIST" | "TPE1" | "IART" if meta.artist.is_none() => {
                        meta.artist = Some(val_str);
                    }
                    "ALBUM" | "TALB" | "IPRD" if meta.album.is_none() => {
                        meta.album = Some(val_str);
                    }
                    "ALBUMARTIST" | "ALBUM ARTIST" | "TPE2" if meta.album_artist.is_none() => {
                        meta.album_artist = Some(val_str);
                    }
                    "GENRE" | "TCON" | "IGNR" if meta.genre.is_none() => {
                        meta.genre = Some(val_str);
                    }
                    "YEAR" | "TYER" | "TDRC" | "ICRD" if meta.year.is_none() => {
                        meta.year = parse_year(&val_str);
                    }
                    "COMMENT" | "COMM" | "ICMT" if meta.comment.is_none() => {
                        meta.comment = Some(val_str);
                    }
                    _ => {}
                }
            }
        }
    }

    /// Parses visual / cover art entries into embedded artwork collection.
    fn parse_visuals_into_metadata(
        visuals: &[symphonia::core::meta::Visual],
        meta: &mut AudioMetadata,
    ) {
        for visual in visuals {
            let data = visual.data.to_vec();
            if data.is_empty() {
                continue;
            }

            let mut mime_type = visual.media_type.clone();
            if mime_type.is_empty() {
                mime_type = detect_image_mime(&data);
            }

            let picture_type = visual
                .usage
                .map(|u| match format!("{u:?}").to_lowercase().as_str() {
                    "coverfront" | "frontcover" => AudioPictureType::CoverFront,
                    "coverback" | "backcover" => AudioPictureType::CoverBack,
                    "fileicon" => AudioPictureType::FileIcon,
                    "artist" | "leadartist" => AudioPictureType::Artist,
                    "composer" => AudioPictureType::Composer,
                    "bandlogo" => AudioPictureType::BandLogo,
                    "publisherlogo" => AudioPictureType::PublisherLogo,
                    "illustration" => AudioPictureType::Illustration,
                    _ => AudioPictureType::CoverFront,
                })
                .unwrap_or(AudioPictureType::CoverFront);

            let (sniffed_w, sniffed_h) = probe_image_dimensions(&data).unwrap_or((0, 0));
            let width = visual
                .dimensions
                .map(|d| d.width)
                .filter(|&w| w > 0)
                .or(if sniffed_w > 0 { Some(sniffed_w) } else { None });
            let height = visual
                .dimensions
                .map(|d| d.height)
                .filter(|&h| h > 0)
                .or(if sniffed_h > 0 { Some(sniffed_h) } else { None });

            let cover = AudioCoverArt {
                mime_type,
                picture_type,
                description: None,
                width,
                height,
                data,
            };

            meta.covers.push(cover);
        }
    }
}

/// Helper function to parse 4-digit release year from date strings (e.g. "2024-05-01", "1998").
fn parse_year(s: &str) -> Option<u32> {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 4 {
        let year_str = &digits[0..4];
        if let Ok(y) = year_str.parse::<u32>() {
            if (1000..=3000).contains(&y) {
                return Some(y);
            }
        }
    }
    None
}

/// Helper function to parse number and total (e.g. "5/12" -> (Some(5), Some(12))).
fn parse_number_and_total(s: &str) -> (Option<u32>, Option<u32>) {
    if let Some((left, right)) = s.split_once('/') {
        let num = left.trim().parse::<u32>().ok();
        let total = right.trim().parse::<u32>().ok();
        (num, total)
    } else {
        let num = s.trim().parse::<u32>().ok();
        (num, None)
    }
}

/// Sniffs image MIME type from binary magic bytes.
fn detect_image_mime(data: &[u8]) -> String {
    if data.starts_with(b"\xFF\xD8\xFF") {
        "image/jpeg".to_string()
    } else if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png".to_string()
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        "image/gif".to_string()
    } else if data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
        "image/webp".to_string()
    } else {
        "application/octet-stream".to_string()
    }
}

/// Pure Safe Rust image dimension inspector for JPEG, PNG, GIF, and WebP headers.
fn probe_image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 16 {
        return None;
    }

    // 1. PNG Header (IHDR chunk)
    if data.starts_with(b"\x89PNG\r\n\x1a\n") && data.len() >= 24 {
        let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        return Some((width, height));
    }

    // 2. GIF Header
    if (data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a")) && data.len() >= 10 {
        let width = u16::from_le_bytes([data[6], data[7]]) as u32;
        let height = u16::from_le_bytes([data[8], data[9]]) as u32;
        return Some((width, height));
    }

    // 3. WebP Header
    if data.starts_with(b"RIFF") && data.len() >= 30 && &data[8..12] == b"WEBP" {
        let chunk_fourcc = &data[12..16];
        if chunk_fourcc == b"VP8 " && data.len() >= 30 {
            // Lossy VP8 bitstream
            let width = (u16::from_le_bytes([data[26], data[27]]) & 0x3FFF) as u32;
            let height = (u16::from_le_bytes([data[28], data[29]]) & 0x3FFF) as u32;
            return Some((width, height));
        } else if chunk_fourcc == b"VP8L" && data.len() >= 25 {
            // Lossless VP8L
            if data[20] == 0x2F {
                let b1 = data[21] as u32;
                let b2 = data[22] as u32;
                let b3 = data[23] as u32;
                let b4 = data[24] as u32;
                let width = 1 + (b1 | ((b2 & 0x3F) << 8));
                let height = 1 + ((b2 >> 6) | (b3 << 2) | ((b4 & 0x0F) << 10));
                return Some((width, height));
            }
        }
    }

    // 4. JPEG SOF0 / SOF2 markers
    if data.starts_with(b"\xFF\xD8") {
        let mut idx = 2;
        while idx + 8 < data.len() {
            if data[idx] != 0xFF {
                idx += 1;
                continue;
            }
            let marker = data[idx + 1];
            if marker == 0xC0 || marker == 0xC2 {
                // SOF0 (Baseline) or SOF2 (Progressive)
                if idx + 8 < data.len() {
                    let height = u16::from_be_bytes([data[idx + 5], data[idx + 6]]) as u32;
                    let width = u16::from_be_bytes([data[idx + 7], data[idx + 8]]) as u32;
                    return Some((width, height));
                }
            }
            if idx + 3 < data.len() {
                let segment_len = u16::from_be_bytes([data[idx + 2], data[idx + 3]]) as usize;
                if segment_len < 2 {
                    break;
                }
                idx += 2 + segment_len;
            } else {
                break;
            }
        }
    }

    None
}
