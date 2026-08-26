// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! High-performance full-format file property and media metadata probing engine.

pub mod font;
pub mod image;
pub mod media;
pub mod model_3d;
pub mod types;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use memmap2::MmapOptions;

pub use font::*;
pub use image::*;
pub use media::*;
pub use model_3d::*;
pub use types::*;

use super::sniffer::detect_format_buffer;

/// Probes Adobe PDF format.
pub fn probe_pdf(data: &[u8]) -> Option<DocumentProbeResult> {
    if !data.starts_with(b"%PDF-") { return None; }
    let version = if data.len() >= 8 { Some(format!("PDF {}", String::from_utf8_lossy(&data[5..8]))) } else { Some("PDF".to_string()) };
    Some(DocumentProbeResult { format_name: "Adobe Portable Document Format (PDF)".into(), version, page_count: None, title: None, author: None })
}

fn wrap_img(img: ImageProbeResult, fmt: &'static str, mime: &'static str, size: u64) -> UnifiedMetadataProbe {
    let mut a = HashMap::new();
    a.insert("Format".into(), fmt.into());
    a.insert("MIME Type".into(), mime.into());
    a.insert("Dimensions".into(), format!("{} × {}", img.width, img.height));
    if let Some(ref cs) = img.color_space { a.insert("Color Space".into(), cs.clone()); }
    if let Some(ref m) = img.camera_model { a.insert("Camera".into(), m.clone()); }
    if let Some(ref l) = img.lens_model { a.insert("Lens".into(), l.clone()); }
    if let Some(iso) = img.iso_speed { a.insert("ISO".into(), iso.to_string()); }
    if let Some(f) = img.f_number { a.insert("Aperture".into(), format!("f/{f:.1}")); }
    if let Some(focal) = img.focal_length_mm { a.insert("Focal Length".into(), format!("{focal:.1}mm")); }
    if let Some(exp) = img.exposure_time_secs { a.insert("Exposure".into(), if exp > 0.0 && exp < 1.0 { format!("1/{:.0}s", 1.0 / exp) } else { format!("{exp:.2}s") }); }
    UnifiedMetadataProbe {
        media_type: MediaType::Image, format_name: fmt.into(), mime_type: mime.into(),
        file_size: size, is_container: false, image: Some(img), audio: None, video: None, font: None, model_3d: None, document: None, attributes: a,
    }
}

fn wrap_aud(aud: AudioProbeResult, fmt: &'static str, mime: &'static str, size: u64, is_cnt: bool) -> UnifiedMetadataProbe {
    let mut a = HashMap::new();
    a.insert("Format".into(), fmt.into());
    a.insert("MIME Type".into(), mime.into());
    a.insert("Sample Rate".into(), format!("{} Hz", aud.sample_rate));
    a.insert("Channels".into(), format!("{}", aud.channels));
    if aud.duration_secs > 0.0 { a.insert("Duration".into(), format!("{:.2}s", aud.duration_secs)); }
    if let Some(ref t) = aud.title { a.insert("Title".into(), t.clone()); }
    if let Some(ref art) = aud.artist { a.insert("Artist".into(), art.clone()); }
    UnifiedMetadataProbe {
        media_type: MediaType::Audio, format_name: fmt.into(), mime_type: mime.into(),
        file_size: size, is_container: is_cnt, image: None, audio: Some(aud), video: None, font: None, model_3d: None, document: None, attributes: a,
    }
}

