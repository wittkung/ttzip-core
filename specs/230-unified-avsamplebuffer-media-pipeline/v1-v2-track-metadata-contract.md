# V1 → V2 Track Metadata & Native C-ABI Bridge Contract (2026-10-17)

**Feature Directory**: `products/ttzip/core/specs/230-unified-avsamplebuffer-media-pipeline`

**Owner**: V1 (Native MKV Demux & C-ABI Producer)  
**Consumer**: V2 (Swift `MediaWorkerDirectTransport` / `AVSampleBufferAudioPipeline` / `AVSampleBufferWorkerPlaybackController`)  
**Build & Integration Handoff**: H (`BUILD.bazel` / `empty.c` symbol retention), P2 (Application Playback Acceptance)

**V1 Exclusive File Scope**:
- `products/ttzip/core/specs/230-unified-avsamplebuffer-media-pipeline/v1-v2-track-metadata-contract.md`
- `products/ttzip/core/specs/230-unified-avsamplebuffer-media-pipeline/v1-mkv-structured-demux-and-cues-seek.md`
- `infra/ttmedia/src/demux/*`
- `infra/ttmedia/src/c_abi.rs`
- `infra/ttmedia/include/ttmedia_demux.h`
- `infra/ttmedia/Tests/test_track_metadata_contract.rs`

---

## 1. Contract Invariants & Core Semantics

