// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Versioned `.ttvec` v2 + `.ttidx` persistent index store with 96-byte binary header,
//! BLAKE3 payload integrity verification, incremental file synchronization, and model upgrade rebuild.

use crate::chunker::ContentSupportTier;
use crate::model::{validate_non_zero_int8_vector, SemanticModelError, PINNED_MODEL_FINGERPRINT};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

pub const TT_EMBEDDING_MAGIC: u64 = 0x5454_454D_4231_0000; // "TTEMB1\0\0"
pub const TT_EMBEDDING_SCHEMA_VERSION_2: u32 = 2;
pub const TT_SECTION_TYPE_SUBTREE_END_IDS: u32 = 9;
pub const TT_SECTION_TYPE_EMBEDDINGS_V2: u32 = 10;
pub const INDEX_HEADER_V2_SIZE: usize = 96;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexHeaderV2 {
    pub magic: u64,
    pub schema_version: u32,
    pub dimension: u16,
    pub quant_type: u8,
    pub flags: u8,
    pub model_fingerprint: u64,
    pub gguf_sha256_prefix: [u8; 16],
    pub scope_fingerprint: u64,
    pub index_generation: u64,
    pub segment_count: u32,
    pub file_count: u32,
    pub payload_blake3: [u8; 32],
}

impl IndexHeaderV2 {
    pub fn to_bytes(&self) -> [u8; INDEX_HEADER_V2_SIZE] {
        let mut buf = [0u8; INDEX_HEADER_V2_SIZE];
        buf[0..8].copy_from_slice(&self.magic.to_le_bytes());
        buf[8..12].copy_from_slice(&self.schema_version.to_le_bytes());
        buf[12..14].copy_from_slice(&self.dimension.to_le_bytes());
        buf[14] = self.quant_type;
        buf[15] = self.flags;
        buf[16..24].copy_from_slice(&self.model_fingerprint.to_le_bytes());
        buf[24..40].copy_from_slice(&self.gguf_sha256_prefix);
        buf[40..48].copy_from_slice(&self.scope_fingerprint.to_le_bytes());
        buf[48..56].copy_from_slice(&self.index_generation.to_le_bytes());
        buf[56..60].copy_from_slice(&self.segment_count.to_le_bytes());
        buf[60..64].copy_from_slice(&self.file_count.to_le_bytes());
        buf[64..96].copy_from_slice(&self.payload_blake3);
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < INDEX_HEADER_V2_SIZE {
            return None;
        }
        let magic = u64::from_le_bytes(bytes[0..8].try_into().ok()?);
        let schema_version = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
        let dimension = u16::from_le_bytes(bytes[12..14].try_into().ok()?);
        let quant_type = bytes[14];
        let flags = bytes[15];
        let model_fingerprint = u64::from_le_bytes(bytes[16..24].try_into().ok()?);
        let mut gguf_sha256_prefix = [0u8; 16];
        gguf_sha256_prefix.copy_from_slice(&bytes[24..40]);
        let scope_fingerprint = u64::from_le_bytes(bytes[40..48].try_into().ok()?);
        let index_generation = u64::from_le_bytes(bytes[48..56].try_into().ok()?);
        let segment_count = u32::from_le_bytes(bytes[56..60].try_into().ok()?);
        let file_count = u32::from_le_bytes(bytes[60..64].try_into().ok()?);
        let mut payload_blake3 = [0u8; 32];
        payload_blake3.copy_from_slice(&bytes[64..96]);

        Some(Self {
            magic,
            schema_version,
            dimension,
            quant_type,
            flags,
            model_fingerprint,
            gguf_sha256_prefix,
            scope_fingerprint,
            index_generation,
            segment_count,
            file_count,
            payload_blake3,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedFileRecord {
    pub file_id: u32,
    pub relative_path: String,
    pub archive_entry_path: Option<String>,
    pub mtime_ns: i64,
    pub file_size: u64,
    pub content_blake3: [u8; 32],
    pub tier: ContentSupportTier,
}

/// Metadata for one file being inserted or refreshed, before an internal file ID is assigned.
#[derive(Clone, Debug)]
pub struct FileIndexInput {
    pub relative_path: String,
    pub archive_entry_path: Option<String>,
    pub mtime_ns: i64,
    pub file_size: u64,
    pub content_blake3: [u8; 32],
    pub tier: ContentSupportTier,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedSegmentRecord {
    pub segment_id: u32,
    pub file_id: u32,
    pub chunk_index: u32,
    pub is_path_segment: bool,
    pub line_start: u32,
    pub line_end: u32,
    pub definition_strength: u8,
    pub snippet_preview: String,
    pub vector_int8: Vec<i8>,
}

impl IndexedSegmentRecord {
    pub fn vector_array(&self) -> Option<[i8; 128]> {
        if self.vector_int8.len() != 128 {
            return None;
        }
        let mut arr = [0i8; 128];
        arr.copy_from_slice(&self.vector_int8);
        Some(arr)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexLoadOutcome {
    Loaded {
        index_generation: u64,
        file_count: usize,
        segment_count: usize,
    },
    NeedsRebuild {
        reason: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SidecarMetadataPayload {
    files: Vec<IndexedFileRecord>,
    segments_meta: Vec<SidecarSegmentMeta>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SidecarSegmentMeta {
    segment_id: u32,
    file_id: u32,
    chunk_index: u32,
    is_path_segment: bool,
    line_start: u32,
    line_end: u32,
    definition_strength: u8,
    snippet_preview: String,
}

#[derive(Clone, Debug)]
pub struct PersistentScopeIndex {
    pub scope_id: String,
    pub scope_fingerprint: u64,
    pub model_fingerprint: u64,
    pub index_generation: u64,
    next_file_id: u32,
    next_segment_id: u32,
    files_by_path: BTreeMap<String, IndexedFileRecord>,
    segments: Vec<IndexedSegmentRecord>,
}

impl PersistentScopeIndex {
    pub fn new(scope_id: impl Into<String>, scope_fingerprint: u64) -> Self {
        Self {
            scope_id: scope_id.into(),
            scope_fingerprint,
            model_fingerprint: PINNED_MODEL_FINGERPRINT,
            index_generation: 1,
            next_file_id: 1,
            next_segment_id: 1,
            files_by_path: BTreeMap::new(),
            segments: Vec::new(),
        }
    }

    pub fn files(&self) -> impl Iterator<Item = &IndexedFileRecord> {
        self.files_by_path.values()
    }

    pub fn file_by_id(&self, file_id: u32) -> Option<&IndexedFileRecord> {
        self.files_by_path.values().find(|f| f.file_id == file_id)
    }

    pub fn segments(&self) -> &[IndexedSegmentRecord] {
        &self.segments
    }

    /// Checks whether a file at `relative_path` is already up to date in the index.
    pub fn is_file_up_to_date(
        &self,
        relative_path: &str,
        mtime_ns: i64,
        file_size: u64,
        content_blake3: &[u8; 32],
    ) -> bool {
        self.files_by_path.get(relative_path).is_some_and(|rec| {
            rec.mtime_ns == mtime_ns
                && rec.file_size == file_size
                && &rec.content_blake3 == content_blake3
        })
    }

    /// Upserts a file and its non-zero 128-D INT8 segments, replacing any stale segments for that path.
    pub fn upsert_file_segments(
        &mut self,
        file: FileIndexInput,
        raw_segments: Vec<(bool, u32, u32, u8, String, [i8; 128])>,
    ) -> Result<u32, SemanticModelError> {
        let FileIndexInput {
            relative_path,
            archive_entry_path,
            mtime_ns,
            file_size,
            content_blake3,
            tier,
        } = file;
        for (_, _, _, _, _, vec_i8) in &raw_segments {
            validate_non_zero_int8_vector(vec_i8)?;
        }

        let file_id = if let Some(existing) = self.files_by_path.get(&relative_path) {
            let id = existing.file_id;
            self.segments.retain(|seg| seg.file_id != id);
            id
        } else {
            let id = self.next_file_id;
            self.next_file_id += 1;
            id
        };

        let record = IndexedFileRecord {
            file_id,
            relative_path: relative_path.clone(),
            archive_entry_path,
            mtime_ns,
            file_size,
            content_blake3,
            tier,
        };
        self.files_by_path.insert(relative_path, record);

        for (idx, (is_path_segment, line_start, line_end, def_strength, snippet, vec_i8)) in
            raw_segments.into_iter().enumerate()
        {
            let segment_id = self.next_segment_id;
            self.next_segment_id += 1;
            self.segments.push(IndexedSegmentRecord {
                segment_id,
                file_id,
                chunk_index: idx as u32,
                is_path_segment,
                line_start,
                line_end,
                definition_strength: def_strength,
                snippet_preview: snippet,
                vector_int8: vec_i8.to_vec(),
            });
        }
        self.index_generation = self.index_generation.saturating_add(1);
        Ok(file_id)
    }

    /// Removes a deleted file and all of its segments immediately.
    pub fn remove_file(&mut self, relative_path: &str) -> bool {
        if let Some(removed) = self.files_by_path.remove(relative_path) {
            self.segments.retain(|s| s.file_id != removed.file_id);
            self.index_generation = self.index_generation.saturating_add(1);
            true
        } else {
            false
        }
    }

    /// Clears the entire scope index (e.g., on scope revocation or model upgrade).
    pub fn clear(&mut self) {
        self.files_by_path.clear();
        self.segments.clear();
        self.index_generation = self.index_generation.saturating_add(1);
    }

    fn sidecar_metadata(&self) -> SidecarMetadataPayload {
        SidecarMetadataPayload {
            files: self.files_by_path.values().cloned().collect(),
            segments_meta: self
                .segments
                .iter()
                .map(|s| SidecarSegmentMeta {
                    segment_id: s.segment_id,
                    file_id: s.file_id,
                    chunk_index: s.chunk_index,
                    is_path_segment: s.is_path_segment,
                    line_start: s.line_start,
                    line_end: s.line_end,
                    definition_strength: s.definition_strength,
                    snippet_preview: s.snippet_preview.clone(),
                })
                .collect(),
        }
    }

    fn encoded_vectors(&self) -> Result<Vec<u8>, String> {
        let mut vec_payload = Vec::with_capacity(self.segments.len() * 128);
        for seg in &self.segments {
            if seg.vector_int8.len() != 128 || seg.vector_int8.iter().all(|&b| b == 0) {
                return Err("Refusing to persist zero or malformed 128-D vector".to_string());
            }
            for &b in &seg.vector_int8 {
                vec_payload.push(b as u8);
            }
        }

        Ok(vec_payload)
    }

    /// Saves `.ttvec` v2 binary vectors and `.ttidx` metadata atomically via `.tmp` rename.
    pub fn save_to_disk(&self, ttvec_path: &Path, ttidx_path: &Path) -> Result<(), String> {
        let meta_bytes = serde_json::to_vec(&self.sidecar_metadata()).map_err(|e| e.to_string())?;
        let vec_payload = self.encoded_vectors()?;

        let mut hasher = blake3::Hasher::new();
        hasher.update(&vec_payload);
        hasher.update(&meta_bytes);
        let payload_blake3: [u8; 32] = *hasher.finalize().as_bytes();

        let header = IndexHeaderV2 {
            magic: TT_EMBEDDING_MAGIC,
            schema_version: TT_EMBEDDING_SCHEMA_VERSION_2,
            dimension: 128,
            quant_type: 1,
            flags: 0b0000_0111,
            model_fingerprint: self.model_fingerprint,
            gguf_sha256_prefix: [
                0x21, 0x88, 0xac, 0x1d, 0xec, 0xa4, 0xb7, 0x7d, 0xff, 0xef, 0xd6, 0x03, 0xc2, 0x77,
                0x6a, 0x9d,
            ],
            scope_fingerprint: self.scope_fingerprint,
            index_generation: self.index_generation,
            segment_count: self.segments.len() as u32,
            file_count: self.files_by_path.len() as u32,
            payload_blake3,
        };

        let tmp_vec = ttvec_path.with_extension("ttvec.tmp");
        let tmp_idx = ttidx_path.with_extension("ttidx.tmp");

        if let Some(parent) = ttvec_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        {
            let mut f_vec = fs::File::create(&tmp_vec).map_err(|e| e.to_string())?;
            f_vec
                .write_all(&header.to_bytes())
                .map_err(|e| e.to_string())?;
            f_vec.write_all(&vec_payload).map_err(|e| e.to_string())?;
            f_vec.sync_all().map_err(|e| e.to_string())?;
        }
        {
            let mut f_idx = fs::File::create(&tmp_idx).map_err(|e| e.to_string())?;
            f_idx.write_all(&meta_bytes).map_err(|e| e.to_string())?;
            f_idx.sync_all().map_err(|e| e.to_string())?;
        }
        fs::rename(&tmp_vec, ttvec_path).map_err(|e| e.to_string())?;
        fs::rename(&tmp_idx, ttidx_path).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn read_index_payload(
        ttvec_path: &Path,
        ttidx_path: &Path,
        expected_model_fingerprint: u64,
    ) -> Result<(IndexHeaderV2, SidecarMetadataPayload, Vec<u8>), String> {
        let Ok(vec_file_bytes) = fs::read(ttvec_path) else {
            return Err("Missing .ttvec file".to_string());
        };
        let Ok(meta_bytes) = fs::read(ttidx_path) else {
            return Err("Missing .ttidx sidecar file".to_string());
        };
        let Some(header) = IndexHeaderV2::from_bytes(&vec_file_bytes) else {
            return Err("Truncated .ttvec header (< 96 bytes)".to_string());
        };
        if header.magic != TT_EMBEDDING_MAGIC {
            return Err("Invalid .ttvec magic header".to_string());
        }
        if header.schema_version != TT_EMBEDDING_SCHEMA_VERSION_2 {
            return Err(format!(
                "Incompatible schema version {} (expected {})",
                header.schema_version, TT_EMBEDDING_SCHEMA_VERSION_2
            ));
        }
        if header.model_fingerprint != expected_model_fingerprint {
            return Err(format!(
                "Model fingerprint changed: index={:#x}, expected={:#x}",
                header.model_fingerprint, expected_model_fingerprint
            ));
        }
        let vec_payload = &vec_file_bytes[INDEX_HEADER_V2_SIZE..];
        let expected_vec_bytes = (header.segment_count as usize) * 128;
        if vec_payload.len() != expected_vec_bytes {
            return Err(format!(
                "Vector payload length mismatch: expected {}, got {}",
                expected_vec_bytes,
                vec_payload.len()
            ));
        }

        let mut hasher = blake3::Hasher::new();
        hasher.update(vec_payload);
        hasher.update(&meta_bytes);
        if hasher.finalize().as_bytes() != &header.payload_blake3 {
            return Err(
                "BLAKE3 checksum mismatch (corrupted .ttvec or .ttidx payload)".to_string(),
            );
        }

        let Ok(meta_payload) = serde_json::from_slice::<SidecarMetadataPayload>(&meta_bytes) else {
            return Err("Malformed .ttidx JSON metadata".to_string());
        };
        if meta_payload.segments_meta.len() != header.segment_count as usize {
            return Err("Segment metadata count mismatch".to_string());
        }

        Ok((header, meta_payload, vec_file_bytes))
    }

    /// Loads `.ttvec` v2 + `.ttidx` from disk, verifying magic, schema version 2, model fingerprint,
    /// non-zero vectors, and BLAKE3 checksum. Returns `IndexLoadOutcome::NeedsRebuild` on any mismatch.
    pub fn load_from_disk(
        &mut self,
        ttvec_path: &Path,
        ttidx_path: &Path,
        expected_model_fingerprint: u64,
    ) -> IndexLoadOutcome {
        let (header, meta_payload, vec_file_bytes) =
            match Self::read_index_payload(ttvec_path, ttidx_path, expected_model_fingerprint) {
                Ok(payload) => payload,
                Err(reason) => return IndexLoadOutcome::NeedsRebuild { reason },
            };
        let vec_payload = &vec_file_bytes[INDEX_HEADER_V2_SIZE..];
        let mut loaded_segments = Vec::with_capacity(meta_payload.segments_meta.len());
        let mut max_seg_id = 0u32;
        for (i, meta) in meta_payload.segments_meta.into_iter().enumerate() {
            let offset = i * 128;
            let raw_u8 = &vec_payload[offset..offset + 128];
            let vec_i8: Vec<i8> = raw_u8.iter().map(|&b| b as i8).collect();
            if vec_i8.iter().all(|&b| b == 0) {
                return IndexLoadOutcome::NeedsRebuild {
                    reason: "Zero-filled vector detected in persisted index".to_string(),
                };
            }
            max_seg_id = max_seg_id.max(meta.segment_id);
            loaded_segments.push(IndexedSegmentRecord {
                segment_id: meta.segment_id,
                file_id: meta.file_id,
                chunk_index: meta.chunk_index,
                is_path_segment: meta.is_path_segment,
                line_start: meta.line_start,
                line_end: meta.line_end,
                definition_strength: meta.definition_strength,
                snippet_preview: meta.snippet_preview,
                vector_int8: vec_i8,
            });
        }

        let mut files_map = BTreeMap::new();
        let mut max_file_id = 0u32;
        for f in meta_payload.files {
            max_file_id = max_file_id.max(f.file_id);
            files_map.insert(f.relative_path.clone(), f);
        }

        self.model_fingerprint = header.model_fingerprint;
        self.index_generation = header.index_generation;
        self.files_by_path = files_map;
        self.segments = loaded_segments;
        self.next_file_id = max_file_id + 1;
        self.next_segment_id = max_seg_id + 1;

        IndexLoadOutcome::Loaded {
            index_generation: self.index_generation,
            file_count: self.files_by_path.len(),
            segment_count: self.segments.len(),
        }
    }
}
