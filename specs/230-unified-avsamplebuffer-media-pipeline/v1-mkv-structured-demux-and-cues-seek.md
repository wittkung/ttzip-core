# V1: MKV Structured Track Parsing & Cues/SeekHead Keyframe Seeking (2026-10-17)

**Feature Directory**: `products/ttzip/core/specs/230-unified-avsamplebuffer-media-pipeline`

**Status**: Completed & Verified

**Exclusive Source Scope**:
- `infra/ttmedia/src/demux/mod.rs`
- `infra/ttmedia/src/demux/ebml.rs`
- `infra/ttmedia/src/demux/mkv_tracks.rs`
- `infra/ttmedia/src/demux/mkv_index.rs`
- `infra/ttmedia/src/demux/mkv_fallback_seek.rs`
- `infra/ttmedia/src/demux/format_demuxer.rs`
- `infra/ttmedia/src/demux/mkv_demux_tests.rs`
- `infra/ttmedia/src/demux/mkv_seek_tests.rs`

---

## 1. Problem Statement & Root Causes

Prior to V1, `infra/ttmedia/src/demux/format_demuxer.rs` relied on ad-hoc byte pattern scanning over a fixed 128 KiB head buffer and a sequential packet-by-packet file scan for seeking:

1. **Global Byte-Search & Field-Order Assumptions in `TrackEntry`**:
   - Video codec detection scanned the entire 128 KiB head buffer with `windows(...)` for `"V_MPEGH/ISO/HEVC"`, `"V_MPEG4/ISO/AVC"`, etc., defaulting blindly to `CodecId::H264`.
   - `CodecPrivate` (`0x63A2`) was matched globally across the first 128 KiB and assigned to the video track's `extradata`, while `audio_extradata` was hardcoded to `None`. If a video track lacked `CodecPrivate` or appeared after another track with `CodecPrivate`, foreign `CodecPrivate` bytes leaked across tracks, and audio tracks (`A_AAC`, `A_OPUS`, `A_FLAC`) always lost their `CodecPrivate`.
   - Audio track discovery searched for `0x83 0x81 <type>` (`TrackType`), scanned backward up to 64 bytes for `0xD7` (`TrackNumber`), and scanned forward up to 256 bytes (or until any `0xAE` byte inside payload data) for `"A_AC3"`, `0xB5` (`SamplingFrequency`), and `0x9F` (`Channels`).
   - In real Blu-ray REMUX / Matroska muxes such as `REF-V6` (`Dead Poets Society`), `CodecID` (`0x86`) is written **before** `TrackType` (`0x83`) inside `TrackEntry` (`0xD7 -> 0x73C5 -> 0x9C -> 0x536E -> 0x22B59C -> 0x88 -> 0x86 -> 0x83 -> 0x23E383 -> 0xE1`). Because `0x86` preceded `0x83`, the forward scan missed `"A_AC3"` on Tracks #4–#7 and fell into the fallback branch on Track #2 (`A_DTS`).
2. **Disguising Unknown Codecs as `CodecId::Aac`**:
   - When an audio `TrackEntry`'s `CodecID` did not match the hardcoded forward-scan list (e.g., `"A_DTS"` or when `0x86` preceded `0x83`), `format_demuxer.rs` executed `audio_codec_id = CodecId::Aac`, mislabeling unsupported `A_DTS` tracks as `AAC`.
3. **Linear Full-File Scan on `seek()`**:
   - `FormatDemuxer::seek` always rewound to byte `0` and stepped through every EBML element—including every `SimpleBlock` (`0xA3`) and `Block` (`0xA1`) inside every `Cluster` (`0x1F43B675`)—from the beginning of the file. On multi-gigabyte 4K / Blu-ray REMUX files (`REF-V4` 19.92 GB, `REF-V5` 20.58 GB, `REF-V6` 29.45 GB), seeking to a mid-movie timestamp required scanning gigabytes of clusters without consulting `SeekHead` (`0x114D9B74`) or `Cues` (`0x1C53BB6B`), and lacked any fallback budget, cancellation mechanism, or I/O telemetry.

---

## 2. Structured EBML & `TrackEntry` Parsing Architecture (`ebml.rs`)

