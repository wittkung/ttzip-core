# Technical Research: 073-media-preview-audit-tui-engine-acceleration

## Research Overview
This document consolidates findings and architectural decisions from deep research into Desktop Media Preview lifecycle management, CLI Interactive TUI Mode, and Core SIMD Decompression Acceleration.

---

### R001: Desktop Media Preview Memory Leak Audit, AVPlayer Resource Cleanup, and 50MP+ Image Downsampling

- **Decision**:
  1. Implement an asynchronous downsampling pipeline using CoreGraphics / ImageIO (`CGImageSourceCreateThumbnailAtIndex`):
     - Initialize `CGImageSource` with `kCGImageSourceShouldCache: false`.
     - Decode directly to destination thumbnail size (`kCGImageSourceThumbnailMaxPixelSize: 2048`, `kCGImageSourceCreateThumbnailWithTransform: true`).
     - Downsample 50MP+ images (8192×6144, ~200MB uncompressed bitmap) to 2048px (~12.5MB bitmap), achieving a 93.7% RAM reduction.
  2. Implement a 5-step AVPlayer/CoreAudio teardown protocol in `SharedVideoPlayerStore`, `UnifiedVideoPlayerView`, and `UnifiedAudioPlayerView`:
     - Invalidate hover/hide timers.
     - Remove time observers (`removeTimeObserver`).
     - Call `player?.pause()` and `player?.rate = 0`.
     - Call `player?.replaceCurrentItem(with: nil)` to immediately release CoreAudio HAL units and hardware video decoder tracks.
     - Set `player = nil` on `.onDisappear`.
  3. Optimize `AudioWaveformVisualizerView` by pausing the timer when not playing, and batch regex token application in `CodeSyntaxPreviewView`.
  4. Implement virtual archive item Finder drag-and-drop using `NSFilePromiseProvider` with background async extraction via `TTZipEngineFacade.shared.extractSingleEntry`.
  5. Intercept Spacebar (`keyCode == 49`) in `FinderMillerColumnsView` and `ArchiveExplorerView` for instant asynchronous `QLPreviewPanel` invocation.

- **Rationale**:
  - ImageIO decodes directly into the destination thumbnail buffer without allocating intermediate full-resolution bitmaps.
  - `replaceCurrentItem(with: nil)` immediately releases CoreMedia decoders and prevents background battery drain.
  - `NSFilePromiseProvider` follows standard macOS Finder file promise drag conventions without blocking the UI thread during drag start.

- **Alternatives Considered**:
  - `NSBitmapImageRep.draw(in:)`: Rejected because it still allocates and decompresses the full 50MP bitmap into memory before drawing, triggering severe RAM spikes.
  - Synchronous extraction on `.onDrag`: Rejected because extracting gigabyte-sized files on drag start freezes the cursor and blocks the main thread.

- **Source**:
  - `Sources/TTZipApp/Services/MediaPreviewFactory.swift:55-57, 121-125`
  - `Sources/TTZipApp/Views/Preview/InteractiveZoomImageView.swift:6-30`
  - `Sources/TTZipApp/Views/Preview/VideoAudioPlayerPreviewView.swift:20-80`
  - `Sources/TTZipApp/Views/Explorer/MillerColumnItemRowView.swift:95-99, 359-368`
  - `Sources/TTZipApp/Views/Explorer/FinderMillerColumnsView.swift:168-190`

---

### R002: CLI Interactive Terminal TUI Mode Architecture (`ttzip-cli explore`) in Swift & POSIX