| Dimension | Contract Rule |
| :--- | :--- |
| **1. Track Identity (`轨道身份`)** | Every parsed track exposes four explicit identifiers:<br>• `catalog_index` (`u32`): 0-based ordinal position inside Matroska `Tracks` (`0x1654AE6B`).<br>• `container_track_number` (`u64`): raw Matroska `TrackNumber` (`0xD7`, must satisfy `1 <= n <= u32::MAX`; `0` or `> u32::MAX` is rejected as `TT_MEDIA_DEMUX_ERR_CORRUPT_DATA = -3`).<br>• `playback_track_id` (`u32`): canonical track ID used across `StreamTrackInfo.track_index`, `TTMediaVideoTrackInfo.trackIndex`, and `TTMediaAudioTrackInfo.trackIndex`. Always equals `container_track_number as u32`.<br>• `packet_stream_index` (`u32`): value written to `TTMediaPacket.stream_index` (and `DemuxedPacket.stream_id as u32`). Always equals `playback_track_id == container_track_number as u32`.<br>• `track_uid` (`u64`): Matroska `TrackUID` (`0x73C5`, `0` if absent in container). |
| **2. Codec Identification (`编码`)** | • Known codecs map to unified canonical enum values (`TTVideoCodec` / `TTAudioCodec`) and lowercase tokens (`canonical_codec`).<br>• Unknown or passthrough-only codecs (`"A_DTS"`, `"A_TRUEHD"`, `"A_MLP"`, `"V_MPEG2"`, etc.) map to `TT_VIDEO_CODEC_UNKNOWN (0)` / `TT_AUDIO_CODEC_UNKNOWN (0)` in C-ABI structs and `canonical_codec: null` in JSON, while preserving the exact container string in `raw_codec_id`.<br>• **Zero AAC / H.264 Spoofing**: Unknown audio is never reported as `TT_AUDIO_CODEC_AAC (1)` or `"aac"`; unknown video is never reported as `TT_VIDEO_CODEC_H264 (1)` or `"h264"`.<br>• **Demux Recognition vs. CoreAudio Playability**: `demux_selectable: true` indicates only that the Rust container demuxer recognizes the elementary framing and can emit packets for that codec ID. `coreaudio_verified_playable` is always `false` at the V1 demux layer; V1 never claims CoreAudio decoder/renderer playability, which must be verified at runtime by V2 (`CMAudioFormatDescriptionCreate` / `AVSampleBufferAudioRenderer`). |
| **3. Selected Tracks vs. Full Catalog (`选中轨道`)** | • `tracks_catalog`: all tracks present in `Tracks` (`0x1654AE6B`) in container order (video, audio, subtitle), each annotated with `is_primary_track` and `is_selected_for_packets`.<br>• `selected_tracks`: the active primary video and/or primary audio tracks reported in `FormatDemuxer.metadata.tracks` and `TTMediaStreamInfo`.<br>• `selected_video_track_number`: `u32` `TrackNumber` of the video track actively emitting packets (`null` if no playable video track exists).<br>• `selected_audio_track_number`: `u32` `TrackNumber` of the audio track actively emitting packets (`null` if no audio track exists OR if all audio tracks are non-selectable unknown codecs such as `A_DTS`-only / `A_TRUEHD`-only).<br>• `primary_audio_track_number`: `u32` `TrackNumber` of the primary audio track surfaced in `TTMediaStreamInfo.audio_desc` and `selected_tracks` (`null` only if the container has zero audio tracks). When a file has only `A_DTS` (Track #2), `primary_audio_track_number = 2` (with `c_audio_codec = 0`, `raw_codec_id = "A_DTS"`) while `selected_audio_track_number = null` (so V2 can report `unsupportedAudioTrack("A_DTS")` while receiving zero unplayable audio packets). |
| **4. Audio Parameters & Provenance (`音频参数`)** | • `sample_rate_hz` (`f64`), `sample_rate` (`u32`, rounded Hz), and `sample_rate_source`:<br>&nbsp;&nbsp;- `"explicit_sampling_frequency"`: explicitly parsed from `0xB5 SamplingFrequency` inside `0xE1 Audio`.<br>&nbsp;&nbsp;- `"explicit_output_sampling_frequency"`: `0xB5` absent, parsed from `0x78B5 OutputSamplingFrequency`.<br>&nbsp;&nbsp;- `"container_spec_default"`: `0xB5` and `0x78B5` absent; Matroska RFC 9559 default `8000.0` Hz applied.<br>• `channels` (`u32`) and `channels_source`:<br>&nbsp;&nbsp;- `"explicit_channels"`: explicitly parsed from `0x9F Channels` inside `0xE1 Audio`.<br>&nbsp;&nbsp;- `"container_spec_default"`: `0x9F` absent; Matroska RFC 9559 default `1` channel applied.<br>• `bit_depth` (`u32 | null`) and `bit_depth_source`: `"explicit_bit_depth"` when `0x6264 BitDepth` is present, else `null` / `"absent"` (`TTAudioFormatDesc.bits_per_channel = 0` when absent).<br>• **Channel Layout Prohibition**: Matroska `TrackEntry` (`0xE1`) carries only channel count (`0x9F`), not speaker layout masks. `channel_layout` is always `null` (`channel_layout_source: "absent_in_mkv_track_entry"`). Consumers must not infer speaker layout solely from `channels`.<br>• **Invalid Values Rejected**: `SamplingFrequency <= 0.0` / `NaN` / `Inf`, `Channels == 0` or `> 256`, or explicit `BitDepth == 0` or `> 64` fail `open` with `TT_MEDIA_DEMUX_ERR_CORRUPT_DATA (-3)`. |
| **5. Codec Configuration (`编码配置`)** | • `codec_private_present` (`bool`), `codec_private_len` (`usize`), `codec_private_hex` (`string | null`), `codec_private_sha256` (`string | null`), `codec_private_source` (`"track_entry_codec_private_0x63a2"` or `"absent"`), and `codec_private_format` (`"avcc"`, `"hvcc"`, `"av1c"`, `"vp9_codec_private"`, `"aac_audio_specific_config"`, `"opus_head"`, `"flac_streaminfo"`, `"raw_codec_private"`, or `"none"`).<br>• Raw `0x63A2 CodecPrivate` bytes belong strictly to their enclosing `TrackEntry`.<br>• When a track has no `0x63A2`, `TTVideoFormatDesc`/`TTAudioFormatDesc` set `extradata = NULL, extradata_len = 0`, and JSON sets `codec_private_present = false, codec_private_len = 0, codec_private_hex = null, codec_private_sha256 = null`. |
| **6. Video Parameters (`视频参数`)** | • `c_video_codec` (`TTVideoCodec`), `raw_codec_id`, `canonical_codec`.<br>• `pixel_width`, `pixel_height`, `display_width`, `display_height`, `effective_width`, `effective_height`, and `dimensions_source` (`"explicit_pixel_dimensions"`, `"fallback_display_dimensions"`, or `"legacy_raw_cluster_default"`).<br>• In standard `Segment`-based MKV/WebM files, a video `TrackEntry` with missing or zero effective width/height is rejected with `TT_MEDIA_DEMUX_ERR_CORRUPT_DATA (-3)` rather than fabricating `1920x1080`. |
| **7. Packet & Seek Consistency (`包的一致性`)** | • `ttmedia_demux_read_packet` emits packets **only** for `selected_video_track_number` (`media_type = TT_MEDIA_TYPE_VIDEO = 1`) and `selected_audio_track_number` (`media_type = TT_MEDIA_TYPE_AUDIO = 2`).<br>• Every emitted video packet satisfies `packet.stream_index == selected_video_track_number`.<br>• Every emitted audio packet satisfies `packet.stream_index == selected_audio_track_number`. When `selected_audio_track_number == null`, zero audio packets are emitted.<br>• Calling `ttmedia_demux_seek` preserves `selected_video_track_number`, `selected_audio_track_number`, codec descriptors, and `CodecPrivate` unchanged, and post-seek packets preserve the exact same `stream_index` mapping. |

---

## 2. Native C-ABI Signatures & Buffer Ownership Contract

### 2.1 Binary Compatibility of Existing Types & Functions

All existing enums, structs, and function signatures in [`ttmedia_demux.h`](file://../../infra/ttmedia/include/ttmedia_demux.h) and [`c_abi.rs`](file://../../infra/ttmedia/src/c_abi.rs) remain layout- and ABI-identical:

- `TTMediaDemuxStatus`:
  - `TT_MEDIA_DEMUX_OK = 0`
  - `TT_MEDIA_DEMUX_EOF = 1`
  - `TT_MEDIA_DEMUX_ERR_INVALID_ARGUMENT = -1`
  - `TT_MEDIA_DEMUX_ERR_IO = -2`
  - `TT_MEDIA_DEMUX_ERR_CORRUPT_DATA = -3`
  - `TT_MEDIA_DEMUX_ERR_UNSUPPORTED_FORMAT = -4`
  - `TT_MEDIA_DEMUX_ERR_OUT_OF_MEMORY = -5`
  - `TT_MEDIA_DEMUX_ERR_SEEK_FAILED = -6`
- `TTVideoCodec`:
  - `TT_VIDEO_CODEC_UNKNOWN = 0`
  - `TT_VIDEO_CODEC_H264 = 1`
  - `TT_VIDEO_CODEC_HEVC = 2`
  - `TT_VIDEO_CODEC_AV1 = 3`
  - `TT_VIDEO_CODEC_VP9 = 4`
  - `TT_VIDEO_CODEC_VP8 = 5`
  - `TT_VIDEO_CODEC_PRORES = 6`
- `TTAudioCodec`:
  - `TT_AUDIO_CODEC_UNKNOWN = 0`
  - `TT_AUDIO_CODEC_AAC = 1`
  - `TT_AUDIO_CODEC_ALAC = 2`
  - `TT_AUDIO_CODEC_OPUS = 3`
  - `TT_AUDIO_CODEC_FLAC = 4`
  - `TT_AUDIO_CODEC_VORBIS = 5`
  - `TT_AUDIO_CODEC_MP3 = 6`
  - `TT_AUDIO_CODEC_PCM_S16LE = 7`
  - `TT_AUDIO_CODEC_PCM_S24LE = 8`
  - `TT_AUDIO_CODEC_PCM_F32LE = 9`
  - `TT_AUDIO_CODEC_AC3 = 10`
  - `TT_AUDIO_CODEC_EAC3 = 11`

### 2.2 Corrected `ttmedia_demux_open` Behavior on `TTMediaStreamInfo`

1. **Container Format**:
   - Sets `TT_MEDIA_CONTAINER_FORMAT_MATROSKA (2)` for Matroska and `TT_MEDIA_CONTAINER_FORMAT_WEBM (3)` for WebM.
2. **Video Descriptor (`has_video`, `video_desc`)**:
   - If the container has a primary video track: `has_video = true`, `video_desc.codec` is mapped honestly (`H264 -> 1`, `Hevc -> 2`, `Av1 -> 3`, `Vp9 -> 4`, `Prores -> 6`, `Other(_) -> TT_VIDEO_CODEC_UNKNOWN (0)`), `width` and `height` reflect the track's effective dimensions, and `extradata` / `extradata_len` point to the video track's immutable `CodecPrivate` buffer owned by the handle (`NULL, 0` if absent).
   - If the container has no video track: `has_video = false`, and `video_desc` is zeroed (`codec = TT_VIDEO_CODEC_UNKNOWN (0)`, `width = 0`, `height = 0`, `extradata = NULL`, `extradata_len = 0`).
3. **Audio Descriptor (`has_audio`, `audio_desc`)**:
   - If the container has a primary audio track (even if its codec is unsupported, e.g. `A_DTS`): `has_audio = true`, `audio_desc.codec` is mapped honestly (`Aac -> 1`, `Opus -> 3`, `Flac -> 4`, `PcmS16le -> 7`, `PcmS24le -> 8`, `PcmF32le -> 9`, `Ac3 -> 10`, `Eac3 -> 11`, `Other(_) -> TT_AUDIO_CODEC_UNKNOWN (0)`), `sample_rate` is the track's effective sample rate (`f64`), `channel_count` is the track's effective channel count (`u32`), `bits_per_channel` is the track's explicit `BitDepth` (`0` when absent in container), and `extradata` / `extradata_len` point to that audio track's immutable `CodecPrivate` buffer (`NULL, 0` if absent).
   - If the container has zero audio tracks: `has_audio = false`, and `audio_desc` is zeroed (`codec = TT_AUDIO_CODEC_UNKNOWN (0)`, `sample_rate = 0.0`, `channel_count = 0`, `bits_per_channel = 0`, `extradata = NULL`, `extradata_len = 0`).
4. **Error Status Mapping**:
   - Null pointer or invalid UTF-8 `path` -> `TT_MEDIA_DEMUX_ERR_INVALID_ARGUMENT (-1)`
   - `DemuxError::Io` -> `TT_MEDIA_DEMUX_ERR_IO (-2)`
   - `DemuxError::CorruptData` / `DemuxError::AnnexB` -> `TT_MEDIA_DEMUX_ERR_CORRUPT_DATA (-3)`
   - `DemuxError::UnsupportedFormat` -> `TT_MEDIA_DEMUX_ERR_UNSUPPORTED_FORMAT (-4)`

### 2.3 New Read-Only Bounded JSON Metadata Query Function

Because `TTMediaStreamInfo` has a fixed C layout that cannot represent container `TrackNumber`, raw `CodecID` strings (`"A_DTS"`, `"A_TRUEHD"`), parameter provenance, or the full multi-track catalog, V1 adds a read-only, caller-buffered query function in `include/ttmedia_demux.h` and `src/c_abi.rs`:

```c
/**
 * Queries the structured stream and track metadata for an open demuxer handle
 * as a NUL-terminated UTF-8 JSON string written into a caller-provided buffer.
 *
 * Buffer & Ownership Contract:
 * 1. `handle` must be a non-null handle returned by `ttmedia_demux_open()`.
 * 2. `out_required_len` must be a non-null pointer (`size_t *`). On every call
 *    with a valid `handle` and `out_required_len`, `*out_required_len` is set to
 *    the exact UTF-8 byte length of the JSON payload EXCLUDING the trailing NUL
 *    terminator (`'\0'`). To receive the full NUL-terminated string, the caller
 *    must supply a buffer with `buf_capacity >= *out_required_len + 1`.
 * 3. Length-Query Mode:
 *    Pass `out_json_buf == NULL` and `buf_capacity == 0`.
 *    Writes `*out_required_len = payload_len` and returns `TT_MEDIA_DEMUX_OK` (0).
 * 4. Copy Mode:
 *    Pass `out_json_buf != NULL` and `buf_capacity >= payload_len + 1`.
 *    Writes `payload_len` bytes of valid UTF-8 JSON into `out_json_buf`, writes
 *    `out_json_buf[payload_len] = '\0'`, sets `*out_required_len = payload_len`,
 *    and returns `TT_MEDIA_DEMUX_OK` (0).
 *    The caller owns `out_json_buf`; no native heap allocation is retained or leaked.
 * 5. Insufficient Capacity (Never Silently Truncates):
 *    If `out_json_buf != NULL` and `buf_capacity < payload_len + 1`, sets
 *    `*out_required_len = payload_len`, writes `out_json_buf[0] = '\0'` when
 *    `buf_capacity > 0`, and returns `TT_MEDIA_DEMUX_ERR_OUT_OF_MEMORY` (-5).
 * 6. Invalid Arguments:
 *    Returns `TT_MEDIA_DEMUX_ERR_INVALID_ARGUMENT` (-1) if `handle == NULL`,
 *    `out_required_len == NULL`, or (`out_json_buf == NULL && buf_capacity > 0`).
 * 7. Maximum Payload Bound:
 *    Bounded to `4 MiB` (`4 * 1024 * 1024` bytes); returns
 *    `TT_MEDIA_DEMUX_ERR_CORRUPT_DATA` (-3) if exceeded.
 */
int32_t ttmedia_demux_query_tracks_json(
    const void * _Nonnull handle,
    char * _Nullable out_json_buf,
    size_t buf_capacity,
    size_t * _Nonnull out_required_len
);
```

#### Return Code Summary Table for `ttmedia_demux_query_tracks_json`

| Call Pattern | `*out_required_len` | `out_json_buf` State | Return Code |
| :--- | :--- | :--- | :--- |
| `handle == NULL` or `out_required_len == NULL` | Unchanged (if NULL) or `0` | Unchanged | `TT_MEDIA_DEMUX_ERR_INVALID_ARGUMENT (-1)` |
| `out_json_buf == NULL && buf_capacity > 0` | `0` | N/A | `TT_MEDIA_DEMUX_ERR_INVALID_ARGUMENT (-1)` |
| `out_json_buf == NULL && buf_capacity == 0` | `payload_len` (excludes `'\0'`) | N/A | `TT_MEDIA_DEMUX_OK (0)` |
| `out_json_buf != NULL && buf_capacity == 0` | `payload_len` | Unchanged | `TT_MEDIA_DEMUX_ERR_OUT_OF_MEMORY (-5)` |
| `out_json_buf != NULL && 0 < buf_capacity <= payload_len` | `payload_len` | `out_json_buf[0] = '\0'` (no partial JSON written) | `TT_MEDIA_DEMUX_ERR_OUT_OF_MEMORY (-5)` |
| `out_json_buf != NULL && buf_capacity >= payload_len + 1` | `payload_len` | Valid UTF-8 JSON (`payload_len` bytes) + `'\0'` | `TT_MEDIA_DEMUX_OK (0)` |

### 2.4 Scope Boundary: Track Selection vs. Cross-Layer UI Track Switching

- **Rust Native Layer (`FormatDemuxer`)**:
  - `FormatDemuxer::open` automatically selects the primary playable video track and primary playable audio track according to the deterministic priority rule (`flag_enabled` -> `demux_selectable` -> `flag_default` -> `!flag_forced` -> container order).
  - `FormatDemuxer::select_audio_track(track_index: u32)` is available in Rust for programmatic audio track switching; when invoked in Rust, `demuxer.audio_track_num`, `demuxer.metadata.tracks`, `demuxer.query_tracks_json()`, and subsequent `read_next_packet()` calls immediately reflect the newly selected track.
- **Application / XPC Layer**:
  - This round does **not** add a new user-facing audio-track switcher UI or modify `TTMediaWorkerProtocol.swift`. V2 consumes `ttmedia_demux_open` and `ttmedia_demux_query_tracks_json` (via `dlsym` in `MediaWorkerDirectTransport`) to obtain accurate `trackIndex`, `codec`, `codecName` (`raw_codec_id`), `sampleRate`, `channels`, and `extradata` for the selected tracks and full catalog.

---

## 3. Canonical JSON Schema (`schema_version: 1`)

### 3.1 Top-Level Object (`DemuxTracksMetadataContract`)

| Field | JSON Type | Description |
| :--- | :--- | :--- |
| `schema_version` | `number` (`1`) | Fixed contract schema version (`1`). |
| `container_format` | `string` | `"matroska"` or `"webm"`. |
| `c_container_format` | `number` | `TTMediaContainerFormat` integer (`2` = Matroska, `3` = WebM). |
| `duration_nanos` | `number` (`u64`) | Container duration in nanoseconds (`0` if unspecified). |
| `duration_seconds` | `number` (`f64`) | Container duration in seconds (`duration_nanos / 1e9`). |
| `timecode_scale_nanos` | `number` (`u64`) | Matroska `TimecodeScale` (`0x2AD7B1`) in nanoseconds (default `1_000_000`). |
| `has_cues_index` | `boolean` | `true` if a `Cues` (`0x1C53BB6B`) index was discovered in `Segment` or via `SeekHead`. |
| `selected_video_track_number` | `number \| null` | `TrackNumber` (`u32`) of the video track emitting packets, or `null` if none. |
| `selected_audio_track_number` | `number \| null` | `TrackNumber` (`u32`) of the audio track emitting packets, or `null` if no playable audio track is selected. |
| `primary_audio_track_number` | `number \| null` | `TrackNumber` (`u32`) of the primary audio track surfaced in `selected_tracks` / `TTMediaStreamInfo.audio_desc` (even if its codec is unknown/non-selectable, e.g. `A_DTS`), or `null` if the container has zero audio tracks. |
| `selected_tracks` | `Array<TrackContractEntry>` | Ordered `[video?, audio?]` entries corresponding to `FormatDemuxer.metadata.tracks` and `TTMediaStreamInfo`. |
| `tracks_catalog` | `Array<TrackContractEntry>` | All tracks parsed from `Tracks` (`0x1654AE6B`) in container order. |

### 3.2 Per-Track Object (`TrackContractEntry`)

| Field | JSON Type | Description |
| :--- | :--- | :--- |
| `catalog_index` | `number` (`u32`) | 0-based index in `tracks_catalog`. |
| `container_track_number` | `number` (`u64`) | Matroska `0xD7 TrackNumber` (`1..=u32::MAX`). |
| `playback_track_id` | `number` (`u32`) | Equal to `container_track_number as u32`. |
| `packet_stream_index` | `number` (`u32`) | Equal to `container_track_number as u32`; matches `TTMediaPacket.stream_index`. |
| `track_uid` | `number` (`u64`) | Matroska `0x73C5 TrackUID` (`0` if omitted). |
| `media_type` | `string` | `"video"`, `"audio"`, `"subtitle"`, or `"unknown"`. |
| `c_media_type` | `number` (`u8`) | `TTMediaType` enum value (`1` Video, `2` Audio, `3` Subtitle, `0` Unknown). |
| `raw_track_type` | `number` (`u64`) | Raw Matroska `0x83 TrackType` (`1` Video, `2` Audio, `17` Subtitle). |
| `flag_enabled` | `boolean` | Matroska `0xB9 FlagEnabled` (default `true`). |
| `flag_default` | `boolean` | Matroska `0x88 FlagDefault` (default `true`). |
| `flag_forced` | `boolean` | Matroska `0x55AA FlagForced` (default `false`). |
| `raw_codec_id` | `string` | Exact Matroska `0x86 CodecID` string (e.g. `"V_MPEGH/ISO/HEVC"`, `"A_AC3"`, `"A_DTS"`). |
| `canonical_codec` | `string \| null` | Unified codec token (`"h264"`, `"hevc"`, `"av1"`, `"vp9"`, `"prores"`, `"aac"`, `"ac3"`, `"eac3"`, `"flac"`, `"opus"`, `"pcm"`) or `null` when unknown. |
| `c_video_codec` | `number` (`i32`) | `TTVideoCodec` enum value (`0` for non-video or unknown video codec). |
| `c_audio_codec` | `number` (`i32`) | `TTAudioCodec` enum value (`0` for non-audio or unknown audio codec). |
| `demux_selectable` | `boolean` | `true` if the Rust demuxer recognizes and can frame packets for this track; `false` for unknown/passthrough-only codecs (`A_DTS`, `A_TRUEHD`, etc.). |
| `coreaudio_verified_playable` | `boolean` | Always `false` from V1 demuxer (runtime CoreAudio verification is owned by V2). |
| `is_primary_track` | `boolean` | `true` if this track is in `selected_tracks` / `TTMediaStreamInfo`. |
| `is_selected_for_packets` | `boolean` | `true` if `ttmedia_demux_read_packet` actively emits packets for this track. |
| `language` | `string` | BCP-47 (`0x22B59D`) or ISO 639-2 (`0x22B59C`) language tag (default `"und"`). |
| `name` | `string` | Matroska `0x536E Name` (default `""`). |
| `default_duration_nanos` | `number \| null` | Matroska `0x23E383 DefaultDuration` in nanoseconds, or `null` if absent. |
| `codec_private_present` | `boolean` | `true` iff `0x63A2 CodecPrivate` is present and non-empty on this track. |
| `codec_private_len` | `number` (`usize`) | Byte length of `CodecPrivate` (`0` when absent). |
| `codec_private_format` | `string` | `"avcc"`, `"hvcc"`, `"av1c"`, `"vp9_codec_private"`, `"aac_audio_specific_config"`, `"opus_head"`, `"flac_streaminfo"`, `"raw_codec_private"`, or `"none"`. |
| `codec_private_source` | `string` | `"track_entry_codec_private_0x63a2"` when present, `"absent"` when omitted. |
| `codec_private_hex` | `string \| null` | Lowercase hex encoding of `CodecPrivate` bytes (for `codec_private_len <= 4096`), or `null` when absent. |
| `codec_private_sha256` | `string \| null` | Lowercase hex SHA-256 digest of `CodecPrivate` bytes, or `null` when absent. |
| `video` | `VideoTrackDetails \| null` | Present iff `media_type == "video"`. |
| `audio` | `AudioTrackDetails \| null` | Present iff `media_type == "audio"`. |

---

## 4. Deterministic Fixtures for Parallel V2 Development

V2 can immediately unit-test its metadata parser and audio/video pipeline initialization against the following deterministic fixtures (which are also asserted verbatim in `infra/ttmedia/Tests/test_track_metadata_contract.rs`).

### Fixture S1: DTS First (Track #2, Default) + AC-3 Second (Track #5, Non-Default)
- **Scenario**: Video Track #1 (`V_MPEGH/ISO/HEVC`, `3840x2160`, `CodecPrivate = [0x01, 0x02, 0x03, 0x04]`), Audio Track #2 (`A_DTS`, `FlagDefault=1`, 8ch, 48kHz), Audio Track #5 (`A_AC3`, `FlagDefault=0`, 6ch, 48kHz).
- **`TTMediaStreamInfo`**:
  - `container_format = TT_MEDIA_CONTAINER_FORMAT_MATROSKA (2)`
  - `has_video = true`, `video_desc.codec = TT_VIDEO_CODEC_HEVC (2)`, `width = 3840`, `height = 2160`, `extradata_len = 4`
  - `has_audio = true`, `audio_desc.codec = TT_AUDIO_CODEC_AC3 (10)`, `sample_rate = 48000.0`, `channel_count = 6`, `bits_per_channel = 0`, `extradata = NULL`, `extradata_len = 0`
- **First Audio Packet from `ttmedia_demux_read_packet`**:
  - `media_type = TT_MEDIA_TYPE_AUDIO (2)`, `stream_index = 5` (matches `selected_audio_track_number = 5`), payload starts with `[0x0B, 0x77]`. Zero packets are ever emitted with `stream_index = 2`.
- **JSON Payload (`ttmedia_demux_query_tracks_json`)**:
```json
{
  "schema_version": 1,
  "container_format": "matroska",
  "c_container_format": 2,
  "duration_nanos": 1000000000,
  "duration_seconds": 1.0,
  "timecode_scale_nanos": 1000000,
  "has_cues_index": false,
  "selected_video_track_number": 1,
  "selected_audio_track_number": 5,
  "primary_audio_track_number": 5,
  "selected_tracks": [
    {
      "catalog_index": 0,
      "container_track_number": 1,
      "playback_track_id": 1,
      "packet_stream_index": 1,
      "track_uid": 0,
      "media_type": "video",
      "c_media_type": 1,
      "raw_track_type": 1,
      "flag_enabled": true,
      "flag_default": true,
      "flag_forced": false,
      "raw_codec_id": "V_MPEGH/ISO/HEVC",
      "canonical_codec": "hevc",
      "c_video_codec": 2,
      "c_audio_codec": 0,
      "demux_selectable": true,
      "coreaudio_verified_playable": false,
      "is_primary_track": true,
      "is_selected_for_packets": true,
      "language": "und",
      "name": "",
      "default_duration_nanos": 40000000,
      "codec_private_present": true,
      "codec_private_len": 4,
      "codec_private_format": "hvcc",
      "codec_private_source": "track_entry_codec_private_0x63a2",
      "codec_private_hex": "01020304",
      "codec_private_sha256": "9f64a747e1b97f131fabb6b447296c9b6f0201e79fb3c5356e6c77e89b6a806a",
      "video": {
        "pixel_width": 3840,
        "pixel_height": 2160,
        "display_width": null,
        "display_height": null,
        "effective_width": 3840,
        "effective_height": 2160,
        "dimensions_source": "explicit_pixel_dimensions"
      },
      "audio": null
    },
    {
      "catalog_index": 2,
      "container_track_number": 5,
      "playback_track_id": 5,
      "packet_stream_index": 5,
      "track_uid": 0,
      "media_type": "audio",
      "c_media_type": 2,
      "raw_track_type": 2,
      "flag_enabled": true,
      "flag_default": false,
      "flag_forced": false,
      "raw_codec_id": "A_AC3",
      "canonical_codec": "ac3",
      "c_video_codec": 0,
      "c_audio_codec": 10,
      "demux_selectable": true,
      "coreaudio_verified_playable": false,
      "is_primary_track": true,
      "is_selected_for_packets": true,
      "language": "eng",
      "name": "",
      "default_duration_nanos": null,
      "codec_private_present": false,
      "codec_private_len": 0,
      "codec_private_format": "none",
      "codec_private_source": "absent",
      "codec_private_hex": null,
      "codec_private_sha256": null,
      "video": null,
      "audio": {
        "sample_rate_hz": 48000.0,
        "sample_rate": 48000,
        "sample_rate_source": "explicit_sampling_frequency",
        "output_sample_rate_hz": null,
        "channels": 6,
        "channels_source": "explicit_channels",
        "bit_depth": null,
        "bit_depth_source": "absent",
        "channel_layout": null,
        "channel_layout_source": "absent_in_mkv_track_entry"
      }
    }
  ],
  "tracks_catalog": [
    "/* catalog_index 0: Track #1 V_MPEGH/ISO/HEVC (same as selected_tracks[0]) */",
    {
      "catalog_index": 1,
      "container_track_number": 2,
      "playback_track_id": 2,
      "packet_stream_index": 2,
      "track_uid": 0,
      "media_type": "audio",
      "c_media_type": 2,
      "raw_track_type": 2,
      "flag_enabled": true,
      "flag_default": true,
      "flag_forced": false,
      "raw_codec_id": "A_DTS",
      "canonical_codec": null,
      "c_video_codec": 0,
      "c_audio_codec": 0,
      "demux_selectable": false,
      "coreaudio_verified_playable": false,
      "is_primary_track": false,
      "is_selected_for_packets": false,
      "language": "eng",
      "name": "",
      "default_duration_nanos": null,
      "codec_private_present": false,
      "codec_private_len": 0,
      "codec_private_format": "none",
      "codec_private_source": "absent",
      "codec_private_hex": null,
      "codec_private_sha256": null,
      "video": null,
      "audio": {
        "sample_rate_hz": 48000.0,
        "sample_rate": 48000,
        "sample_rate_source": "explicit_sampling_frequency",
        "output_sample_rate_hz": null,
        "channels": 8,
        "channels_source": "explicit_channels",
        "bit_depth": null,
        "bit_depth_source": "absent",
        "channel_layout": null,
        "channel_layout_source": "absent_in_mkv_track_entry"
      }
    },
    "/* catalog_index 2: Track #5 A_AC3 (same as selected_tracks[1]) */"
  ]
}
```

### Fixture S2: DTS / TrueHD Only (No Playable Audio Track)
- **Scenario**: Video Track #1 (`V_MPEG4/ISO/AVC`, `1920x1080`), Audio Track #2 (`A_DTS`, `FlagDefault=1`, 6ch, 48kHz), Audio Track #3 (`A_TRUEHD`, `FlagDefault=0`, 8ch, 48kHz).
- **`TTMediaStreamInfo`**:
  - `has_video = true`, `video_desc.codec = TT_VIDEO_CODEC_H264 (1)`
  - `has_audio = true`, `audio_desc.codec = TT_AUDIO_CODEC_UNKNOWN (0)` (**never** `TT_AUDIO_CODEC_AAC (1)`!), `sample_rate = 48000.0`, `channel_count = 6`, `extradata = NULL`, `extradata_len = 0`
- **JSON Summary**:
  - `selected_video_track_number = 1`
  - `selected_audio_track_number = null` (no audio packets will be emitted by `ttmedia_demux_read_packet`)
  - `primary_audio_track_number = 2`
  - `selected_tracks[1]`: `container_track_number = 2`, `raw_codec_id = "A_DTS"`, `canonical_codec = null`, `c_audio_codec = 0`, `demux_selectable = false`, `is_primary_track = true`, `is_selected_for_packets = false`.
  - `tracks_catalog[2]`: `container_track_number = 3`, `raw_codec_id = "A_TRUEHD"`, `canonical_codec = null`, `c_audio_codec = 0`, `demux_selectable = false`, `is_primary_track = false`, `is_selected_for_packets = false`.

### Fixture S3: AAC With Valid `AudioSpecificConfig` vs. AAC Without `CodecPrivate`
- **Scenario S3a (With `CodecPrivate`)**:
  - Audio Track #2 (`A_AAC`, 2ch, 44100 Hz, `CodecPrivate = [0x12, 0x10]`).
  - `TTAudioFormatDesc`: `codec = TT_AUDIO_CODEC_AAC (1)`, `sample_rate = 44100.0`, `channel_count = 2`, `extradata != NULL`, `extradata_len = 2`, `slice(extradata, 2) == [0x12, 0x10]`.
  - JSON: `codec_private_present = true`, `codec_private_len = 2`, `codec_private_format = "aac_audio_specific_config"`, `codec_private_source = "track_entry_codec_private_0x63a2"`, `codec_private_hex = "1210"`, `codec_private_sha256 = "ba3d722a88cacce18f22962612eda4d50e7a61cd9b7db60694043efed2a61448"`.
- **Scenario S3b (Without `CodecPrivate` & Omitted `0xE1 Audio` Sub-Elements)**:
  - Audio Track #2 (`A_AAC`, `0xE1 Audio` omitted so container spec defaults apply, `0x63A2` omitted).
  - `TTAudioFormatDesc`: `codec = TT_AUDIO_CODEC_AAC (1)`, `sample_rate = 8000.0`, `channel_count = 1`, `extradata = NULL`, `extradata_len = 0`.
  - JSON: `codec_private_present = false`, `codec_private_len = 0`, `codec_private_format = "none"`, `codec_private_source = "absent"`, `codec_private_hex = null`, `codec_private_sha256 = null`, `sample_rate_source = "container_spec_default"`, `channels_source = "container_spec_default"`.

---

## 6. Real-Corpus C-ABI & Contract Telemetry (`REF-V4` / `REF-V5` / `REF-V6`)

> [!IMPORTANT]
> **Rust Real-Corpus Verification vs. Application Playback Acceptance**:
> The table below records **Rust native C-ABI (`ttmedia_demux_open` + `ttmedia_demux_query_tracks_json` + `ttmedia_demux_read_packet`) real-corpus inspection** with **zero `exists()` / early-return skip branches** (missing files fail `assert!(path.exists())`). Application-level end-to-end audio/video playback verification (`CoreAudio` / `AVSampleBufferAudioRenderer` / `AVSampleBufferDisplayLayer`) is tracked separately by **V2** (Swift consumer) and **P2** (application playback acceptance).

| Corpus | Video Track (`selected_video_track_number`) | Video `CodecPrivate` (`len` / `sha256`) | Audio Catalog & Selection (`selected_audio_track_number`) | Audio Parameters & `CodecPrivate` | First Video Packet (`stream_index`, `pts`, `len`, 8B head, `sha256`) | First Audio Packet (`stream_index`, `pts`, `len`, 8B head, `sha256`) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **`REF-V4`** (`[BD-Remux] 蓦然回首 Look Back 2024 [2160p HEVC-10bit HDR]...mkv`) | Track `#1`: `V_MPEGH/ISO/HEVC` (`canonical_codec = "hevc"`, `c_video_codec = 2`), `3840x2160` (`explicit_pixel_dimensions`) | `len = 131`<br>`format = "hvcc"`<br>`sha256 = 324ae6023a77bad1d04e6b4e1eaec60d6ab70fee6b94b6c2b72368690545214f` | Track `#2`: `A_DTS` (8ch, 48kHz, `demux_selectable = false`, `is_selected_for_packets = false`)<br>**Selected Track `#3`**: `A_AC3` (`canonical_codec = "ac3"`, `c_audio_codec = 10`, `is_selected_for_packets = true`) | `48000.0 Hz` (`explicit_sampling_frequency`), `6 ch` (`explicit_channels`), `codec_private_present = false`, `len = 0`, `sha256 = null` | `stream_index = 1`<br>`pts = 0 ms`<br>`len = 2129`<br>`head = 0000001840010c01`<br>`sha256 = d33ec4b93c426170f8e20945d6cd4900af44bed645e4b8738f9ed7cd63c243b8` | `stream_index = 3`<br>`pts = 0 ms`<br>`len = 1536`<br>`head = 0b77e8e01c30e1b4` (AC-3 `0x0B77` syncword)<br>`sha256 = 1389e4bb2ad2c5a7ff09454bc5f6a657e9ad446c84e60b93528d40ff98dd6cfc` |
| **`REF-V5`** (`Look.Back.2024.2160p.AMZN.WEB-DL.DDP5.1.H.265-OurBits.mkv`) | Track `#1`: `V_MPEGH/ISO/HEVC` (`canonical_codec = "hevc"`, `c_video_codec = 2`), `3836x2072` (`explicit_pixel_dimensions`) | `len = 132`<br>`format = "hvcc"`<br>`sha256 = 0b2a58941049b2b820f11c65fe303e4b58e1729fa7460d8b6075ad316d087b29` | **Selected Track `#2`**: `A_EAC3` (`canonical_codec = "eac3"`, `c_audio_codec = 11`, `is_selected_for_packets = true`) | `48000.0 Hz` (`explicit_sampling_frequency`), `6 ch` (`explicit_channels`), `codec_private_present = false`, `len = 0`, `sha256 = null` | `stream_index = 1`<br>`pts = 0 ms`<br>`len = 2654`<br>`head = 0000000346011000`<br>`sha256 = 78e2ebf0858c7c0e9a9d60e0e61791a46cd74e088ff97bd97a82f85e4d8a02d2` | `stream_index = 2`<br>`pts = 0 ms`<br>`len = 2560`<br>`head = 0b7704ff3f867ffa` (E-AC-3 `0x0B77` syncword)<br>`sha256 = 3202de6782fe0246f17b2870c94a184d7f2cbfab956abb883c4c31b77da9c383` |
| **`REF-V6`** (`Once.Upon.a.Time.in.America.1984.EXTENDED.1080p.BluRay.x264.DTS-WiKi.mkv`) | Track `#1`: `V_MPEG4/ISO/AVC` (`canonical_codec = "h264"`, `c_video_codec = 1`), `1920x1080` (`explicit_pixel_dimensions`) | `len = 72`<br>`format = "avcc"`<br>`sha256 = 3419ed6b5a0525492f41b35a058244743b2ff12da0dfebc7fbd48732cf7d69f6` | Track `#2`: `A_DTS` (6ch, `demux_selectable = false`)<br>Track `#3`: `A_DTS` (1ch, `demux_selectable = false`)<br>**Selected Track `#4`**: `A_AC3` (`canonical_codec = "ac3"`, `c_audio_codec = 10`, `is_selected_for_packets = true`)<br>Track `#5`: `A_AC3` (6ch, `is_selected_for_packets = false`) | `48000.0 Hz` (`explicit_sampling_frequency`), `5 ch` (`explicit_channels`), `codec_private_present = false`, `len = 0`, `sha256 = null` | `stream_index = 1`<br>`pts = 0 ms`<br>`len = 2449`<br>`head = 0000000209100000`<br>`sha256 = 6b7866b11b9b8d405cd40f215f317f540c10cc62a0fa809bcb737657fa21b87b` | `stream_index = 4`<br>`pts = 0 ms`<br>`len = 2560`<br>`head = 0b773b032430e0dc` (AC-3 `0x0B77` syncword)<br>`sha256 = 77f5ae971687ec6ae483c9d021b2a393be0e4f8a03ce4b5ce1bab98c1227e24e` |

---

## 7. Raw Verification Logs (Zero `|| true`, Zero `exists()` Skips)

### 7.1 `cargo clippy` & `cargo test --test test_track_metadata_contract` & `cargo test --lib`
```text
$ cargo clippy --manifest-path infra/ttmedia/Cargo.toml --lib --test test_track_metadata_contract -- -D warnings && cargo test --manifest-path infra/ttmedia/Cargo.toml --test test_track_metadata_contract -- --nocapture && cargo test --manifest-path infra/ttmedia/Cargo.toml --lib -- demux:: --nocapture
    Checking ttmedia v0.1.0 (../../infra/ttmedia)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.68s
   Compiling ttmedia v0.1.0 (../../infra/ttmedia)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.58s
     Running tests/test_track_metadata_contract.rs (infra/ttmedia/target/debug/deps/test_track_metadata_contract-ba1462c6c2a3dde5)

running 8 tests
test test_contract_6_query_tracks_json_buffer_bounds_null_pointers_and_no_truncation ... ok
test test_contract_1_dts_before_ac3_selects_ac3_and_matches_first_packet_stream_index ... ok
test test_contract_2_dts_and_truehd_only_preserves_raw_codec_and_c_abi_never_disguises_as_aac ... ok
test test_contract_4_multi_track_codec_private_isolation_and_field_order_invariance ... ok
test test_contract_7_seek_invariance_of_track_metadata_and_post_seek_packets ... ok
test test_contract_3_aac_codec_private_exact_passthrough_and_missing_explicitly_absent ... ok
test test_contract_5_non_standard_track_numbers_and_invalid_parameters_rejected ... ok
[REF-V4] v_track=#1 (V_MPEGH/ISO/HEVC 3840x2160, cp_len=131, cp_sha256=Some("324ae6023a77bad1d04e6b4e1eaec60d6ab70fee6b94b6c2b72368690545214f")) | a_track=#3 (A_AC3 48000Hz 6ch, cp_len=0, cp_sha256=None) | first_v=(stream_idx=1, pts=0ms, len=2129, head=0000001840010c01, sha256=d33ec4b93c426170f8e20945d6cd4900af44bed645e4b8738f9ed7cd63c243b8) | first_a=(stream_idx=3, pts=0ms, len=1536, head=0b77e8e01c30e1b4, sha256=1389e4bb2ad2c5a7ff09454bc5f6a657e9ad446c84e60b93528d40ff98dd6cfc)
[REF-V5] v_track=#1 (V_MPEGH/ISO/HEVC 3836x2072, cp_len=132, cp_sha256=Some("0b2a58941049b2b820f11c65fe303e4b58e1729fa7460d8b6075ad316d087b29")) | a_track=#2 (A_EAC3 48000Hz 6ch, cp_len=0, cp_sha256=None) | first_v=(stream_idx=1, pts=0ms, len=2654, head=0000000346011000, sha256=78e2ebf0858c7c0e9a9d60e0e61791a46cd74e088ff97bd97a82f85e4d8a02d2) | first_a=(stream_idx=2, pts=0ms, len=2560, head=0b7704ff3f867ffa, sha256=3202de6782fe0246f17b2870c94a184d7f2cbfab956abb883c4c31b77da9c383)
[REF-V6] v_track=#1 (V_MPEG4/ISO/AVC 1920x1080, cp_len=72, cp_sha256=Some("3419ed6b5a0525492f41b35a058244743b2ff12da0dfebc7fbd48732cf7d69f6")) | a_track=#4 (A_AC3 48000Hz 5ch, cp_len=0, cp_sha256=None) | first_v=(stream_idx=1, pts=0ms, len=2449, head=0000000209100000, sha256=6b7866b11b9b8d405cd40f215f317f540c10cc62a0fa809bcb737657fa21b87b) | first_a=(stream_idx=4, pts=0ms, len=2560, head=0b773b032430e0dc, sha256=77f5ae971687ec6ae483c9d021b2a393be0e4f8a03ce4b5ce1bab98c1227e24e)
test test_real_corpora_ref_v4_v5_v6_c_abi_and_metadata_contract ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

     Running unittests src/lib.rs (infra/ttmedia/target/debug/deps/ttmedia-348295f54b733ccb)

running 8 tests
test demux::mkv_demux_tests::test_v1_t2_t3_multi_audio_selection_and_independent_codec_private ... ok
test demux::mkv_demux_tests::test_v1_t4_unknown_codecs_preserved_never_disguised_as_aac ... ok
test demux::mkv_seek_tests::test_v1_t6_cues_index_seek_and_unindexed_fallback ... ok
test demux::mkv_demux_tests::test_v1_t1_track_entry_field_order_permutations_and_decoy_immunity ... ok
test demux::mkv_seek_tests::test_v1_t6_unindexed_fallback_cancellation_and_budget ... ok
test demux::mkv_demux_tests::test_v1_t5_corrupted_inputs_rejected_cleanly ... ok
[REF-V6 seek 600s] sync_pts=599974ms bytes_read=1388030 syscalls=2 blocks=1 elapsed=29.676834ms
[REF-V6 seek 1800s] sync_pts=1799631ms bytes_read=65536 syscalls=1 blocks=1 elapsed=309.292µs
[REF-V6 seek 3600s] sync_pts=3599721ms bytes_read=65536 syscalls=1 blocks=1 elapsed=160.542µs
test demux::mkv_seek_tests::test_v1_t7_real_corpus_ref_v6_ac3_selection_and_cues_seek ... ok
[REF-V4 seek 600s] sync_pts=598056ms bytes_read=1429251 syscalls=2 blocks=1 elapsed=30.681875ms
[REF-V4 seek 1800s] sync_pts=1798841ms bytes_read=65536 syscalls=1 blocks=1 elapsed=160.959µs
[REF-V4 seek 3600s] sync_pts=3599224ms bytes_read=65536 syscalls=1 blocks=1 elapsed=143.667µs
[REF-V5 seek 600s] sync_pts=597667ms bytes_read=307029 syscalls=2 blocks=1 elapsed=5.382417ms
[REF-V5 seek 1800s] sync_pts=1797917ms bytes_read=65536 syscalls=1 blocks=1 elapsed=213.125µs
[REF-V5 seek 3600s] sync_pts=3596917ms bytes_read=65536 syscalls=1 blocks=1 elapsed=134.042µs
test demux::mkv_seek_tests::test_v1_t7_real_corpora_ref_v4_and_v5_tracks_and_cues_seek ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 41 filtered out; finished in 0.04s
```

### 7.2 Bazel Build & Test + Scoped Quality Gates (Zero `|| true`)
```text
$ bazel build //infra/ttmedia:ttmedia //infra/ttmedia:TTMediaCABI && bazel test //infra/ttmedia:ttmedia_test
Target //infra/ttmedia:TTMediaCABI up-to-date:
  bazel-bin/infra/ttmedia/libTTMediaCABI.lo
Target //infra/ttmedia:ttmedia up-to-date:
  bazel-bin/infra/ttmedia/libttmedia-621770663.rlib
//infra/ttmedia:ttmedia_test                                             PASSED in 0.8s
Executed 1 out of 1 test: 1 test passes.

$ python3 tools/ci/lint_loc_gate.py --dirs infra/ttmedia/src/demux infra/ttmedia/src/c_abi.rs infra/ttmedia/include/ttmedia_demux.h infra/ttmedia/Tests/test_track_metadata_contract.rs --max-loc 800 --min-files 1 --no-baseline
Scanning target roots: infra/ttmedia/src/demux, infra/ttmedia/src/c_abi.rs, infra/ttmedia/include/ttmedia_demux.h, infra/ttmedia/Tests/test_track_metadata_contract.rs (Ceiling: 800 LOC)
Scanned 10 source files (4,587 total physical LOC).
✅ LOC GATE PASSED: All 10 source files strictly conform to the <= 800 LOC gate.

$ python3 tools/ci/lint_code_quality.py --strict --no-baseline --dirs infra/ttmedia/src/demux infra/ttmedia/src/c_abi.rs infra/ttmedia/Tests/test_track_metadata_contract.rs
Thresholds: File LOC <= 800 | Fn LOC <= 80 | McCabe CC <= 15
Scanned 8 files (3,726 physical LOC). Analyzed 133 functions.
📊 Function LOC Distribution:
    <=20 LOC :    65 ( 48.9%)
   21-50 LOC :    57 ( 42.9%)
   51-80 LOC :    11 (  8.3%)
     >80 LOC :     0 (  0.0%)
📊 McCabe Cyclomatic Complexity Distribution:
     1-5 CC  :    74 ( 55.6%)
    6-10 CC  :    48 ( 36.1%)
   11-15 CC  :    11 (  8.3%)
     >15 CC  :     0 (  0.0%)
✅ QUALITY GATE PASSED: All source files and architecture invariants strictly conform to standards.

$ python3 tools/ci/normalize_headers.py --check
[Found] 0 non-conforming file(s).
```

---

## 8. Handoff Notes for H (Build Integration), V2 (Swift Consumer), and P2 (Playback Acceptance)

1. **For H (`infra/ttmedia/BUILD.bazel`)**:
   - V1 respected the exclusive file boundary and did **not** modify `infra/ttmedia/BUILD.bazel` or `infra/ttmedia/Cargo.toml`.
   - When H integrates the shared build targets, please register `Tests/test_track_metadata_contract.rs` as a `rust_test` target in `infra/ttmedia/BUILD.bazel`:
     ```python
     rust_test(
         name = "test_track_metadata_contract",
         srcs = ["Tests/test_track_metadata_contract.rs"],
         deps = [":ttmedia"],
     )
     ```
2. **For V2 (Swift Media Pipeline Consumer)**:
   - Call `ttmedia_demux_query_tracks_json(ctx, nil, 0, &requiredLen)` after `ttmedia_demux_open`, allocate `requiredLen + 1` bytes, and call `ttmedia_demux_query_tracks_json(ctx, buf, requiredLen + 1, &writtenLen)`.
   - Use `selected_video_track_number` and `selected_audio_track_number` to match `TTMediaPacket.stream_index`.
   - When `selected_audio_track_number == null` (e.g., DTS/TrueHD-only files where `primary_audio_track_number != null`), `TTMediaStreamInfo.audio_desc.codec` is `TT_AUDIO_CODEC_UNKNOWN (0)` and `tracks_catalog` exposes the true `raw_codec_id` (`"A_DTS"`, `"A_TRUEHD"`) so Swift can report unsupported audio diagnostics without attempting to initialize an AAC `CMAudioFormatDescription`.
   - Do **not** infer `AudioChannelLayout` solely from `channels`; `channel_layout_source` is explicitly `"absent_in_mkv_track_entry"` when the container does not carry an explicit channel layout descriptor.
3. **For P2 (Application Playback Acceptance)**:
   - Rust-level demuxer and C-ABI verification on `REF-V4`, `REF-V5`, and `REF-V6` is complete and recorded in Section 6.
   - Application-level verification (`CoreAudio` / `AVSampleBufferAudioRenderer` decoding of AC-3/E-AC-3 tracks `#3`, `#2`, `#4` and `AVSampleBufferDisplayLayer` rendering after Cues seek) belongs to P2's real-app acceptance stage after V2 + H integration.

---

## 9. Final File SHA-256 Digest Inventory & Write-Freeze Status

- **V1 Write Status**: **FROZEN (停止写入)** — All files within V1's exclusive scope are finalized and frozen for H / V2 integration.

| File Path | SHA-256 Digest |
| :--- | :--- |
| `infra/ttmedia/src/demux/annexb.rs` | `7bb61d30995b949df42dad5fc9ecf8d9bfc22edeb8d703a8fc148249dac60d83` |
| `infra/ttmedia/src/demux/ebml.rs` | `36fd552786b9a4d2e9b8ef8e2c298fdd094f8fdaac5e1282dc184fe9bbc439d9` |
| `infra/ttmedia/src/demux/format_demuxer.rs` | `fa2e3620066a10e55be66307b18ac279eb95efc439bc6a29c0a6accd601c8617` |
| `infra/ttmedia/src/demux/mkv_demux_tests.rs` | `35f680d8fd4f3fb0149efeeac0f88f5da164873479f4adef9cf9dea42171b6e7` |
| `infra/ttmedia/src/demux/mkv_fallback_seek.rs` | `e57ea98628ee831e61f90c2c02f6b64eb7505113a62553fc61131fb536362e67` |
| `infra/ttmedia/src/demux/mkv_index.rs` | `0ce2f1c0f03b7347767a5caa8341ac9ba3fd6aa8ea3b7a806df1e5980bb640ad` |
| `infra/ttmedia/src/demux/mkv_metadata_contract.rs` | `dfc1fdeff317e15dbbbda83ad8b9634217cac3b84de63192466d1010f24b050a` |
| `infra/ttmedia/src/demux/mkv_seek_tests.rs` | `b46b1f8fe2ab64a217ff0c8585db43cb7b2f6a800837443d371c025933d64c97` |
| `infra/ttmedia/src/demux/mkv_tracks.rs` | `09aad32ea4ba693e606ef1cf7ca8e2655b336d360cc3da092cb87c2075a19688` |
| `infra/ttmedia/src/demux/mod.rs` | `2444a6506ea0958182823d598afd2b618f00e068b4dc233e944d0a348faaad80` |
| `infra/ttmedia/src/c_abi.rs` | `f44d681f33b4a3b4e29ccd81a3eb6f64d96697974ab2450ce41d6c77cd226b52` |
| `infra/ttmedia/include/ttmedia_demux.h` | `7b0689c38d9e848d3feadd7bce9cdca30ba9b050988bf1f0017901441a5a75a4` |
| `infra/ttmedia/Tests/test_track_metadata_contract.rs` | `3476d97daf7e1300750caf824054467ad343bb1236377e085815b336f77794df` |
| `infra/ttmedia/Tests/test_format_demuxer.rs` | `7ef73340120737d170ea9d998b86a9deb44121c90ff0076b70238a499b7742d1` |
| `infra/ttmedia/Tests/test_worker_integration.rs` | `cb6ca0a9a2538f189eefe12c2395112dd00022b15eaeadacf3b230d0a3f1a807` |
| `infra/ttmedia/Tests/TTMediaKitTests/MediaWorkerTransportLifecycleTests.swift` | `eb613b2dec98032673d5b4d15d625b11fdfbc6f2cac2b8314401b618f08e9062` |
| `infra/ttmedia/Tests/TTMediaKitTests/MediaWorkerTransportTestDoubles.swift` | `90307636ec0f490bd84640e22162e9a5bbc5d65e03f333e1e31a21792cffeffb` |
| `infra/ttmedia/Tests/TTMediaKitTests/SubtitleRequestCancellationTests.swift` | `aa0b70e294e1a752a3ea6c1e3fb33fd94c6644923cb9a30146be76e92a140983` |


