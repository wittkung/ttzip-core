// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! 3D Mesh and scene file header probing (glTF, GLB, STL, OBJ, PLY).

use super::types::Model3DProbeResult;

/// Probes glTF / GLB 3D format.
pub fn probe_glb_gltf(data: &[u8]) -> Option<Model3DProbeResult> {
    if data.len() >= 12 && data.starts_with(b"glTF") {
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let chunk_len = if data.len() >= 16 {
            u32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize
        } else {
            0
        };
        let chunk_type = if data.len() >= 20 { &data[16..20] } else { b"" };

        let mut generator = None;
        if chunk_type == b"JSON" && 20 + chunk_len <= data.len() {
            if let Ok(json_str) = std::str::from_utf8(&data[20..20 + chunk_len.min(4096)]) {
                if let Some(pos) = json_str.find("\"generator\"") {
                    let rest = &json_str[pos + 11..];
                    if let Some(start) = rest.find('"') {
                        if let Some(end) = rest[start + 1..].find('"') {
                            generator = Some(rest[start + 1..start + 1 + end].to_string());
                        }
                    }
                }
            }
        }

        return Some(Model3DProbeResult {
            format_name: format!("glTF Binary (GLB v{version})"),
            triangle_count: None,
            vertex_count: None,
            generator_version: generator,
        });
    }

    if data.starts_with(b"{") && data.windows(7).any(|w| w == b"\"asset\"") {
        return Some(Model3DProbeResult {
            format_name: "glTF JSON 3D Model".to_string(),
            triangle_count: None,
            vertex_count: None,
            generator_version: Some("glTF 2.0".to_string()),
        });
    }

    None
}

/// Probes STL (Stereolithography 3D Mesh).
pub fn probe_stl(data: &[u8]) -> Option<Model3DProbeResult> {
    if data.len() >= 84 {
        let tri_count = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as u64;
        let expected_size = 84 + tri_count * 50;

        if (data.len() as u64) == expected_size || (tri_count > 0 && tri_count < 100_000_000 && !data[0..80].contains(&0)) {
            return Some(Model3DProbeResult {
                format_name: "Binary STL 3D Mesh".to_string(),
                triangle_count: Some(tri_count),
                vertex_count: Some(tri_count * 3),
                generator_version: None,
            });
        }
    }

    if data.starts_with(b"solid ") || data.starts_with(b"solid\n") || data.starts_with(b"solid\r\n") {
        return Some(Model3DProbeResult {
            format_name: "ASCII STL 3D Mesh".to_string(),
            triangle_count: None,
            vertex_count: None,
            generator_version: None,
        });
    }

    None
}

/// Probes Wavefront OBJ 3D Model.
pub fn probe_obj(data: &[u8]) -> Option<Model3DProbeResult> {
    if data.starts_with(b"#") || data.starts_with(b"v ") || data.starts_with(b"mtllib ") || data.starts_with(b"o ") {
        let mut v_count = 0u64;
        let mut f_count = 0u64;

        for line in data.split(|&b| b == b'\n').take(5000) {
            if line.starts_with(b"v ") {
                v_count += 1;
            } else if line.starts_with(b"f ") {
                f_count += 1;
            }
        }

        if v_count > 0 || f_count > 0 {
            return Some(Model3DProbeResult {
                format_name: "Wavefront OBJ 3D Model".to_string(),
                triangle_count: if f_count > 0 { Some(f_count) } else { None },
                vertex_count: if v_count > 0 { Some(v_count) } else { None },
                generator_version: None,
            });
        }
    }

    None
}

/// Probes PLY (Polygon File Format).
pub fn probe_ply(data: &[u8]) -> Option<Model3DProbeResult> {
    if !data.starts_with(b"ply\n") && !data.starts_with(b"ply\r\n") {
        return None;
    }

    let mut v_count = None;
    let mut f_count = None;

    if let Ok(header) = std::str::from_utf8(&data[..data.len().min(1024)]) {
        for line in header.lines() {
            if let Some(rest) = line.strip_prefix("element vertex ") {
                v_count = rest.trim().parse::<u64>().ok();
            } else if let Some(rest) = line.strip_prefix("element face ") {
                f_count = rest.trim().parse::<u64>().ok();
            }
        }
    }

    Some(Model3DProbeResult {
        format_name: "Polygon File Format (PLY)".to_string(),
        triangle_count: f_count,
        vertex_count: v_count,
        generator_version: None,
    })
}
