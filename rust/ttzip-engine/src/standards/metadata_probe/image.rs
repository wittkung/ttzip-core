// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Zero-allocation image metadata and EXIF header parsing engine.

use super::types::ImageProbeResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Endianness { Little, Big }

struct ExifReader<'a> {
    data: &'a [u8],
    endian: Endianness,
}

impl<'a> ExifReader<'a> {
    fn new(data: &'a [u8]) -> Option<Self> {
        let raw = if data.starts_with(b"Exif\0\0") && data.len() >= 14 { &data[6..] } else { data };
        if raw.len() < 8 { return None; }
        let endian = match &raw[0..2] {
            b"II" => Endianness::Little,
            b"MM" => Endianness::Big,
            _ => return None,
        };
        let tag = match endian {
            Endianness::Little => u16::from_le_bytes([raw[2], raw[3]]),
            Endianness::Big => u16::from_be_bytes([raw[2], raw[3]]),
        };
        if tag != 42 && tag != 43 { return None; }
        Some(Self { data: raw, endian })
    }

    #[inline]
    fn read_u16(&self, off: usize) -> Option<u16> {
        let b = self.data.get(off..off + 2)?;
        Some(if self.endian == Endianness::Little { u16::from_le_bytes([b[0], b[1]]) } else { u16::from_be_bytes([b[0], b[1]]) })
    }

    #[inline]
    fn read_u32(&self, off: usize) -> Option<u32> {
        let b = self.data.get(off..off + 4)?;
        Some(if self.endian == Endianness::Little { u32::from_le_bytes([b[0], b[1], b[2], b[3]]) } else { u32::from_be_bytes([b[0], b[1], b[2], b[3]]) })
    }

    fn read_rational(&self, off: usize) -> Option<f64> {
        let (num, den) = (self.read_u32(off)?, self.read_u32(off + 4)?);
        if den == 0 { None } else { Some(num as f64 / den as f64) }
    }

    fn read_ascii_string(&self, off: usize, count: usize) -> Option<String> {
        let slice = self.data.get(off..off + count.min(256))?;
        let nul = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
        let s = String::from_utf8_lossy(&slice[..nul]).trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    }

    fn parse_ifd_entries(&self, mut off: usize, out: &mut ParsedExifTags, depth: usize) {
        if depth > 4 || off >= self.data.len() { return; }
        let num = match self.read_u16(off) { Some(n) => n as usize, None => return };
        off += 2;
        for _ in 0..num.min(128) {
            if off + 12 > self.data.len() { break; }
            let (tag, ftype) = (self.read_u16(off).unwrap_or(0), self.read_u16(off + 2).unwrap_or(0));
            let count = self.read_u32(off + 4).unwrap_or(0) as usize;
            let val_or_off = self.read_u32(off + 8).unwrap_or(0) as usize;
            let is_inline = (matches!(ftype, 1 | 2 | 7) && count <= 4) || (ftype == 3 && count <= 2) || (matches!(ftype, 4 | 9) && count <= 1);
            let d_off = if is_inline { off + 8 } else { val_or_off };

            match tag {
                0x0112 => out.orientation = self.read_u16(d_off).map(|v| v as u32),
                0x010F => out.make = self.read_ascii_string(d_off, count),
                0x0110 => out.model = self.read_ascii_string(d_off, count),
                0x0131 => out.software = self.read_ascii_string(d_off, count),
                0x0132 => out.date_time = self.read_ascii_string(d_off, count),
                0x8769 => self.parse_ifd_entries(val_or_off, out, depth + 1),
                0x829A => out.exposure_time = self.read_rational(d_off),
                0x829D => out.f_number = self.read_rational(d_off),
                0x8827 => out.iso = self.read_u16(d_off).map(|v| v as u32).or_else(|| self.read_u32(d_off)),
                0x9003 => out.date_time_original = self.read_ascii_string(d_off, count),
                0x920A => out.focal_length = self.read_rational(d_off),
                0xA001 => out.color_space = self.read_u16(d_off).map(|v| match v { 1 => "sRGB".to_string(), 2 => "Adobe RGB".to_string(), 65535 => "Uncalibrated".to_string(), _ => format!("ColorSpace ({v})") }),
                0xA002 => out.pixel_x = self.read_u32(d_off).or_else(|| self.read_u16(d_off).map(|v| v as u32)),
                0xA003 => out.pixel_y = self.read_u32(d_off).or_else(|| self.read_u16(d_off).map(|v| v as u32)),
                0xA434 => out.lens_model = self.read_ascii_string(d_off, count),
                _ => {}
            }
            off += 12;
        }
    }
}

