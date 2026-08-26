# Tasks: Real Audio Waveform Extraction & Workspace UI Layout Harmonization

**Feature Identifier**: `002-real-audio-waveform`  
**Classification**: `[Lean SDD]`  
**Status**: `Completed`

---

## Phase 1: Rust Microkernel Downsampling & PCM Audio Engine

- [x] T001 Create `audio` module in `core/rust/ttzip-engine/src/audio/` supporting RIFF WAV, AIFF/AIFC PCM parsing, generic compressed stream energy estimation, and deterministic fallback waveform.
- [x] T002 Expose C-ABI exports `ttzip_extract_audio_waveform` and `ttzip_extract_audio_waveform_from_memory` in `core/rust/ttzip-engine/src/ffi/audio_ffi.rs`.
- [x] T003 Build and package universal `libTTZipVendor.a` into `core/Vendor/TTZipVendor.xcframework` and update `ttzip_rust_glue.h`.

## Phase 2: Core SDK & App Services Bridge

- [x] T004 Add `NativeMicrokernelBridge.extractAudioWaveform(path:bucketCount:)` and `extractAudioWaveformFromMemory(data:bucketCount:)` in `core/Sources/TTZipCore/Bridge/ArchiveEngineBridge.swift`.
- [x] T005 Refactor `AudioWaveformExtractor.swift` in `apple/Sources/TTZipApp/Services/AudioWaveformExtractor.swift` to invoke Rust microkernel with AVFoundation secondary fallback and thread-safe actor caching.

## Phase 3: UI Layout Harmonization & Waveform Presentation

- [x] T006 [US1] Remove fixed `680x600` modal bounding box in `apple/Sources/TTZipApp/Views/CompressModalView.swift`, upgrading it to a fluid responsive workspace (`maxWidth: 900`, centered with generous padding).
- [x] T007 [US1] Fix `MainView.swift` so the right inspector panel is only shown on the `.home` file explorer tab when a disk item is active, preventing background file previews from crowding the "新建归档" workspace.
- [x] T008 [US2] Update `AudioWaveformVisualizerView.swift` & `UnifiedAudioPlayerView.swift` to render 48 wide bars spanning the full width of the audio player card aligned with the time scrubber.

## Phase 4: Automated Testing & Verification

- [x] T009 [P] [US3] Unit tests in `core/rust/ttzip-engine/src/audio/tests.rs` (205 Rust tests passing).
- [x] T010 [P] [US3] Unit tests in `apple/Tests/TTZipAppTests/AudioWaveformExtractionTests.swift` (116 Swift tests passing, 60 Core tests passing).
