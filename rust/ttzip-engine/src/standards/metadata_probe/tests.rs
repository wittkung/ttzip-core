// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

use super::*;

#[test]
fn test_probe_jpeg_with_exif() {
    let mut jpeg = vec![0xFF, 0xD8];
    let mut exif = Vec::from(&b"Exif\0\0II\x2A\x00\x08\0\0\0\x04\0"[..]);
    // Tag 0x0112 (Orientation=6), 0x010F (Make=Sony@62), 0x0110 (Model=A7R5@68), Next IFD 0
    exif.extend_from_slice(&[0x12, 0x01, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0]);
    exif.extend_from_slice(&[0x0F, 0x01, 2, 0, 5, 0, 0, 0, 62, 0, 0, 0]);
    exif.extend_from_slice(&[0x10, 0x01, 2, 0, 5, 0, 0, 0, 68, 0, 0, 0]);
    exif.extend_from_slice(&[0u8; 12]);
    while exif.len() < 6 + 62 { exif.push(0); }
    exif.extend_from_slice(b"Sony\0");
    while exif.len() < 6 + 68 { exif.push(0); }
    exif.extend_from_slice(b"A7R5\0");

    let app1_len = (exif.len() + 2) as u16;
    jpeg.extend_from_slice(&[0xFF, 0xE1]);
    jpeg.extend_from_slice(&app1_len.to_be_bytes());
    jpeg.extend_from_slice(&exif);
    jpeg.extend_from_slice(&[0xFF, 0xC0, 0, 17, 8, 4, 0x38, 7, 0x80, 3, 1, 0x11, 0, 2, 0x11, 1, 3, 0x11, 1, 0xFF, 0xD9]);

    let probe = probe_metadata_buffer(&jpeg, Some("photo.jpg"), None);
    assert_eq!(probe.media_type, MediaType::Image);
    assert_eq!(probe.format_name, "JPEG Image");
    assert_eq!(probe.mime_type, "image/jpeg");
    let img = probe.image.expect("Image record expected");
    assert_eq!(img.width, 1920);
    assert_eq!(img.height, 1080);
    assert_eq!(img.orientation, 6);
    assert_eq!(img.camera_make.as_deref(), Some("Sony"));
    assert_eq!(img.camera_model.as_deref(), Some("A7R5"));
}

#[test]
fn test_probe_png_rgba() {
    let mut png = Vec::from(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR");
    png.extend_from_slice(&800u32.to_be_bytes());
    png.extend_from_slice(&600u32.to_be_bytes());
    png.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]); // RGBA + CRC

    let mut iccp = Vec::from(b"Display P3\0\0");
    iccp.extend_from_slice(&[0x78, 0x9C, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01]);
    png.extend_from_slice(&(iccp.len() as u32).to_be_bytes());
    png.extend_from_slice(b"iCCP");
    png.extend_from_slice(&iccp);
    png.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82]);

    let probe = probe_metadata_buffer(&png, Some("graphic.png"), None);
    assert_eq!(probe.media_type, MediaType::Image);
    assert_eq!(probe.format_name, "PNG Image");
    let img = probe.image.unwrap();
    assert_eq!(img.width, 800);
    assert_eq!(img.height, 600);
    assert!(img.has_alpha);
    assert_eq!(img.icc_profile_name.as_deref(), Some("Display P3"));
}

#[test]
fn test_probe_webp_lossy_and_lossless() {
    let mut webp = Vec::from(b"RIFF\x18\0\0\0WEBPVP8L\x05\0\0\0");
    let w: u32 = 99;
    let h: u32 = 199;
    let (b1, b2, b3, b4) = (
        (w & 0xFF) as u8,
        (((w >> 8) & 0x3F) | ((h & 0x03) << 6)) as u8,
        ((h >> 2) & 0xFF) as u8,
        0x10u8,
    );
    webp.extend_from_slice(&[0x2F, b1, b2, b3, b4, 0]);

    let probe = probe_metadata_buffer(&webp, Some("sticker.webp"), None);
    assert_eq!(probe.media_type, MediaType::Image);
    let img = probe.image.unwrap();
    assert_eq!(img.width, 100);
    assert_eq!(img.height, 200);
    assert!(img.has_alpha);
}