#[derive(Default, Debug)]
struct ParsedExifTags {
    orientation: Option<u32>,
    make: Option<String>,
    model: Option<String>,
    software: Option<String>,
    date_time: Option<String>,
    date_time_original: Option<String>,
    exposure_time: Option<f64>,
    f_number: Option<f64>,
    iso: Option<u32>,
    focal_length: Option<f64>,
    color_space: Option<String>,
    pixel_x: Option<u32>,
    pixel_y: Option<u32>,
    lens_model: Option<String>,
}

impl ParsedExifTags {
    fn into_result(self, width: u32, height: u32, bit_depth: u32, has_alpha: bool, icc: Option<String>) -> ImageProbeResult {
        ImageProbeResult {
            width,
            height,
            orientation: self.orientation.unwrap_or(1),
            bit_depth,
            color_space: self.color_space.or_else(|| icc.clone()).or_else(|| Some("sRGB".to_string())),
            has_alpha,
            camera_make: self.make,
            camera_model: self.model,
            lens_model: self.lens_model,
            focal_length_mm: self.focal_length,
            f_number: self.f_number,
            exposure_time_secs: self.exposure_time,
            iso_speed: self.iso,
            date_time_original: self.date_time_original.or(self.date_time),
            icc_profile_name: icc,
        }
    }
}

fn parse_raw_exif_block(data: &[u8]) -> Option<ParsedExifTags> {
    let reader = ExifReader::new(data)?;
    let ifd0 = reader.read_u32(4)? as usize;
    let mut tags = ParsedExifTags::default();
    reader.parse_ifd_entries(ifd0, &mut tags, 0);
    Some(tags)
}

fn simple_img(w: u32, h: u32, depth: u32, color: &'static str, alpha: bool) -> ImageProbeResult {
    ImageProbeResult {
        width: w, height: h, orientation: 1, bit_depth: depth,
        color_space: Some(color.to_string()), has_alpha: alpha,
        camera_make: None, camera_model: None, lens_model: None, focal_length_mm: None,
        f_number: None, exposure_time_secs: None, iso_speed: None, date_time_original: None,
        icc_profile_name: None,
    }
}

/// Probes JPEG (SOF0..SOF15, APP1 EXIF, APP2 ICC).
pub fn probe_jpeg(data: &[u8]) -> Option<ImageProbeResult> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 || data[2] != 0xFF { return None; }
    let (mut offset, mut width, mut height, mut bit_depth) = (2, 0u32, 0u32, 8u32);
    let (mut exif_tags, mut icc_profile_name) = (None, None);

    while offset + 4 <= data.len() {
        if data[offset] != 0xFF { offset += 1; continue; }
        let marker = data[offset + 1];
        offset += 2;
        if marker == 0x00 || marker == 0xFF { continue; }
        if marker == 0xD8 || marker == 0xD9 || (0xD0..=0xD7).contains(&marker) {
            if marker == 0xD9 { break; }
            continue;
        }
        if offset + 2 > data.len() { break; }
        let seg_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
        if seg_len < 2 || offset + seg_len > data.len() { break; }
        let payload = &data[offset + 2..offset + seg_len];

        let is_sof = matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF);
        if is_sof && payload.len() >= 6 {
            bit_depth = payload[0] as u32;
            height = u16::from_be_bytes([payload[1], payload[2]]) as u32;
            width = u16::from_be_bytes([payload[3], payload[4]]) as u32;
        } else if marker == 0xE1 && payload.starts_with(b"Exif\0\0") {
            exif_tags = parse_raw_exif_block(payload);
        } else if marker == 0xE2 && payload.starts_with(b"ICC_PROFILE\0") && payload.len() >= 30 {
            icc_profile_name = Some(match &payload[14..18] {
                b"RGB " => "sRGB Profile", b"CMYK" => "CMYK Profile", b"GRAY" => "Grayscale Profile", _ => "ICC Profile",
            }.to_string());
        } else if marker == 0xDA {
            break;
        }
        offset += seg_len;
    }

    if width == 0 || height == 0 {
        if let Some(ref ex) = exif_tags {
            width = ex.pixel_x.unwrap_or(0);
            height = ex.pixel_y.unwrap_or(0);
        }
    }
    if width == 0 || height == 0 { return None; }
    let tags = exif_tags.unwrap_or_default();
    Some(tags.into_result(width, height, bit_depth, false, icc_profile_name))
}

