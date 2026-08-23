# Feature Specification: 073-media-preview-audit-tui-engine-acceleration

## Title
Media Preview Deep Audit & Polish, Interactive Terminal TUI Mode (`explore`), and Core SIMD Decompression Acceleration

## Metadata
- **Feature Directory**: `specs/073-media-preview-audit-tui-engine-acceleration/`
- **Created**: 2026-08-18
- **Status**: Draft / In Progress
- **Target Branch**: `main`
- **Priority**: P1

---

## 1. Executive Summary

TTZip currently delivers a powerful multi-format native desktop app and an industrial-grade CLI with pipe streaming, dynamic completions, and 6-stage CI gates.

This feature advances three synergistic engineering pillars:
1. **Desktop Media Preview & Finder Drag-Drop Deep Audit & Polish**: Deeply inspect and harden the media preview matrix (Images, Video, Audio waveform, PDF, EPUB, Syntax-highlighted code, Docx, QuickLook) in `Sources/TTZipApp/Views/MediaPreviewView.swift`, `MediaPreviewFactory.swift`, and `MillerColumnItemRowView.swift`. Fix any player leaks, downsample 50MP+ images, and ensure flawless Finder file promise drag-and-drop.
2. **CLI Interactive Terminal TUI Mode (`ttzip-cli explore`)**: Provide an interactive, zero-dependency ANSI/VT100 terminal user interface for exploring compressed archives with Vim/arrow-key navigation, spacebar multi-select, in-terminal syntax preview, and selective extraction.
3. **Core LZ4/ZSTD SIMD Decompression Acceleration**: Optimize streaming decompression pipelines with Direct-IO page-aligned memory mapping, ARM NEON match unrolling, and interleaved PMULL hardware checksumming.

---

## 2. User Scenarios & Personas

### Scenario 1: macOS Power User (Desktop Media Preview & Finder Drag-and-Drop)
As a macOS desktop user inspecting a multi-gigabyte archive in `TTZipApp`, I want to select image, audio, video, PDF, or code files in the sidebar and get instant, fluid preview artboards, toggle full-screen mode, trigger Spacebar Quick Look, and drag files directly to Finder/Desktop without experiencing memory leaks, audio playback hangs, or UI freezes.

### Scenario 2: Server / Terminal Power User (CLI Interactive TUI Mode)
As a developer working in a remote SSH terminal or CLI environment, I want to execute `ttzip-cli explore archive.tar.zst` to interactively navigate the archive's internal directory tree using arrow keys or Vim bindings (`j`/`k`/`h`/`l`), press `p` to view text/hex content, mark multiple entries with `Space`, and extract only the selected files via `e` with standard exit codes.

### Scenario 3: Performance Architect (Core SIMD Decompression Acceleration)
As an engineer processing high-throughput compressed streams, I want LZ4 and Zstandard decompression pipelines to utilize Direct-IO zero-copy buffers and hardware-accelerated NEON/PMULL verification, ensuring maximum multi-gigabyte-per-second extraction throughput without regression.

---

## 2.1 Clarifications & Design Decisions

- **C1 (Non-TTY Fallback)**: If `!isatty(STDIN_FILENO) || !isatty(STDOUT_FILENO)`, `ttzip-cli explore` automatically falls back to standard non-interactive tree/table listing (`ttzip-cli list`) without entering raw mode.
- **C2 (TUI Peek Truncation)**: In-terminal file peek (`p`) reads up to 64KB / 1,000 lines for text files and the first 256 bytes formatted with `FastHexDiffEngine` for binary files.
- **C3 (High-DPI Downsampling)**: Image previews for bitmaps exceeding 4096px dimension use CoreGraphics `CGImageSourceCreateThumbnailAtIndex` with `maxPixelSize: 2048` to cap single-image RAM usage under 16MB.
- **C4 (Audio/Video Resource Cleanup)**: `VideoAudioPlayerPreviewView` and `AudioWaveformVisualizerView` pause playback, clear observers, and set player instance to `nil` inside SwiftUI `.onDisappear`.

---

## 3. Functional Requirements

- **FR-001 [Media Preview Memory & Lifecycle Hardening]**:
  - Image preview (`InteractiveZoomImageView`) must automatically downsample images > 4096px to screen resolution using `CGImageSourceCreateThumbnailAtIndex` to eliminate RAM spikes.
  - Video/Audio preview (`VideoAudioPlayerPreviewView`, `AudioWaveformVisualizerView`) must properly release `AVPlayer` / `AVPlayerItem` on `.onDisappear` or URL change to prevent background audio playback leaks.
  - EPUB/Docx/Code preview must clamp maximum loaded text slice (<= 5MB) to prevent WebKit / TextKit memory inflation on oversized log files.
- **FR-002 [Interactive Terminal TUI Engine (`ttzip-cli explore`)]**:
  - Implement `InteractiveTUIExplorer` in `Sources/TTZipCore/CLI/TUI/InteractiveTUIExplorer.swift`.
  - Wire `ttzip-cli explore <archive> [--password <pwd>]` in `CLICommandRouter.swift` and `CLICommandSpec.swift`.
  - Handle raw terminal mode via POSIX `termios` (`tcgetattr`, `tcsetattr`, `cfmakeraw`).
  - Key bindings:
    - `↑` / `k`: Move cursor up
    - `↓` / `j`: Move cursor down
    - `Enter` / `→` / `l`: Enter directory / drill down
    - `Backspace` / `←` / `h`: Return to parent directory
    - `Space`: Toggle entry selection checkbox
    - `p`: Peek preview (syntax-highlighted text / formatted hex dump)
    - `e`: Extract selected or current item to current working directory
    - `q` / `Esc` / `Ctrl+C`: Cleanly exit TUI and restore terminal state
- **FR-003 [Core SIMD Decompression & Zero-Copy Pipeline]**:
  - Optimize LZ4 and ZSTD block stream decoders with zero-allocation page buffers.
  - Interleave PMULL hardware CRC calculation during block stream decoding to eliminate redundant post-extraction hashing passes.

---

## 4. Non-Functional Requirements & Invariants

- **NFR-001 (Zero Terminal Glitch)**: TUI explorer must cleanly trap signals (`SIGINT`, `SIGTERM`, `SIGWINCH`) and restore standard terminal canonical mode and cursor visibility under all exit conditions.
- **NFR-002 (Zero Third-Party TUI Dependency)**: TUI engine must be 100% written in Swift + POSIX C with zero external ncurses / termbox package dependencies.
- **NFR-003 (UI 60/120fps Fluidity)**: Media preview transitions must maintain 60fps/120fps frame rates with 0 UI thread beachballing.
- **NFR-004 (Hard Performance Floor)**: All 13 throughput gates in `XCTestPerformanceMeasureTests` and 5 frontend rendering gates in `FrontendPerformanceGateTests` must pass 100%.

---

## 5. Success Criteria & Verification Metrics

1. `swift test --filter MediaPreviewAuditTests` passes 100%, verifying image downsampling, AV player resource release, and text truncation bounds.
2. `swift test --filter InteractiveTUITests` passes 100%, verifying TUI state machine, key navigation, selection tracking, and terminal restoration.
3. `swift test --filter XCTestPerformanceMeasureTests` and `./scripts/run_local_ci_gate.sh` pass all 6 stages cleanly with zero regressions.