#[test]
fn test_probe_gif() {
    let gif = Vec::from(b"GIF89a\x40\x01\xF0\0\xF7\0\0");
    let probe = probe_metadata_buffer(&gif, Some("anim.gif"), None);
    assert_eq!(probe.media_type, MediaType::Image);
    let img = probe.image.unwrap();
    assert_eq!(img.width, 320);
    assert_eq!(img.height, 240);
    assert_eq!(img.bit_depth, 8);
}

#[test]
fn test_probe_bmp() {
    let mut bmp = Vec::from(b"BM\x46\0\0\0\0\0\0\0\x36\0\0\0\x28\0\0\0");
    bmp.extend_from_slice(&640i32.to_le_bytes());
    bmp.extend_from_slice(&480i32.to_le_bytes());
    bmp.extend_from_slice(&[1, 0, 24, 0]);

    let probe = probe_metadata_buffer(&bmp, Some("image.bmp"), None);
    assert_eq!(probe.media_type, MediaType::Image);
    let img = probe.image.unwrap();
    assert_eq!(img.width, 640);
    assert_eq!(img.height, 480);
    assert_eq!(img.bit_depth, 24);
}

#[test]
fn test_probe_ico() {
    let mut ico = Vec::from(&[0u8, 0, 1, 0, 2, 0][..]);
    // 32x32 32bpp
    ico.extend_from_slice(&[32, 32, 0, 0, 1, 0, 32, 0, 0xE8, 3, 0, 0, 0x26, 0, 0, 0]);
    // 256x256 (0) 32bpp
    ico.extend_from_slice(&[0, 0, 0, 0, 1, 0, 32, 0, 0x88, 0x13, 0, 0, 0x0E, 4, 0, 0]);

    let probe = probe_metadata_buffer(&ico, Some("app.ico"), None);
    assert_eq!(probe.media_type, MediaType::Image);
    let img = probe.image.unwrap();
    assert_eq!(img.width, 256);
    assert_eq!(img.height, 256);
}

#[test]
fn test_probe_psd() {
    let psd = Vec::from(b"8BPS\0\x01\0\0\0\0\0\0\0\x04\0\0\x04\0\0\0\x08\0\0\x08\0\x03");
    let probe = probe_metadata_buffer(&psd, Some("project.psd"), None);
    assert_eq!(probe.media_type, MediaType::Image);
    let img = probe.image.unwrap();
    assert_eq!(img.width, 2048);
    assert_eq!(img.height, 1024);
    assert_eq!(img.bit_depth, 8);
    assert!(img.has_alpha);
}

#[test]
fn test_probe_wav_audio() {
    let mut wav = Vec::from(b"RIFF,\0\0\0WAVEfmt \x10\0\0\0\x01\0\x02\0\x80\xBB\0\0\0\xEE\x02\0\x04\0\x10\0data");
    wav.extend_from_slice(&960000u32.to_le_bytes()); // 5 secs

    let probe = probe_metadata_buffer(&wav, Some("sound.wav"), None);
    assert_eq!(probe.media_type, MediaType::Audio);
    assert_eq!(probe.format_name, "WAV Audio");
    let aud = probe.audio.unwrap();
    assert_eq!(aud.sample_rate, 48000);
    assert_eq!(aud.channels, 2);
    assert_eq!(aud.bit_depth, 16);
    assert_eq!(aud.duration_secs, 5.0);
}

#[test]
fn test_probe_flac_audio() {
    let mut flac = Vec::from(b"fLaC\x80\0\0\x22");
    let mut sinfo = vec![0u8; 34];
    sinfo[10] = (44100 >> 12) as u8;
    sinfo[11] = ((44100 >> 4) & 0xFF) as u8;
    sinfo[12] = (((44100 & 0x0F) as u8) << 4) | (1 << 1);
    sinfo[13] = 15 << 4;
    sinfo[15] = ((441000u32 >> 16) & 0xFF) as u8;
    sinfo[16] = ((441000u32 >> 8) & 0xFF) as u8;
    sinfo[17] = (441000u32 & 0xFF) as u8;
    flac.extend_from_slice(&sinfo);

    let probe = probe_metadata_buffer(&flac, Some("track.flac"), None);
    assert_eq!(probe.media_type, MediaType::Audio);
    let aud = probe.audio.unwrap();
    assert_eq!(aud.sample_rate, 44100);
    assert_eq!(aud.channels, 2);
    assert_eq!(aud.bit_depth, 16);
    assert_eq!(aud.duration_secs, 10.0);
}