/// Probes PNG (IHDR, eXIf, iCCP, sRGB).
pub fn probe_png(data: &[u8]) -> Option<ImageProbeResult> {
    if data.len() < 24 || !data.starts_with(b"\x89PNG\r\n\x1a\n") { return None; }
    let (mut offset, mut width, mut height, mut bit_depth, mut has_alpha) = (8, 0u32, 0u32, 8u32, false);
    let (mut exif_tags, mut icc_profile_name) = (None, None);

    while offset + 12 <= data.len() {
        let chunk_len = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]) as usize;
        let chunk_type = &data[offset + 4..offset + 8];
        let c_off = offset + 8;
        if c_off + chunk_len > data.len() { break; }
        let c_data = &data[c_off..c_off + chunk_len];

        if chunk_type == b"IHDR" && c_data.len() >= 13 {
            width = u32::from_be_bytes([c_data[0], c_data[1], c_data[2], c_data[3]]);
            height = u32::from_be_bytes([c_data[4], c_data[5], c_data[6], c_data[7]]);
            bit_depth = c_data[8] as u32;
            has_alpha = c_data[9] == 4 || c_data[9] == 6;
        } else if chunk_type == b"eXIf" {
            exif_tags = parse_raw_exif_block(c_data);
        } else if chunk_type == b"iCCP" && !c_data.is_empty() {
            if let Some(nul) = c_data.iter().position(|&b| b == 0) {
                if let Ok(name) = std::str::from_utf8(&c_data[..nul]) {
                    icc_profile_name = Some(name.to_string());
                }
            }
        } else if chunk_type == b"sRGB" {
            icc_profile_name = Some("sRGB".to_string());
        } else if chunk_type == b"IEND" {
            break;
        }
        offset += 12 + chunk_len;
    }

    if width == 0 || height == 0 { return None; }
    let tags = exif_tags.unwrap_or_default();
    Some(tags.into_result(width, height, bit_depth, has_alpha, icc_profile_name))
}

/// Probes WebP (VP8, VP8L, VP8X, EXIF, ICCP).
pub fn probe_webp(data: &[u8]) -> Option<ImageProbeResult> {
    if data.len() < 16 || !data.starts_with(b"RIFF") || &data[8..12] != b"WEBP" { return None; }
    let (mut offset, mut width, mut height, mut has_alpha) = (12, 0u32, 0u32, false);
    let (mut exif_tags, mut icc_profile_name) = (None, None);

    while offset + 8 <= data.len() {
        let chunk_fourcc = &data[offset..offset + 4];
        let chunk_size = u32::from_le_bytes([data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]]) as usize;
        let c_off = offset + 8;
        if c_off + chunk_size > data.len() { break; }
        let c_data = &data[c_off..c_off + chunk_size];

        if chunk_fourcc == b"VP8 " && c_data.len() >= 10 && c_data[3..6] == [0x9D, 0x01, 0x2A] {
            width = (u16::from_le_bytes([c_data[6], c_data[7]]) & 0x3FFF) as u32;
            height = (u16::from_le_bytes([c_data[8], c_data[9]]) & 0x3FFF) as u32;
        } else if chunk_fourcc == b"VP8L" && c_data.len() >= 5 && c_data[0] == 0x2F {
            let (b1, b2, b3, b4) = (c_data[1] as u32, c_data[2] as u32, c_data[3] as u32, c_data[4] as u32);
            width = 1 + (b1 | ((b2 & 0x3F) << 8));
            height = 1 + ((b2 >> 6) | (b3 << 2) | ((b4 & 0x0F) << 10));
            has_alpha = (b4 & 0x10) != 0;
        } else if chunk_fourcc == b"VP8X" && c_data.len() >= 10 {
            has_alpha = (c_data[0] & 0x10) != 0;
            width = 1 + (c_data[4] as u32 | ((c_data[5] as u32) << 8) | ((c_data[6] as u32) << 16));
            height = 1 + (c_data[7] as u32 | ((c_data[8] as u32) << 8) | ((c_data[9] as u32) << 16));
        } else if chunk_fourcc == b"EXIF" {
            exif_tags = parse_raw_exif_block(c_data);
        } else if chunk_fourcc == b"ICCP" {
            icc_profile_name = Some("Embedded ICC Profile".to_string());
        }
        offset += 8 + ((chunk_size + 1) & !1);
    }

    if width == 0 || height == 0 { return None; }
    let tags = exif_tags.unwrap_or_default();
    Some(tags.into_result(width, height, 8, has_alpha, icc_profile_name))
}