/// Probes all file properties and media metadata directly from an in-memory byte buffer.
#[must_use]
pub fn probe_metadata_buffer(buffer: &[u8], filename_hint: Option<&str>, total_file_size: Option<u64>) -> UnifiedMetadataProbe {
    let file_size = total_file_size.unwrap_or(buffer.len() as u64);
    if buffer.is_empty() { return UnifiedMetadataProbe::unknown(file_size); }

    if let Some(img) = probe_jpeg(buffer) { return wrap_img(img, "JPEG Image", "image/jpeg", file_size); }
    if let Some(img) = probe_png(buffer) {
        let alpha = img.has_alpha;
        let mut p = wrap_img(img, "PNG Image", "image/png", file_size);
        p.attributes.insert("Alpha Channel".into(), if alpha { "Yes".into() } else { "No".into() });
        return p;
    }
    if let Some(img) = probe_webp(buffer) { return wrap_img(img, "WebP Image", "image/webp", file_size); }
    if let Some(img) = probe_gif(buffer) { return wrap_img(img, "GIF Image", "image/gif", file_size); }
    if let Some(img) = probe_bmp(buffer) { return wrap_img(img, "Windows Bitmap (BMP)", "image/bmp", file_size); }
    if let Some(img) = probe_ico(buffer) { return wrap_img(img, "Windows Icon (ICO)", "image/x-icon", file_size); }
    if let Some(img) = probe_psd(buffer) { return wrap_img(img, "Photoshop Document (PSD)", "image/vnd.adobe.photoshop", file_size); }
    if let Some(img) = probe_tiff(buffer) { return wrap_img(img, "TIFF Image", "image/tiff", file_size); }

    if let Some(iso) = probe_isobmff(buffer) {
        return match iso {
            IsobMffOutcome::Image(img, fmt, mime) => wrap_img(img, fmt, mime, file_size),
            IsobMffOutcome::Video(vid, fmt, mime) => {
                let mut a = HashMap::new();
                a.insert("Format".into(), fmt.into());
                a.insert("MIME Type".into(), mime.into());
                a.insert("Resolution".into(), format!("{} × {}", vid.width, vid.height));
                a.insert("Video Codec".into(), vid.video_codec.clone());
                if let Some(ref ac) = vid.audio_codec { a.insert("Audio Codec".into(), ac.clone()); }
                if vid.duration_secs > 0.0 { a.insert("Duration".into(), format!("{:.2}s", vid.duration_secs)); }
                UnifiedMetadataProbe {
                    media_type: MediaType::Video, format_name: fmt.into(), mime_type: mime.into(),
                    file_size, is_container: true, image: None, audio: None, video: Some(vid), font: None, model_3d: None, document: None, attributes: a,
                }
            }
            IsobMffOutcome::Audio(aud, fmt, mime) => wrap_aud(aud, fmt, mime, file_size, true),
        };
    }

    if let Some(ebml) = probe_ebml(buffer) { return ebml; }
    if let Some(flac) = probe_flac(buffer) { return wrap_aud(flac, "FLAC Audio", "audio/flac", file_size, false); }
    if let Some(wav) = probe_wav(buffer) { return wrap_aud(wav, "WAV Audio", "audio/wav", file_size, false); }
    if let Some(aiff) = probe_aiff(buffer) { return wrap_aud(aiff, "AIFF Audio", "audio/aiff", file_size, false); }
    if let Some(ogg) = probe_ogg(buffer) { return wrap_aud(ogg, "Ogg Audio", "audio/ogg", file_size, true); }

    if let Some(f) = probe_ttf_otf(buffer).or_else(|| probe_woff(buffer)).or_else(|| probe_woff2(buffer)) {
        let mut a = HashMap::new();
        a.insert("Format".into(), f.format_flavor.clone());
        a.insert("MIME Type".into(), "font/ttf".into());
        if let Some(ref fam) = f.font_family { a.insert("Family".into(), fam.clone()); }
        if let Some(ref sub) = f.font_subfamily { a.insert("Subfamily".into(), sub.clone()); }
        if let Some(ref ps) = f.postscript_name { a.insert("PostScript Name".into(), ps.clone()); }
        a.insert("Units Per Em".into(), f.units_per_em.to_string());
        a.insert("Glyphs".into(), f.num_glyphs.to_string());
        return UnifiedMetadataProbe {
            media_type: MediaType::Font, format_name: f.format_flavor.clone(), mime_type: "font/ttf".into(),
            file_size, is_container: false, image: None, audio: None, video: None, font: Some(f), model_3d: None, document: None, attributes: a,
        };
    }

    if let Some(m) = probe_glb_gltf(buffer).or_else(|| probe_stl(buffer)).or_else(|| probe_obj(buffer)).or_else(|| probe_ply(buffer)) {
        let mut a = HashMap::new();
        a.insert("Format".into(), m.format_name.clone());
        a.insert("MIME Type".into(), "model/gltf-binary".into());
        if let Some(tri) = m.triangle_count { a.insert("Triangles".into(), tri.to_string()); }
        if let Some(v) = m.vertex_count { a.insert("Vertices".into(), v.to_string()); }
        return UnifiedMetadataProbe {
            media_type: MediaType::Model3D, format_name: m.format_name.clone(), mime_type: "model/3d".into(),
            file_size, is_container: false, image: None, audio: None, video: None, font: None, model_3d: Some(m), document: None, attributes: a,
        };
    }

    if let Some(doc) = probe_pdf(buffer) {
        let mut a = HashMap::new();
        a.insert("Format".into(), doc.format_name.clone());
        a.insert("MIME Type".into(), "application/pdf".into());
        if let Some(ref v) = doc.version { a.insert("Version".into(), v.clone()); }
        return UnifiedMetadataProbe {
            media_type: MediaType::Document, format_name: doc.format_name.clone(), mime_type: "application/pdf".into(),
            file_size, is_container: false, image: None, audio: None, video: None, font: None, model_3d: None, document: Some(doc), attributes: a,
        };
    }

    if let Some(mp3) = probe_mp3(buffer) {
        let mut p = wrap_aud(mp3.clone(), "MP3 Audio", "audio/mpeg", file_size, false);
        p.attributes.insert("Bitrate".into(), format!("{} kbps", mp3.bitrate_kbps));
        return p;
    }

    let sniff = detect_format_buffer(buffer, filename_hint);
    if sniff.format != super::signatures::DetectedFormat::Unknown {
        let mut a = HashMap::new();
        a.insert("Format".into(), sniff.description.to_string());
        a.insert("MIME Type".into(), sniff.mime_type.to_string());
        a.insert("Is SFX".into(), if sniff.is_sfx { "Yes".into() } else { "No".into() });
        a.insert("Confidence".into(), format!("{}%", sniff.confidence));
        return UnifiedMetadataProbe {
            media_type: MediaType::Archive, format_name: sniff.description.to_string(), mime_type: sniff.mime_type.to_string(),
            file_size, is_container: true, image: None, audio: None, video: None, font: None, model_3d: None, document: None, attributes: a,
        };
    }

    UnifiedMetadataProbe::unknown(file_size)
}

/// Probes full-format file metadata from disk utilizing zero-copy memory mapping.
pub fn probe_metadata_file<P: AsRef<Path>>(path: P) -> io::Result<UnifiedMetadataProbe> {
    let p = path.as_ref();
    let file = File::open(p)?;
    let file_size = file.metadata()?.len();
    let hint = p.file_name().and_then(|n| n.to_str());
    if file_size == 0 { return Ok(UnifiedMetadataProbe::unknown(0)); }
    if let Ok(mmap) = unsafe { MmapOptions::new().map(&file) } {
        return Ok(probe_metadata_buffer(&mmap, hint, Some(file_size)));
    }
    let mut buffer = vec![0u8; (file_size as usize).min(65536)];
    std::io::BufReader::new(file).read_exact(&mut buffer)?;
    Ok(probe_metadata_buffer(&buffer, hint, Some(file_size)))
}

/// Alias for `probe_metadata_file` for API consistency across standards modules.
#[inline]
pub fn probe_file_metadata<P: AsRef<Path>>(path: P) -> io::Result<UnifiedMetadataProbe> {
    probe_metadata_file(path)
}
