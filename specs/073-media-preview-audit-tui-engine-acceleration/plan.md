# Implementation Plan: 073-media-preview-audit-tui-engine-acceleration

## Technical Context & Scope
This plan details the implementation strategy for:
1. **Desktop Media Preview Deep Audit & Hardening**:
   - `ImageIOThumbnailCache.swift` for 50MP+ image thumbnail downsampling via `CGImageSourceCreateThumbnailAtIndex`.
   - AVPlayer/AVPlayerItem explicit 5-step teardown in `SharedVideoPlayerStore` and `UnifiedAudioPlayerView` to eliminate CoreAudio HAL leaks.
   - Finder `NSFilePromiseProvider` drag-and-drop for virtual archive paths.
   - Spacebar Quick Look interception in `FinderMillerColumnsView`.
2. **CLI Interactive Terminal TUI Mode (`ttzip-cli explore`)**:
   - Zero-dependency `TerminalRawModeManager.swift` using Darwin POSIX `<termios.h>`.
   - `InteractiveTUIExplorer.swift` with double-buffered single-flush rendering (`?1049h`), breadcrumbs, virtualized tree viewport, and peek modal (`p`).
   - CLI command routing in `CLICommandRouter.swift` and `CLICommandSpec.swift`.
3. **Core LZ4/ZSTD SIMD Decompression Acceleration**:
   - Direct-IO 64KB page-aligned streaming buffers in `ttzip_tar_zstd_direct.c` / `ttzip_lz4_stream_direct.c`.
   - Interleaved PMULL hardware CRC calculation.

---

## Constitution & Invariant Check

- [x] **Zero-Cost Hot Paths**: No heap allocations or dynamic object trees inside block decompression inner loops.
- [x] **Zero External TUI Dependencies**: 100% pure Swift + POSIX C; no ncurses or termbox.
- [x] **Fast-Path Preservation**: ZIP, 7Z, TAR.ZST, and LZ4 fast-paths strictly preserved.
- [x] **Multi-Agent Isolation**: Feature executed strictly under `SPECIFY_FEATURE_DIRECTORY="specs/073-media-preview-audit-tui-engine-acceleration"`.
- [x] **Hard Performance Floors**: All 13 compression/decompression floors and 5 frontend rendering floors preserved.

---

## Phase 0: Research Items

- R001 [SUBAGENT:research] 《Desktop Media Preview Memory Leak Audit & High-Res Downsampling》: `research.md#r001`
- R002 [SUBAGENT:research] 《CLI Interactive Terminal TUI Mode Architecture (`ttzip-cli explore`)》: `research.md#r002`
- R003 [SUBAGENT:research] 《Core LZ4/ZSTD SIMD Decompression & Interleaved PMULL CRC》: `research.md#r003`

---

## Phase 1: Design & Contract Artifacts

- **Data Model**: [`data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/073-media-preview-audit-tui-engine-acceleration/data-model.md)
- **Contracts**:
  - `contracts/tui_session_state.json` [SUBAGENT:research]
  - `contracts/media_preview_audit_report.json` [SUBAGENT:research]
- **Verification**: [`quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/073-media-preview-audit-tui-engine-acceleration/quickstart.md)

---

## Phase 2: Implementation & Target Component Changes

### 1. Media Preview & GUI Optimization Layer (`TTZipApp`)
- [NEW] `Sources/TTZipApp/Services/ImageIOThumbnailCache.swift`: High-DPI image thumbnail downsampling.
- [MODIFY] `Sources/TTZipApp/Views/Preview/VideoAudioPlayerPreviewView.swift`: AVPlayer 5-step teardown on `.onDisappear`.
- [MODIFY] `Sources/TTZipApp/Views/Preview/UnifiedAudioPlayerView.swift`: Audio HAL teardown and timer lifecycle.
- [MODIFY] `Sources/TTZipApp/Views/Explorer/MillerColumnItemRowView.swift`: Virtual item `NSFilePromiseProvider` drag support.
- [MODIFY] `Sources/TTZipApp/Views/Explorer/FinderMillerColumnsView.swift`: Spacebar Quick Look interception.

### 2. Interactive Terminal TUI Engine Layer (`TTZipCore` / `TTZipCLI`)
- [NEW] `Sources/TTZipCore/CLI/TUI/TerminalRawModeManager.swift`: POSIX `termios` raw mode manager with RAII exit cleanup.
- [NEW] `Sources/TTZipCore/CLI/TUI/TUIKeyParser.swift`: Multi-byte ANSI escape & Vim key parser.
- [NEW] `Sources/TTZipCore/CLI/TUI/InteractiveTUIExplorer.swift`: Double-buffered alternate screen TUI explorer.
- [MODIFY] `Sources/TTZipCore/CLI/CLICommandSpec.swift`: Add `explore` subcommand specification.
- [MODIFY] `Sources/TTZipCLI/CLICommandRouter.swift`: Wire `explore` command dispatch.

### 3. Test Suites (`Tests/TTZipTests`)
- [NEW] `Tests/TTZipTests/MediaPreviewAuditTests.swift`: Validates image downsampling, AVPlayer teardown, and Finder drag promise.
- [NEW] `Tests/TTZipTests/InteractiveTUITests.swift`: Validates TUI key navigation, state machine, and terminal restoration.
