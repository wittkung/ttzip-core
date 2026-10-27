# Data Model: Hybrid Semantic Search, `.ttvec` v2 & Snapshot Migration (Feature 232)

**Feature**: Hybrid Lexical-Semantic Search Engine with EmbeddingGemma 2 GGUF & `llama.cpp`  
**Location**: `products/ttzip/core/specs/232-embedding-semantic-search/data-model.md`  
**Updated**: 2026-10-18  
**Canonical Twin**: [`products/ttzip/docs/twin/semantic-search.md`](../../docs/twin/semantic-search.md)  

---

## 1. Snapshot Section ID Migration (`TT_SECTION_TYPE_EMBEDDINGS_V2 = 10`)

### 1.1 Legacy Collision Problem
In [`products/ttzip/core/rust/crates/ttzip-types/src/snapshot.rs`](../../rust/crates/ttzip-types/src/snapshot.rs), `TT_SECTION_TYPE_EMBEDDINGS = 9` collides with `TT_SECTION_TYPE_SUBTREE_END_IDS = 9`.

### 1.2 Safe Migration Contract
1. **New Canonical Section ID**:
   ```rust
   pub const TT_SECTION_TYPE_SUBTREE_END_IDS: u32 = 9;
   pub const TT_SECTION_TYPE_EMBEDDINGS_V2: u32 = 10;
   ```
2. **Backward-Compatible Reader Disambiguation**:
   - `SnapshotReader` checks for `TT_SECTION_TYPE_EMBEDDINGS_V2` (`10`) first.
   - If only section `9` exists, the reader inspects the first 16 bytes at `section.offset`:
     - Only if `u32::from_le_bytes(bytes[0..4]) == 0x5454_5645` (`"TTVE"`), `version >= 2`, and `model_fingerprint != 0` is section `9` treated as an embedding section; otherwise section `9` is unambiguously `TT_SECTION_TYPE_SUBTREE_END_IDS`.

---

## 2. Sidecar Vector & Segment Index Binary Geometry (`.ttvec` v2 / `.ttidx`)

Persisted under `~/Library/Caches/com.metastudyline.ttzip/vectors/<scope_id_blake3>.ttvec`:

```
Offset (Bytes)   Field Type   Description
00..04           u32          Magic = 0x54545645 ("TTVE" Little-Endian)
04..08           u32          Schema Version = 2 (v1 zero-filled files are rejected & unlinked)
08..12           u32          File/Node Count = N
12..16           u32          Segment Count = M (Tier-1 path segments + Tier-2 content chunks)
16..18           u16          Dimension = 128 (MRL-128)
18..19           u8           Quantization = 1 (Signed INT8, [-127, 127])
19..20           u8           Flags (Bit 0: Tier1Complete, Bit 1: Tier2Complete)
20..24           u32          Reserved (0)
24..32           u64          Model Fingerprint (0x914F_7F80_B114_6101 for EmbeddingGemma 2 v1)
32..64           [u8; 32]     Scope / Archive Content BLAKE3 Hash
64..96           [u8; 32]     Payload BLAKE3 Checksum (covers all bytes from offset 96..End)
96..End          Payload      Segment metadata table + contiguous CompactVector128 array (M * 128 B)
```

### 2.1 Validation & Corruption Recovery Invariants
- **Reject Legacy v1 & Zero Payloads**: Any file with `version < 2`, `model_fingerprint == 0`, `payload_blake3` mismatch, or any vector whose L1 norm $\sum |v_i| == 0$ is rejected on load (`IndexLoadStatus::CorruptedRebuilt`) and unlinked.
- **Model Upgrade Rebuild**: If `header.model_fingerprint != ACTIVE_MODEL_FINGERPRINT`, the loader returns `IndexLoadStatus::ModelUpgradedRebuilt` and triggers a clean re-index.

---

## 3. Core Domain Entities (Interface v1)

### 3.1 Model State & Metadata
```rust
pub const PINNED_MODEL_ID: &str = "google/embeddinggemma-2";
pub const PINNED_MODEL_REVISION: &str = "914f7f8";
pub const PINNED_GGUF_REPO: &str = "ggml-org/embeddinggemma-2-GGUF";
pub const PINNED_GGUF_REVISION: &str = "bfcd298762cc34d0357ece5ebdd31791a3a374d8";
pub const PINNED_GGUF_FILENAME: &str = "embeddinggemma-2-Q8_0.gguf";
pub const PINNED_GGUF_SIZE_BYTES: u64 = 309_855_456;
pub const PINNED_GGUF_SHA256: &str =
    "2188ac1deca4b77dffefd603c2776a9d76d9d74ec01841392982ebb840b09135";
pub const PINNED_LLAMA_CPP_TAG: &str = "b11461";
pub const PINNED_LLAMA_CPP_COMMIT: &str = "4d756bc72bf00a4aacf410ae15a2d315f3db400d";
pub const PINNED_MODEL_FINGERPRINT: u64 = 0x914F_7F80_B114_6101;
```

### 3.2 Segment & Hit Records
- **`SemanticSegmentKind`**:
  - `PathOnly`: Tier-1 relative path / filename embedding (`title: {path} | text: `)
  - `TextChunk`: Tier-2 plain text chunk (`1,800` chars, `200` overlap, `line_start..line_end`)
  - `CodeChunk`: Tier-2 source code chunk (`1,800` chars, `200` overlap, `line_start..line_end`, `definition_strength: 0..=2`)
- **`SemanticSearchHit`**:
  - `hit_id: String`
  - `scope_id: String`
  - `source_kind: SemanticScopeKind` (`Directory` | `OpenArchive`)
  - `container_path: String` (authorized root folder or `.zip`/`.7z` file path)
  - `relative_path: String` (file relative path or archive inner `entry_subpath`)
  - `match_kind: SemanticMatchKind` (`LexicalExact` | `PathSemantic` | `ContentSemantic` | `Hybrid`)
  - `snippet: Option<String>`
  - `line_start: Option<u32>`, `line_end: Option<u32>`
  - `lexical_score: f32`, `semantic_cosine: Option<f32>`, `fused_score: f32`
  - `index_version: u32`, `model_fingerprint: u64`