#[test]
fn test_probe_aiff_audio() {
    let aiff = Vec::from(b"FORM\0\0\0,AIFFCOMM\0\0\0\x12\0\x02\0\x06\xBA\xA8\0\x10\x40\x0E\xAC\x44\0\0\0\0\0\0");
    let probe = probe_metadata_buffer(&aiff, Some("audio.aiff"), None);
    assert_eq!(probe.media_type, MediaType::Audio);
    let aud = probe.audio.unwrap();
    assert_eq!(aud.channels, 2);
    assert_eq!(aud.sample_rate, 44100);
    assert_eq!(aud.duration_secs, 10.0);
}

#[test]
fn test_probe_mp4_video() {
    let mut mp4 = Vec::from(b"\0\0\0\x18ftypmp42\0\0\0\0isommp42");
    let mut mvhd = vec![0u8; 100];
    mvhd[12..20].copy_from_slice(&[0, 0, 3, 0xE8, 0, 0, 0x30, 0xD4]); // 1000 ts, 12500 dur

    let mut moov = Vec::new();
    moov.extend_from_slice(&(108u32).to_be_bytes());
    moov.extend_from_slice(b"mvhd");
    moov.extend_from_slice(&mvhd);

    let mut trak = Vec::new();
    let mut tkhd = vec![0u8; 84];
    tkhd[40..44].copy_from_slice(&0x0001_0000i32.to_be_bytes());
    tkhd[56..60].copy_from_slice(&0x0001_0000i32.to_be_bytes());
    tkhd[76..80].copy_from_slice(&(3840u32 << 16).to_be_bytes());
    tkhd[80..84].copy_from_slice(&(2160u32 << 16).to_be_bytes());
    trak.extend_from_slice(&(92u32).to_be_bytes());
    trak.extend_from_slice(b"tkhd");
    trak.extend_from_slice(&tkhd);

    let stsd = Vec::from(b"\0\0\0\0\0\0\0\x01\0\0\0\x10avc1\0\0\0\0\0\0\0\0");
    let mut stbl = Vec::from(&(stsd.len() as u32 + 8).to_be_bytes()[..]);
    stbl.extend_from_slice(b"stsd");
    stbl.extend_from_slice(&stsd);

    let mut minf = Vec::from(&(stbl.len() as u32 + 8).to_be_bytes()[..]);
    minf.extend_from_slice(b"stbl");
    minf.extend_from_slice(&stbl);

    let mut mdia = Vec::from(&(minf.len() as u32 + 8).to_be_bytes()[..]);
    mdia.extend_from_slice(b"minf");
    mdia.extend_from_slice(&minf);

    trak.extend_from_slice(&(mdia.len() as u32 + 8).to_be_bytes());
    trak.extend_from_slice(b"mdia");
    trak.extend_from_slice(&mdia);

    moov.extend_from_slice(&(trak.len() as u32 + 8).to_be_bytes());
    moov.extend_from_slice(b"trak");
    moov.extend_from_slice(&trak);

    mp4.extend_from_slice(&(moov.len() as u32 + 8).to_be_bytes());
    mp4.extend_from_slice(b"moov");
    mp4.extend_from_slice(&moov);

    let probe = probe_metadata_buffer(&mp4, Some("movie.mp4"), None);
    assert_eq!(probe.media_type, MediaType::Video);
    assert_eq!(probe.format_name, "MPEG-4 Video");
    let vid = probe.video.unwrap();
    assert_eq!(vid.width, 3840);
    assert_eq!(vid.height, 2160);
    assert_eq!(vid.duration_secs, 12.5);
    assert_eq!(vid.video_codec, "H.264 / AVC");
}

#[test]
fn test_probe_mp3_with_id3() {
    let mut mp3 = Vec::from(b"ID3\x04\x00\x00\0\0\0\x1eTIT2\0\0\0\x0a\0\0\0Test Song");
    while mp3.len() < 40 { mp3.push(0); }
    mp3.extend_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
    mp3.extend_from_slice(&[0u8; 1000]);

    let probe = probe_metadata_buffer(&mp3, Some("music.mp3"), None);
    assert_eq!(probe.media_type, MediaType::Audio);
    let aud = probe.audio.unwrap();
    assert_eq!(aud.sample_rate, 44100);
    assert_eq!(aud.bitrate_kbps, 128);
    assert_eq!(aud.channels, 2);
    assert_eq!(aud.title.as_deref(), Some("Test Song"));
}

