# Spec: Real Audio Waveform Extraction & Playback Synchronization

**Feature Identifier**: `002-real-audio-waveform`  
**Classification**: `[Lean SDD]` (Internal Audio Visualization & Waveform Extraction)  
**Status**: `Draft / Ready for Implementation`

---

## 1. Background & Problem Statement

Currently, `AudioWaveformVisualizerView` renders a synthetic sine/cosine mock wave during audio playback. Users want to see the **real acoustic waveform** extracted directly from the actual audio file (e.g. MP3, FLAC, WAV, AAC, M4A, OGG), synchronized with playback progress and interactive seeking.

## 2. User Scenarios

### Scenario 1: Real Waveform Display on Audio Load
- **Given** an audio file is selected in TTZip,
- **When** the audio preview opens,
- **Then** the visualizer asynchronously extracts and renders the real amplitude peaks (true track waveform profile).

### Scenario 2: Playback Progress Synchronization & Active Bar Glow
- **Given** an audio file is playing,
- **When** the playhead advances from 0:00 to the end,
- **Then** bars representing elapsed time are filled with active Bamboo Green / Kintsugi Gold gradient,
- **And** the active playing bar has dynamic micro-energy bounce,
- **And** future bars remain translucent.

### Scenario 3: Interactive Waveform Seeking
- **Given** the user views the waveform,
- **When** the user clicks or drags anywhere along the waveform,
- **Then** playback instantly seeks to the corresponding timestamp in the audio track.

## 3. Functional Requirements

1. **`AudioWaveformExtractor` Actor**:
   - Asynchronously reads audio sample buffers via `AVAssetReader` into Linear PCM.
   - Computes normalized peak / RMS amplitudes for `N` bins (e.g. 36 bars).
   - In-memory thread-safe actor caching.
2. **`AudioWaveformVisualizerView` Refactoring**:
   - Accepts `url: URL?`, `isPlaying: Bool`, `currentTime: Double`, `duration: Double`, and `onSeek: ((Double) -> Void)?`.
   - Renders elapsed vs remaining bars with active gradients.
   - Supports tap gesture to seek.
3. **`UnifiedAudioPlayerView` Integration**:
   - Passes `currentTime`, `duration`, `url`, and seek callback to `AudioWaveformVisualizerView`.

## 4. Success Criteria

- [x] Audio player displays the actual acoustic peaks of loaded audio files.
- [x] Waveform progress fills from left to right as audio plays.
- [x] Clicking waveform seeks accurately.
- [x] Automated unit and integration tests pass with 100% success.