### 2.1 Positioned Buffered EBML Reader & Bounds Safety
- `EbmlReader` uses positioned file I/O (`FileExt::read_at`) backed by a 64 KiB sliding window cache and exact physical I/O counters (`bytes_read`, `read_calls`).
- Every EBML Variable Size Integer (VINT) for element IDs (`1..=4` bytes, marker bit preserved) and data sizes (`1..=8` bytes, marker bit stripped, unknown-size sentinel `(1 << (7 * width)) - 1` detected) is validated against integer overflow (`checked_add`) and strictly bounded by its enclosing parent master element's `[start, end)` range and `file_len`.
- Corrupt or truncated headers, oversized integer/float fields, element boundaries exceeding parent limits, and payloads exceeding safety caps (`MAX_CODEC_PRIVATE_BYTES = 16 MiB`, `MAX_CUES_BYTES = 64 MiB`, `MAX_PACKET_PAYLOAD_BYTES = 64 MiB`) return `DemuxError::CorruptData` before any unbounded memory allocation.

### 2.2 Hierarchy-Scoped `Segment`, `SeekHead`, `Info`, and `Tracks` Parsing
- Top-level `EBML` (`0x1A45DFA3`) and `Segment` (`0x18538067`) headers are parsed first, recording `segment_data_start` and `segment_end`.
- `SeekHead` (`0x114D9B74`) entries (`Seek` `0x4DBB` -> `SeekID` `0x53AB`, `SeekPosition` `0x53AC` in quelconque child order) resolve relative to `segment_data_start`. Chained `SeekHead` references are followed up to 4 hops with a visited-offset set to prevent cycles on malformed files.
- `Info` (`0x1549A966`) parses `TimecodeScale` (`0x2AD7B1`, default `1_000_000` ns) and `Duration` (`0x4489`, 4-byte `f32` or 8-byte `f64`).
- `Tracks` (`0x1654AE6B`) iterates only `TrackEntry` (`0xAE`) direct children. Inside each `TrackEntry`, fields are accumulated into an isolated `MkvTrackEntry` regardless of element ordering:
  - `0xD7` (`TrackNumber`), `0x73C5` (`TrackUID`), `0x83` (`TrackType`: `1` Video, `2` Audio, `17` Subtitle)
  - `0xB9` (`FlagEnabled`, default `true`), `0x88` (`FlagDefault`, default `true`), `0x55AA` (`FlagForced`, default `false`)
  - `0x86` (`CodecID`), `0x63A2` (`CodecPrivate` owned strictly by this track), `0x23E383` (`DefaultDuration` owned strictly by this track)
  - `0x22B59C` (`Language`) / `0x22B59D` (`LanguageBCP47`), `0x536E` (`Name`)
  - `0xE0` (`Video` master): `0xB0` (`PixelWidth`), `0xBA` (`PixelHeight`), `0x54B0` (`DisplayWidth`), `0x54BA` (`DisplayHeight`). Encoded pixel dimensions (`PixelWidth`/`PixelHeight`) take precedence, falling back to display dimensions only when pixel dimensions are absent.
  - `0xE1` (`Audio` master): `0xB5` (`SamplingFrequency`, 4-byte `f32` or 8-byte `f64`), `0x78B5` (`OutputSamplingFrequency`), `0x9F` (`Channels`), `0x6264` (`BitDepth`).

### 2.3 Honest Codec Identification (Zero AAC / H.264 Spoofing)
- **Video (`TrackType == 1`)**:
  - `"V_MPEG4/ISO/AVC"` -> `CodecId::H264`
  - `"V_MPEGH/ISO/HEVC"` -> `CodecId::Hevc`
  - `"V_AV1"` -> `CodecId::Av1`
  - `"V_VP9"` -> `CodecId::Vp9`
  - `"V_PRORES"` -> `CodecId::Prores`
  - Any other `CodecID` (e.g., `"V_MPEG2"`, `"V_VP8"`, `"V_MS/VFW/FOURCC"`, unknown string) -> `CodecId::Other(raw_codec_id)`.