/// Probes GIF (GIF87a / GIF89a).
pub fn probe_gif(data: &[u8]) -> Option<ImageProbeResult> {
    if data.len() < 13 || (!data.starts_with(b"GIF87a") && !data.starts_with(b"GIF89a")) { return None; }
    let width = u16::from_le_bytes([data[6], data[7]]) as u32;
    let height = u16::from_le_bytes([data[8], data[9]]) as u32;
    let color_res = (((data[10] >> 4) & 0x07) + 1) as u32;
    Some(simple_img(width, height, color_res, "Indexed Color", true))
}

/// Probes BMP (BITMAPINFOHEADER / OS/2).
pub fn probe_bmp(data: &[u8]) -> Option<ImageProbeResult> {
    if data.len() < 26 || !data.starts_with(b"BM") { return None; }
    let header_size = u32::from_le_bytes([data[14], data[15], data[16], data[17]]);
    let (width, height, bit_depth) = if header_size >= 40 && data.len() >= 30 {
        let w = i32::from_le_bytes([data[18], data[19], data[20], data[21]]).unsigned_abs();
        let h = i32::from_le_bytes([data[22], data[23], data[24], data[25]]).unsigned_abs();
        (w, h, u16::from_le_bytes([data[28], data[29]]) as u32)
    } else if header_size == 12 && data.len() >= 24 {
        let w = u16::from_le_bytes([data[18], data[19]]) as u32;
        let h = u16::from_le_bytes([data[20], data[21]]) as u32;
        (w, h, u16::from_le_bytes([data[24], data[25]]) as u32)
    } else {
        return None;
    };
    Some(simple_img(width, height, bit_depth, "sRGB", bit_depth == 32))
}

/// Probes TIFF / BigTIFF.
pub fn probe_tiff(data: &[u8]) -> Option<ImageProbeResult> {
    let tags = parse_raw_exif_block(data)?;
    let (width, height) = (tags.pixel_x.unwrap_or(0), tags.pixel_y.unwrap_or(0));
    Some(tags.into_result(width, height, 8, false, None))
}

/// Probes Windows ICO / CUR icon directory.
pub fn probe_ico(data: &[u8]) -> Option<ImageProbeResult> {
    if data.len() < 22 || data[0] != 0 || data[1] != 0 || (data[2] != 1 && data[2] != 2) || data[3] != 0 { return None; }
    let count = u16::from_le_bytes([data[4], data[5]]) as usize;
    if count == 0 { return None; }
    let (mut max_w, mut max_h, mut max_bpp) = (0u32, 0u32, 0u32);
    for i in 0..count.min(64) {
        let off = 6 + i * 16;
        if off + 16 > data.len() { break; }
        let (raw_w, raw_h) = (data[off] as u32, data[off + 1] as u32);
        let (w, h) = (if raw_w == 0 { 256 } else { raw_w }, if raw_h == 0 { 256 } else { raw_h });
        let bpp = u16::from_le_bytes([data[off + 6], data[off + 7]]) as u32;
        if w * h >= max_w * max_h { max_w = w; max_h = h; max_bpp = bpp.max(max_bpp); }
    }
    Some(simple_img(max_w, max_h, if max_bpp == 0 { 32 } else { max_bpp }, "sRGB", true))
}

/// Probes Photoshop PSD / PSB format.
pub fn probe_psd(data: &[u8]) -> Option<ImageProbeResult> {
    if data.len() < 26 || !data.starts_with(b"8BPS") { return None; }
    let channels = u16::from_be_bytes([data[12], data[13]]) as u32;
    let height = u32::from_be_bytes([data[14], data[15], data[16], data[17]]);
    let width = u32::from_be_bytes([data[18], data[19], data[20], data[21]]);
    let depth = u16::from_be_bytes([data[22], data[23]]) as u32;
    let mode = u16::from_be_bytes([data[24], data[25]]);
    let color_space = match mode {
        0 => "Bitmap", 1 => "Grayscale", 2 => "Indexed", 3 => "RGB",
        4 => "CMYK", 7 => "Multichannel", 8 => "Duotone", 9 => "Lab",
        _ => "Photoshop Color",
    };
    Some(simple_img(width, height, depth, color_space, channels > 3))
}
