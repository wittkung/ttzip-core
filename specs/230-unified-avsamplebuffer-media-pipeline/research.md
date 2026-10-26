# Research & Architectural Spikes: Unified AVSampleBuffer Media Pipeline (Feature 230)

**Feature**: `230-unified-avsamplebuffer-media-pipeline`

**Status**: Completed

---

## R1. Hermetic C Library Compilation: Whitelist FFmpeg (libavformat) & libdav1d

### Question
How to hermetically compile FFmpeg (`libavformat` pure demuxer) and `libdav1d` under Bazel 8 without linking any `/opt/homebrew` dylibs, while ensuring LGPL compliance and zero patent risk?

### Findings & Evidence
1. **Compilation Framework**:
   - Bazel BCR provides `rules_foreign_cc` (`bazel_dep(name = "rules_foreign_cc", version = "0.13.0")`), which supports `configure_make` for autotools-based projects like FFmpeg and `meson` for `dav1d`.
   - Alternatively, FFmpeg's minimal demuxing core (`libavformat/mov.c`, `matroskadec.c`, `avidec.c`, `flvdec.c`, `mpegts.c`, `mp3dec.c`, `wavdec.c`, `oggdec.c`) depends strictly on `libavcodec` bitstream parsers and `libavutil`.
2. **Strict Whitelist Configure Flags**:
   ```bash
   ./configure \
     --prefix="${INSTALL_DIR}" \
     --disable-everything \
     --disable-programs \
     --disable-doc \
     --disable-network \
     --disable-autodetect \
     --disable-iconv \
     --disable-securetransport \
     --enable-demuxer=mov,matroska,webm,avi,flv,mpegts,mp3,aac,flac,ogg,wav \
     --enable-parser=h264,hevc,av1,vp9,opus,flac,vorbis,mpegaudio,aac \
     --disable-decoders \
     --enable-libdav1d \
     --enable-decoder=libdav1d,vp9,opus,flac,vorbis,alac,mp3,pcm_s16le,pcm_s24le \
     --enable-protocol=file \
     --enable-shared \
     --disable-static \
     --extra-cflags="-fvisibility=hidden -mmacosx-version-min=14.0" \
     --extra-ldflags="-Wl,-rpath,@loader_path"
   ```