- **Audio (`TrackType == 2`)**:
  - `"A_AAC"` or `"A_AAC/*"` -> `CodecId::Aac`
  - `"A_AC3"` or `"A_AC3/*"` -> `CodecId::Ac3`
  - `"A_EAC3"` or `"A_EAC3/*"` -> `CodecId::Eac3`
  - `"A_FLAC"` -> `CodecId::Flac`
  - `"A_OPUS"` or `"A_OPUS/*"` -> `CodecId::Opus`
  - `"A_PCM/INT/LIT"` | `"A_PCM/INT/BIG"` | `"A_PCM/FLOAT/IEEE"` -> `CodecId::Pcm`
  - Any other `CodecID` (e.g., `"A_DTS"`, `"A_TRUEHD"`, `"A_VORBIS"`, `"A_MPEG/L3"`, unknown string) -> `CodecId::Other(raw_codec_id)`. Unknown or unsupported audio codecs are **never** disguised as `CodecId::Aac`.

---

## 3. Deterministic Multi-Audio Track Selection Rules

All parsed video and audio tracks are preserved in container order in `FormatDemuxer::mkv_tracks` and `FormatDemuxer::tracks_catalog`, with each track retaining its own `CodecPrivate`, `sample_rate`, `channels`, and `codec_id`.

To populate the primary playback pair in `FormatDemuxer::metadata.tracks` (`video_track_num` and `audio_track_num`):

1. **Primary Video Track Selection**:
   - Filter `TrackType::Video` tracks preferring `flag_enabled == true` (falling back to all video tracks if none are enabled).
   - Prefer known supported video codecs (`H264`, `Hevc`, `Av1`, `Vp9`, `Prores`) over `CodecId::Other(_)`.
   - Within the same support tier, prefer `flag_default == true`, then container declaration order.
2. **Primary Audio Track Selection**:
   - Filter `TrackType::Audio` tracks preferring `flag_enabled == true` (falling back to all audio tracks if none are enabled).
   - Partition candidates into **Supported Playable Audio Codecs** (`Ac3`, `Eac3`, `Aac`, `Flac`, `Opus`, `Pcm`) vs **Unsupported/Unknown Audio Codecs** (`CodecId::Other(_)`).
   - **Rule A (Playable Track Available)**: When one or more Supported Playable audio tracks exist, select exclusively from the playable set ranked by:
     1. `flag_default == true` over `flag_default == false`
     2. `flag_forced == false` over `flag_forced == true`
     3. Container declaration order (first declared playable track wins).
     - *Verification on real corpora*:
       - `REF-V4`: Track #2 is `A_DTS` (`CodecId::Other("A_DTS")`, 8ch), Track #3 is `A_AC3` (`CodecId::Ac3`, 6ch, 48 kHz). Rule A skips unsupported Track #2 and selects playable Track #3 (`CodecId::Ac3`, 6ch, 48 kHz).
       - `REF-V5`: Track #2 is `A_EAC3` (`CodecId::Eac3`, 6ch, 48 kHz). Rule A selects Track #2 (`CodecId::Eac3`).
       - `REF-V6`: Tracks #2 and #3 are `A_DTS` (`CodecId::Other("A_DTS")`, 6ch), Tracks #4–#7 are `A_AC3` (`CodecId::Ac3`). Even though `0x86` (`CodecID`) precedes `0x83` (`TrackType`) in every `TrackEntry`, structured parsing identifies Tracks #2–#3 as `Other("A_DTS")` and Tracks #4–#7 as `Ac3`, selecting Track #4 (`CodecId::Ac3`, 5ch, 48 kHz).
   - **Rule B (Only Unknown/Unsupported Audio Tracks Exist)**:
     - When all audio tracks in the container have `CodecId::Other(_)`, the first candidate (preferring `flag_default == true`, then declaration order) is exposed in `metadata.tracks` with its true `CodecId::Other(raw_codec_id)` (never `CodecId::Aac`), while `audio_track_num` is set to `None` so undecodable packets are not falsely delivered as AAC frames. Callers can also explicitly switch active audio tracks via `FormatDemuxer::select_audio_track(track_number)`.

---

## 4. `Cues` / `SeekHead` Keyframe Seeking & Unindexed Fallback (`mkv_index.rs`)