#[test]
fn test_probe_3d_stl_and_glb() {
    let mut stl = vec![0u8; 84 + 100 * 50];
    stl[0..80].fill(b'A');
    stl[80..84].copy_from_slice(&100u32.to_le_bytes());

    let probe_stl = probe_metadata_buffer(&stl, Some("model.stl"), None);
    assert_eq!(probe_stl.media_type, MediaType::Model3D);
    let m3d = probe_stl.model_3d.unwrap();
    assert_eq!(m3d.triangle_count, Some(100));
    assert_eq!(m3d.vertex_count, Some(300));

    let glb = Vec::from(b"glTF\x02\0\0\0\x28\0\0\0\x10\0\0\0JSON{\"asset\":{\"version\":\"2.0\"}} ");
    let probe_glb = probe_metadata_buffer(&glb, Some("scene.glb"), None);
    assert_eq!(probe_glb.media_type, MediaType::Model3D);
    assert!(probe_glb.format_name.contains("GLB"));
}

#[test]
fn test_probe_3d_obj_and_ply() {
    let obj_data = b"# OBJ file\nv 0.0 1.0 0.0\nv 1.0 0.0 0.0\nv -1.0 0.0 0.0\nf 1 2 3\n";
    let probe_obj = probe_metadata_buffer(obj_data, Some("mesh.obj"), None);
    assert_eq!(probe_obj.media_type, MediaType::Model3D);
    let m3d = probe_obj.model_3d.unwrap();
    assert_eq!(m3d.vertex_count, Some(3));
    assert_eq!(m3d.triangle_count, Some(1));

    let ply_data = b"ply\nformat ascii 1.0\nelement vertex 42\nelement face 80\nend_header\n";
    let probe_ply = probe_metadata_buffer(ply_data, Some("pointcloud.ply"), None);
    assert_eq!(probe_ply.media_type, MediaType::Model3D);
    let m3d_ply = probe_ply.model_3d.unwrap();
    assert_eq!(m3d_ply.vertex_count, Some(42));
    assert_eq!(m3d_ply.triangle_count, Some(80));
}

#[test]
fn test_probe_font_ttf() {
    let mut ttf = Vec::from(b"\x00\x01\x00\x00\0\x02\0\0\0\0\0\0head\0\0\0\0\0\0\0,\0\0\x006maxp\0\0\0\0\0\0\0b\0\0\0 ");
    let mut head = vec![0u8; 54];
    head[18..20].copy_from_slice(&2048u16.to_be_bytes());
    ttf.extend_from_slice(&head);
    let mut maxp = vec![0u8; 32];
    maxp[4..6].copy_from_slice(&1240u16.to_be_bytes());
    ttf.extend_from_slice(&maxp);

    let probe = probe_metadata_buffer(&ttf, Some("font.ttf"), None);
    assert_eq!(probe.media_type, MediaType::Font);
    let f = probe.font.unwrap();
    assert_eq!(f.units_per_em, 2048);
    assert_eq!(f.num_glyphs, 1240);
    assert_eq!(f.format_flavor, "TrueType (TTF)");
}

#[test]
fn test_probe_pdf_document() {
    let pdf_data = b"%PDF-1.7\n1 0 obj\n<< /Type /Catalog >>\nendobj\n";
    let probe = probe_metadata_buffer(pdf_data, Some("doc.pdf"), None);
    assert_eq!(probe.media_type, MediaType::Document);
    assert_eq!(probe.format_name, "Adobe Portable Document Format (PDF)");
    let doc = probe.document.unwrap();
    assert_eq!(doc.version.as_deref(), Some("PDF 1.7"));
}

#[test]
fn test_probe_corrupted_and_empty_inputs() {
    let empty = probe_metadata_buffer(&[], None, None);
    assert_eq!(empty.media_type, MediaType::Unknown);

    let junk = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04];
    let probe_junk = probe_metadata_buffer(&junk, None, None);
    assert_eq!(probe_junk.media_type, MediaType::Unknown);
}