- **Decision**:
  1. Implement zero-dependency native POSIX Raw Mode (`TerminalRawModeManager`) in `Sources/TTZipCore/CLI/TUI/TerminalRawModeManager.swift`:
     - Use Darwin `<termios.h>` syscalls (`tcgetattr`, `tcsetattr`, `cfmakeraw`).
     - Disable `ICANON`, `ECHO`, `IEXTEN`, `ISIG`, `OPOST`.
     - Configure `c_cc[VMIN] = 0`, `c_cc[VTIME] = 1` for 100ms non-blocking polling.
     - Enforce RAII restoration on exit and register `signal(SIGINT)`, `signal(SIGTERM)` handlers.
  2. Implement double-buffered single-flush rendering in `InteractiveTUIExplorer.swift`:
     - Switch to Alternate Screen Buffer (`\u{001B}[?1049h`) and hide cursor (`\u{001B}[?25l`).
     - Assemble full frame in memory starting with `\u{001B}[H` and submit via a single `fputs(buffer, stdout); fflush(stdout);` to eliminate screen tearing.
  3. Parse multi-byte ANSI escape sequences (`↑`/`↓`/`←`/`→`, `PageUp`/`PageDown`, `Home`/`End`) and Vim bindings (`h`/`j`/`k`/`l`, `Enter`, `Space`, `p`, `e`, `q`).
  4. Four-segment terminal UI layout: Breadcrumb Header, Virtualized Directory Tree Viewport, Status Bar, and Popup Peek Modal (`p`).
  5. Connect `InteractiveTUIExplorer` with `ArchiveComponentTreeBuilder` for instant tree structure and `TTZipEngineFacade.shared.extractSingleEntry` for selective extraction (`e`).

- **Rationale**:
  - Zero external C dependencies (no ncurses / termbox) ensures 100% pure Swift + POSIX portability and eliminates dynamic library linking issues.
  - Alternate screen buffer keeps user's terminal scrollback history completely intact upon exiting.
  - Virtualized viewport makes cursor navigation $O(1)$ even on archives with 100,000+ entries.

- **Alternatives Considered**:
  - `ncurses` / `libcurses` C bindings: Rejected due to global state, thread safety issues, and portability differences across macOS and Linux Docker environments.
  - Full tree re-traversal on every keystroke: Rejected due to frame drops on large archives; viewport virtualization is vastly faster.

- **Source**:
  - `Sources/TTZipCore/CLI/TerminalRenderEngine.swift:20-65`
  - `Sources/TTZipCore/CLI/TerminalPagerEngine.swift:16-60`
  - `Sources/TTZipCore/CLI/ArchiveVisualTreeRenderer.swift:10-175`
  - `Sources/TTZipCore/ArchiveComponentProtocol.swift:1-370`
  - `Sources/TTZipCore/Facades/TTZipEngineFacade.swift:169-181, 765-822`

---

### R003: Core LZ4/ZSTD SIMD Decompression Acceleration and Interleaved PMULL CRC

- **Decision**:
  1. Implement `ttzip_tar_lz4_direct.c` and `ttzip_lz4_stream_direct.c` using `lz4frame.h` (`LZ4F_createDecompressionContext` / `LZ4F_decompress`) with 64KB page-aligned buffers (`posix_memalign`, input 8MB, output 16MB).
  2. Implement adaptive Direct-IO disk streaming: for extracted files $\ge 16\text{MB}$, perform APFS `F_PREALLOCATE` and enable `fcntl(fd, F_NOCACHE, 1)` to eliminate OS Page Cache pollution and double-copying.
  3. Interleave ARM64 PMULL hardware CRC calculation (`ttzip_crc64_pmull` / `__crc32cd`) directly inside the block decompression loop while the buffer is in L1/L2 cache, eliminating post-extraction verification latency.

- **Rationale**:
  - Bypasses generic libarchive per-entry heap allocation overhead for LZ4, lifting throughput past 4,000 MB/s.
  - Interleaving PMULL calculation during decode takes advantage of cache-hot memory and out-of-order execution on Apple Silicon, reducing verification time to ~0ms.

- **Alternatives Considered**:
  - Full file `mmap` for LZ4: Rejected because multi-gigabyte files would cause address space exhaustion and heavy page fault overhead.
  - Post-extraction disk re-read for CRC: Rejected because it causes severe disk I/O contention and adds 30-60% latency.

- **Source**:
  - `Sources/CTTZipBridge/include/lz4frame.h:368-500`
  - `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c:508-545`
  - `Sources/CTTZipBridge/ttzip_crc64.c:48-125`
  - `Sources/CTTZipBridge/include/ttzip_crc64.h:1-35`