### 4.1 Indexed Seeking via `SeekHead` + `Cues` (Zero Linear Packet Scan)
1. **Locating `Cues` (`0x1C53BB6B`)**:
   - During `open()`, `FormatDemuxer` records the `Cues` byte range if encountered in the top-level `Segment` pre-cluster header or resolved via `SeekHead` (`segment_data_start + seek_position`).
2. **Single-Read In-Memory `Cues` Parsing**:
   - On the first non-zero `seek()` (or `ensure_cues_loaded()`), the `Cues` payload is read in a single `read_exact_at` call into a bounded buffer and parsed in memory into sorted `MkvCuePoint` entries.
   - `CueTrackPositions` (`0xB7`) are filtered for `cue_track == video_track_num` (falling back to `audio_track_num` only in audio-only containers) so subtitle cue points (such as Track #4 in `REF-V4` or PGS Tracks #10–#15 in `REF-V6`) never pollute video keyframe lookup.
3. **$O(\log N)$ Lookup & Intra-Cluster Keyframe Alignment**:
   - Given `target_pts_nanos`, binary search (`partition_point`) locates the preceding video cue point with `pts_nanos <= target_pts_nanos` (clamping to the first cue point if `target_pts_nanos` precedes the first cue).
   - The demuxer jumps directly to `cluster_header_offset = segment_data_start + cue_cluster_position`, validates the `Cluster` (`0x1F43B675`) header, reads the cluster `Timecode` (`0xE7`), and inspects only block headers (`0xA3` / `0xA0`->`0xA1`) within that target `Cluster` (using `cue_relative_position` as a fast hint when valid) to position `current_pos` at the exact video keyframe (`is_sync_frame == true`) with `pts_nanos <= target_pts_nanos`.
   - As a result, `seek()` returns the exact video keyframe `sync_pts_nanos`, and the immediately following `read_next_packet()` yields that exact keyframe without reading any prior clusters or pre-keyframe non-sync video blocks.

### 4.2 Unindexed Fallback Strategy & Cooperative Cancellation
When `Cues` is absent, empty, or points to a corrupted offset:
1. **Cluster-Header Hopping (No Packet Payload Reads)**:
   - Starting from `first_cluster_offset` (or the highest previously cached cluster `<= target_pts_nanos` in `discovered_clusters`), the fallback scanner reads only each known-size `Cluster` header and its leading `Timecode` (`0xE7`), jumping directly to `cluster.end` (`O(clusters)` header hops rather than `O(packets)` block scans).
   - Once the target `Cluster` bracket is found, only block headers within the candidate `Cluster` are inspected (skipping block payloads) to locate the last video keyframe `<= target_pts_nanos`.
2. **Cooperative Cancellation & Bounded Fallback Budget (`SeekOptions` / `SeekCancelToken`)**:
   - `SeekOptions` enforces configurable bounds (`max_fallback_clusters`, `max_fallback_bytes_read`, `max_fallback_duration`, and `cancel_token: Option<SeekCancelToken>`).
   - Calling `cancel_token.cancel()` or `demuxer.cancel_pending_seek()` aborts an in-flight fallback scan at the next cluster/block check, restores the demuxer to a clean state, records `SeekMetrics { cancelled: true, .. }`, and returns `Err(DemuxError::Cancelled)`.
   - Exceeding `max_fallback_bytes_read`, `max_fallback_clusters`, or `max_fallback_duration` stops the scan and returns `Err(DemuxError::SeekBudgetExceeded)` (or snaps to the best keyframe when `snap_on_budget_exceeded` is enabled).
3. **Quantitative Telemetry (`SeekMetrics` & `OpenMetrics`)**:
   - Every `open()` and `seek()` records exact physical `bytes_read`, `syscalls_read`, `cues_entries_searched`, `clusters_visited`, `blocks_inspected`, `elapsed`, and `SeekMode`, queryable via `demuxer.open_metrics()` and `demuxer.last_seek_metrics()`.

---

## 5. Acceptance Criteria & Verification Matrix

| ID | Category | Test Case / Corpus | Expected Guarantee |
| :--- | :--- | :--- | :--- |
| **V1-T1** | Field Order Permutation | Synthetic MKV with `TrackEntry`, `Video` (`0xE0`), and `Audio` (`0xE1`) child elements in reversed/shuffled orders (e.g. `CodecID` before `TrackType`, `CodecPrivate` first, `TrackNumber` last) | Identical `StreamMetadata` and `MkvTrackEntry` output across all permutations |
| **V1-T2** | Multi-Audio Selection | Multi-audio MKV (`A_DTS` default + `A_AC3` non-default + `A_AAC` default) and real corpora `REF-V4`, `REF-V5`, `REF-V6` | Playable track selected deterministically (`Ac3`/`Eac3`/`Aac` over `Other("A_DTS")`); `REF-V4` selects Track #3 `Ac3` (6ch), `REF-V6` selects Track #4 `Ac3` (5ch) |
| **V1-T3** | Per-Track `CodecPrivate` | Multi-track MKV where Video, Audio #1, Audio #2, and Subtitle each have distinct `CodecPrivate` (or Video has `None` while Audio/Subtitle have `CodecPrivate`) | Each track's `extradata` matches its own `0x63A2` payload with zero cross-track leakage |
| **V1-T4** | Unknown Codecs | MKV with unknown video (`V_MPEG2`) and unknown audio (`A_DTS`, `A_TRUEHD`, `A_CUSTOM_XYZ`) | Preserved as `CodecId::Other(raw_id)`, never disguised as `CodecId::Aac` or `CodecId::H264` |
| **V1-T5** | Corrupted Input | Empty/truncated files, invalid VINTs, element size overflowing parent/file, oversized `CodecPrivate`/`Cues`/block payloads, corrupt `SeekHead`/`Cues` offsets, malformed lacing | Returns clean `DemuxError` (or safe fallback on bad `Cues`) with zero panic, OOM, or infinite loop |
| **V1-T6** | Unindexed Fallback & Cancel | Unindexed multi-cluster MKV seek, budget exhaustion, and `SeekCancelToken` mid-scan cancellation | Cluster-header hopping resolves sync point; cancellation aborts promptly with `DemuxError::Cancelled` and accurate `SeekMetrics` |
| **V1-T7** | Real Corpora Non-Zero Seek | `REF-V4` (19.92 GB), `REF-V5` (20.58 GB), `REF-V6` (29.45 GB) non-zero seeks (`600s`, `1800s`, `3600s`) | Uses `SeekHead` + `Cues` index, first packet after seek is a valid video keyframe (`is_sync_frame == true`), records actual `bytes_read` and `elapsed` |

---

## 6. Measured Real-Corpus Telemetry & Verification Report

### 6.1 Real-Corpus Track Discovery & Audio Selection Results

| Corpus | File Size | Total Tracks | Video Track | Audio Tracks in Catalog | Selected Primary Audio Track |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **REF-V4** (`Focus.2015.2160p...mkv`) | 19.92 GB | 36 | Track #1 `CodecId::Hevc` (`3840x2160`, `131 B` `CodecPrivate`) | Track #2 `CodecId::Other("A_DTS")` (8ch, 48kHz), Track #3 `CodecId::Ac3` (6ch, 48kHz) | **Track #3 `CodecId::Ac3`** (6ch, 48kHz) |
| **REF-V5** (`The.Invite.2026.2160p...mkv`) | 20.58 GB | 5 | Track #1 `CodecId::Hevc` (`3836x2072`, `132 B` `CodecPrivate`) | Track #2 `CodecId::Eac3` (6ch, 48kHz) | **Track #2 `CodecId::Eac3`** (6ch, 48kHz) |
| **REF-V6** (`Dead Poets Society.1989.1080p...mkv`) | 29.45 GB | 18 | Track #1 `CodecId::H264` (`1920x1080`, `72 B` `CodecPrivate`) | Tracks #2–#3 `CodecId::Other("A_DTS")` (6ch), Track #4 `CodecId::Ac3` (5ch, 48kHz, `FlagDefault=true`), Track #5 `CodecId::Ac3` (6ch, 48kHz, `FlagDefault=true`), Tracks #6–#7 `CodecId::Ac3` (2ch) | **Track #4 `CodecId::Ac3`** (5ch, 48kHz; first audio packet verified with `0x0B77` AC-3 syncword) |

### 6.2 Measured Non-Zero Seek Telemetry (`SeekMode::CuesIndex`)

The first seek on each file includes the one-time positioned read of the tail `Cues` element (`1.36 MB` on `REF-V4`, `241 KB` on `REF-V5`, `1.32 MB` on `REF-V6`) plus one `64 KiB` window read at the target `Cluster`. Subsequent seeks reuse the parsed `Cues` cache and perform a single `64 KiB` positioned `pread` at the target `Cluster`:

| Corpus | Target Timestamp | Resolved Keyframe PTS | `SeekMode` | `clusters_visited` | `blocks_inspected` | Physical `bytes_read` | `syscalls_read` | Elapsed Time |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **REF-V4** | `600s` (`10m`) | `598,056 ms` | `CuesIndex` | `1` | `1` | `1,429,251 B` | `2` | `60.10 ms` |
| **REF-V4** | `1800s` (`30m`) | `1,798,841 ms` | `CuesIndex` | `1` | `1` | `65,536 B` | `1` | `10.38 µs` |
| **REF-V4** | `3600s` (`60m`) | `3,599,224 ms` | `CuesIndex` | `1` | `1` | `65,536 B` | `1` | `17.92 µs` |
| **REF-V5** | `600s` (`10m`) | `597,667 ms` | `CuesIndex` | `1` | `1` | `307,029 B` | `2` | `7.96 ms` |
| **REF-V5** | `1800s` (`30m`) | `1,797,917 ms` | `CuesIndex` | `1` | `1` | `65,536 B` | `1` | `10.46 µs` |
| **REF-V5** | `3600s` (`60m`) | `3,596,917 ms` | `CuesIndex` | `1` | `1` | `65,536 B` | `1` | `12.21 µs` |
| **REF-V6** | `600s` (`10m`) | `599,974 ms` | `CuesIndex` | `1` | `1` | `1,388,030 B` | `2` | `53.38 ms` |
| **REF-V6** | `1800s` (`30m`) | `1,799,631 ms` | `CuesIndex` | `1` | `1` | `65,536 B` | `1` | `11.63 µs` |
| **REF-V6** | `3600s` (`60m`) | `3,599,721 ms` | `CuesIndex` | `1` | `1` | `65,536 B` | `1` | `10.63 µs` |

### 6.3 Verification Commands Executed & Cross-Reference to V1 → V2 Contract

1. `cargo clippy --manifest-path infra/ttmedia/Cargo.toml --lib --test test_track_metadata_contract -- -D warnings` — **PASS** (0 warnings).
2. `cargo test --manifest-path infra/ttmedia/Cargo.toml --test test_track_metadata_contract -- --nocapture` — **PASS** (8/8 tests passed, including all 7 deterministic contract scenarios and `REF-V4` / `REF-V5` / `REF-V6` real-corpus inspection with zero `exists()` skip branches).
3. `cargo test --manifest-path infra/ttmedia/Cargo.toml --lib -- demux:: --nocapture` — **PASS** (8/8 unit tests passed, zero `exists()` skip branches in `mkv_seek_tests.rs`).
4. `bazel build //infra/ttmedia:ttmedia //infra/ttmedia:TTMediaCABI && bazel test //infra/ttmedia:ttmedia_test` — **PASS**.
5. `python3 tools/ci/lint_loc_gate.py --dirs infra/ttmedia/src/demux infra/ttmedia/src/c_abi.rs infra/ttmedia/include/ttmedia_demux.h infra/ttmedia/Tests/test_track_metadata_contract.rs --max-loc 800 --min-files 1 --no-baseline` — **PASS** (10/10 files $\le 800$ LOC).
6. `python3 tools/ci/lint_code_quality.py --strict --no-baseline --dirs infra/ttmedia/src/demux infra/ttmedia/src/c_abi.rs infra/ttmedia/Tests/test_track_metadata_contract.rs` — **PASS** (133/133 functions $\le 80$ LOC and $\le 15$ McCabe CC).
7. Full V1 → V2 track metadata contract, C-ABI `ttmedia_demux_query_tracks_json` specification, real-corpus `CodecPrivate` / first-packet SHA-256 telemetry, and final file SHA-256 inventory are recorded in [`v1-v2-track-metadata-contract.md`](./v1-v2-track-metadata-contract.md).