3. **Patent & Legal Isolation (ADR-017 & Piper Contract)**:
   - Patented video codecs (H.264, HEVC, ProRes) and audio codecs (AAC) have their software decoders **explicitly disabled** (`--disable-decoders`). The worker extracts NALU/ADTS frames only and delivers them to Apple Silicon hardware decoders, which are royalty-paid by Apple at the hardware level (Quanta 2008, *exhaustion doctrine*).
   - Building `libavformat.dylib` as a standalone dynamic library inside `ttmedia-worker.app/Contents/Frameworks/` complies with LGPL 2.1 §6 (reverse engineering and user relinking permitted).
   - Source: [FFmpeg configure options](https://ffmpeg.org/ffmpeg-all.html), Apple Legal Hardened Runtime Guideline.

---

## R2. CoreMedia XPC Serialization & Annex-B to AVCC Conversion

### Question
How to transport video/audio frames across XPC from `ttmedia-worker` to the host application without `CMSampleBuffer` serialization failure, and how to resolve container bitstream format mismatches?

### Findings & Evidence
1. **CoreMedia XPC Non-Transferability**:
   - `CMSampleBufferRef` and `CMFormatDescriptionRef` are CoreMedia opaque reference types and do not conform to `NSSecureCoding`.
   - Attempting to pass them directly across `NSXPCConnection` results in an XPC serialization exception.
2. **DTO Decomposition & Zero-Copy Reconstruction**:
   - The worker decomposes every media packet into a lightweight `MediaPacketDTO`:
     - `payloadData: Data` (or memory descriptor for zero-copy UMA sharing)
     - `presentationTimeStamp: CMTime` (value, timescale, flags, epoch)
     - `decodeTimeStamp: CMTime`
     - `duration: CMTime`
     - `formatParameters: VideoFormatParameters?` (SPS/PPS/VPS extradata, width, height, codecType)
     - `flags: PacketFlags` (`isKeyframe`, `doNotDisplay`)
   - The host calls `CMVideoFormatDescriptionCreateFromH264ParameterSets` (or HEVC equivalent) to cache the format description, constructs a `CMBlockBuffer` from the data, and calls `CMSampleBufferCreateReady`.
3. **Annex-B to AVCC Conversion**:
   - MPEG-TS and raw elementary streams format H.264/HEVC with Annex-B start codes (`00 00 00 01` or `00 00 01`).
   - `AVSampleBufferDisplayLayer` and VideoToolbox hardware decoders strictly mandate AVCC (H.264) or HVCC (HEVC) with a 4-byte big-endian NALU length prefix.
   - The worker parses NALU boundaries during packet extraction and converts start codes into 4-byte big-endian lengths in-place in the packet buffer before XPC transit, incurring zero reallocation overhead.
   - Source: ISO/IEC 14496-15 (Carriage of network abstraction layer (NAL) unit structured video in the ISO Base Media File Format).

---

## R3. Single Master Clock & Synchronization (ASBDL + AudioRenderer)

### Question
How does `AVSampleBufferRenderSynchronizer` govern audio/video lip-sync without dynamic audio-delay injection or frame skipping?

### Findings & Evidence
1. **Unified Timebase Architecture**:
   - `AVSampleBufferRenderSynchronizer` manages an internal `CMTimebase` driven directly by CoreAudio hardware output.
   - Both `AVSampleBufferDisplayLayer` and `AVSampleBufferAudioRenderer` are added to the synchronizer:
     ```swift
     synchronizer.addRenderer(displayLayer)
     synchronizer.addRenderer(audioRenderer)
     synchronizer.setRate(1.0, time: startTime)
     ```
2. **Lip-Sync Precision**:
   - Hardware decoders output frames timestamped with `presentationTimeStamp`.
   - CoreAudio hardware clock drives the master timebase. When audio lags or leads, the synchronizer automatically holds or drops video frames at the display layer level according to the hardware clock.
   - Dynamic software audio delays (such as `mpv --audio-delay`) are strictly prohibited; timestamps are normalized relative to container epoch 0, ensuring lip-sync drift remains within ATSC IS-191 thresholds ($[-15\text{ms}, +45\text{ms}]$).
3. **Picture-in-Picture (PiP)**:
   - `AVPictureInPictureController(sampleBufferDisplayLayer: displayLayer)` natively hooks into ASBDL. No offscreen framebuffer blits or private APIs are necessary.
   - Source: Apple Developer Documentation: `AVSampleBufferRenderSynchronizer`, `AVPictureInPictureController`.

---

## R4. Preroll & Non-Keyframe Seek Handling

### Question
How to execute instantaneous, artifact-free seeking to arbitrary timestamps $T$?

### Findings & Evidence
1. **Keyframe Alignment Rule**:
   - `libavformat` seeks to the nearest keyframe $\le T$ via `av_seek_frame(fmt_ctx, stream_idx, target_ts, AVSEEK_FLAG_BACKWARD)`.
2. **Preroll Marking**:
   - Frames with $PTS < T$ are sent to the host with the `doNotDisplay` flag set.
   - Host attaches `kCMSampleAttachmentKey_DoNotDisplay = kCFBooleanTrue` to the `CMSampleBuffer`.
   - VideoToolbox decodes the frame to populate the reference frame buffer (DPB), but ASBDL suppresses rendering to the display.
   - The first frame rendered is the exact requested frame at timestamp $T$, guaranteeing zero visual tearing or macroblock smear.
   - Source: Apple Developer Documentation: `kCMSampleAttachmentKey_DoNotDisplay`.

---

## R5. Process Lifecycle & Two-Tier Sleep State Machine

### Question
How to prevent resource leaks and background battery drain from the auxiliary worker?

### Findings & Evidence
1. **Lifecycle States**:
   - `ActivePlayback`: Actively streaming packets to host.
   - `WarmStandby`: Initiated 30 seconds after media playback is paused or closed. Demuxers and active decoders are closed; XPC connection remains open; wake-up latency $\le 10\text{ ms}$.
   - `ColdHibernation`: Initiated 300 seconds (5 minutes) after entering standby. Worker terminates cleanly via `exit(0)`.
2. **Crash Resilience**:
   - Host monitors XPC connection invalidation (`connection.invalidationHandler`). If the worker crashes due to hostile media input, the host cleans up the layer and displays an inline media error banner without crashing the main desktop window.
