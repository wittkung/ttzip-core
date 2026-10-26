# Feature Specification: Universal File Manager & Dual-Core Architecture

**Feature Branch**: `228-universal-file-manager-dual-core-architecture`  
**Created**: 2026-09-26  
**Status**: Draft  
**Pipeline Mode**: [Full SDD]  
**Input**: "全面思考和完善方案，并全面推进（全面采用新架构，并完整，无遗留的解决之前发现的这些问题），要全面用最专业，优雅，体验最好的方式来，所有的问题全都好好记录到草稿本中"

---

## 1. Executive Summary & Architectural Invariants

TTZip evolves from a specialized archive utility into an industrial-grade, cross-platform universal file manager and workspace. The architecture establishes a strict **Three-Layer Dual-Core Topology**:

1. **Archive Microkernel (`products/ttzip/core`)**: Headless, pure-Rust engine dedicated exclusively to high-throughput compression, decompression, virtual filesystem (VFS) projections, and WAL-based in-place mutations. All media and document parsers are evicted.
2. **Universal Media & Rich Document Kit (`infra/ttmedia`)**: Monorepo-level shared infrastructure kit acting as a strict superset of `infra/ttmpv`. Provides zero-copy inspection, hardware-accelerated playback, vector rasterization, syntax highlighting, and delta-modification generation across Office, PDF, Ebook, Audio, Video, and Source Code formats. Designed with decoupled sub-engines ready for zero-friction extraction into standalone repositories.
3. **Cross-Platform Application Core (`products/ttzip/app-core`)**: Shared non-compression application microkernel (100% Rust + UniFFI). Coordinates real USB/MTP and wireless ADB device communication, prioritized transfer pipelines with sliding-window telemetry, multi-tab workspace sessions, and an industrial 3-tier PKI plugin runtime.
4. **Thin Platform UI (`products/ttzip/apple`)**: Pure SwiftUI + AppKit presentation tier targeting 120Hz ProMotion zero-drop frame budgets. Strictly disallows disk I/O on the main thread and enforces unidirectional data flow from underlying cores.

---

## 2. User Scenarios & Testing *(mandatory)*

### User Story 1 - Pure Archive Operations with Zero-Copy VFS (Priority: P1)
As a user inspecting a multi-gigabyte compressed archive (ZIP, 7z, TAR, Zstd), I want instantaneous directory navigation, search, and extraction without memory spikes or main-thread beachballs, so that archive management feels like a local SSD folder.

- **Why this priority**: Core identity of TTZip. Eliminates the 10-layer facade round-trip, dual POSIX mmap duplication, 64MB dictionary memmove stops, and multi-format match glue uncovered during audit.
- **Independent Test**: Open a 10GB 7z archive with 100,000 files; measure directory tree construction latency (<15ms), peak resident memory (<32MB), and verify selective extraction does not uncompress preceding solid streams in full.
- **Acceptance Scenarios**:
  1. **Given** an archive file on disk, **When** the user opens it in TTZip, **Then** Rust microkernel establishes a single memory mapping, projects a flat immutable `ArchiveTreeNode` hierarchy, and returns to UI in under 20ms without allocating heap buffers proportional to total archive size.
  2. **Given** a corrupted archive or missing entry, **When** extraction is attempted, **Then** the engine returns a deterministic `TTZipError::FileNotFound` or `ChecksumMismatch` without cascading through fallback parsers or returning fake waveforms.

---

### User Story 2 - Instant Rich Preview & In-Place Editing via `infra/ttmedia` (Priority: P1)
As a knowledge worker browsing heterogeneous files, I want to preview audio waveforms, Office spreadsheets, PDF bookmarks, EPUB chapters, and syntax-highlighted source code immediately within TTZip, and directly edit text/data in-place, without opening third-party heavyweight apps.

- **Why this priority**: Transforms TTZip from a passive archiver into an active universal file workspace.
- **Independent Test**: Select an Excel file and a 50MB audio file inside a nested archive; verify spreadsheet renders cells in read-only sheet view and audio plays with accurate hardware-decoded waveform, consuming <2% main thread CPU.
- **Acceptance Scenarios**:
  1. **Given** a media or document file inside or outside an archive, **When** preview is invoked, **Then** `infra/ttmedia` routes the request to the matching sub-engine (`ttmedia-av` backed by `ttmpv`, `ttmedia-document`, `ttmedia-syntax`, or `ttmedia-image`) and produces an offscreen-rasterized canvas.
  2. **Given** an open text/code file, **When** the user modifies contents and saves, **Then** `ttmedia` emits an atomic byte delta, and `products/ttzip/core` records a WAL mutation slice to commit changes without full-archive rewrite.

---

### User Story 3 - Cross-Platform Device Management & Physical Transfer Pipeline (Priority: P1)
As a power user connecting an Android device or external MTP camera via USB or Wi-Fi, I want to explore mobile storage, batch-transfer files with accurate transfer speed and pause/resume capabilities, without fake delays or artificial mock data.

- **Why this priority**: Fulfills the desktop file manager mandate. Replaces simulated 250ms sleeps, fake SPAKE2 handshakes, and hardcoded "75 MB/s" strings with industrial-grade MTP/ADB protocol execution in `products/ttzip/app-core`.
- **Independent Test**: Connect an Android device over USB; inspect internal storage tree, initiate a 5GB transfer queue, disconnect mid-transfer, reconnect, and verify resumption from the last verified block.
- **Acceptance Scenarios**:
  1. **Given** an Android device attached via USB, **When** `app-core` detects interface arrival via `libusb` / `IOKit`, **Then** it initializes standard MTP session handshakes, queries storage IDs, and streams directory hierarchies into the workspace state machine.
  2. **Given** an ongoing file transfer, **When** progress updates occur, **Then** throughput is computed via sliding-window byte accumulation over 500ms intervals, dispatching telemetry events to the UI at 2Hz.

---

### User Story 4 - Industrial 3-Tier PKI Plugin Runtime (Priority: P2)
As a developer or enterprise user, I want to install third-party format and tool plugins with verified cryptographic integrity, explicit permission manifests, and isolated execution, ensuring malicious plugins cannot compromise system files or steal data.

- **Why this priority**: Replaces the placeholder Base64 public key with a production PKI root-chain validator.
- **Independent Test**: Attempt loading: (a) a valid plugin signed by an authorized developer cert; (b) a plugin with an expired intermediate cert; (c) an untrusted self-signed plugin. Verify (a) loads and runs with declared capabilities, while (b) and (c) are explicitly blocked with clear error diagnostic codes.
- **Acceptance Scenarios**:
  1. **Given** a `.ttplugin` bundle, **When** installation is requested, **Then** `products/ttzip/app-core::plugin::pki` validates the certificate path: Root CA -> Developer Certificate -> Package Digest Signature using Ed25519/ECDSA, verifying all signatures against non-revoked CRL digests.
  2. **Given** an active plugin without `fs.write` permission, **When** it invokes an extraction or write call, **Then** host capability guards immediately reject the call with `SecurityError::CapabilityViolation`.

---

### User Story 5 - macOS Client 120Hz Zero-Drop Fluid Performance (Priority: P2)
As a macOS user navigating thousands of items with keyboard or trackpad, I want silky-smooth scrolling, instantaneous column switches, and responsive search without UI stutter, beachball spins, or battery drain from phantom animations.

- **Why this priority**: Fixes client-side regressions: Equatable calling `FileManager.displayName`, 60Hz audio time root-view invalidations, and 18 abandoned view components.
- **Independent Test**: Launch Instruments (Time Profiler + Core Animation); scroll through a folder with 50,000 items in Miller Column view and play audio simultaneously; verify 0ms main thread I/O wait, sustained 120 FPS on ProMotion displays, and overall app CPU idle <0.5%.
- **Acceptance Scenarios**:
  1. **Given** thousands of files in Miller Columns, **When** scrolling or changing selection, **Then** row diffs evaluate pure memory properties (`path`, cached `displayName`) without triggering POSIX filesystem calls.
  2. **Given** active audio playback, **When** cursor moves across timeline, **Then** time mutations are isolated to leaf cursor subviews; background waveforms remain statically rasterized without CPU path reconstruction.

---

### User Story 6 - Complete Elimination of AI Slop, Fake Tests, and Dead Artifacts (Priority: P3)
As an open-source contributor and monorepo maintainer, I want all build targets, benchmark suites, and SDK bindings to be strictly hermetic, mathematically honest, and fully verified by CI, with zero dead or duplicated files.

- **Why this priority**: Purges 45,000+ LOC of synthetic noise (including the 32,001-line Kotlin monolith, fake `org.junit` mocks, and biased A/B scripts) to establish an unassailable codebase.
- **Independent Test**: Execute `bazel test -c opt //...` and `cargo test --workspace`; confirm zero compilation warnings, zero fake benchmark executions, and 100% LOC gate compliance.
- **Acceptance Scenarios**:
  1. **Given** the monorepo root, **When** running directory audits, **Then** no file exceeds 800 LOC, no duplicated TAR/format definitions exist, and all synthetic benchmark scripts in `core/scripts/benchmark_archive/` are completely removed.

---

## 3. Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Microkernel (`core`) MUST implement unified `ArchiveFormat` enumeration as the Single Source of Truth, eliminating `TTZipArchiveFormat` and `DetectedFormat`.
- **FR-002**: Microkernel MUST provide true zero-copy file mapping via read-only slice borrowing, deprecating secondary POSIX `open/mmap` in Swift bridge.
- **FR-003**: Microkernel MUST implement LZMA2 sliding dictionary using fixed-capacity circular ring buffers (`pos & mask`), banning `Vec::drain` in hot decoding paths.
- **FR-004**: Microkernel MUST eliminate all `catch_unwind` wrappers in release profiles where `panic = "abort"` is set, strictly returning typed `Result<T, TTZipError>`.
- **FR-005**: Infrastructure (`infra/ttmedia`) MUST be established as an independent kit holding sub-modules: `ttmedia-av`, `ttmedia-document`, `ttmedia-syntax`, and `ttmedia-image`.
- **FR-006**: `infra/ttmedia` MUST consume `//infra/ttmpv` as its audio/video playback and Metal offscreen rendering subsystem without duplicating libmpv bindings.
- **FR-007**: `infra/ttmedia` MUST expose a uniform `MediaInspector` protocol delivering format identification, metadata maps, and vector/raster rendering pipelines.
- **FR-008**: All sub-modules in `infra/ttmedia` MUST maintain zero circular dependencies and be isolated such that any sub-module can be extracted into a standalone package.
- **FR-009**: Application Core (`app-core`) MUST be established under `products/ttzip/app-core` as a headless Rust crate exposing UniFFI bindings.
- **FR-010**: `app-core` MUST implement genuine USB MTP 1.1 protocol communication via `libusb` / `IOKit`, supporting object enumeration, metadata queries, and streaming reads/writes.
- **FR-011**: `app-core` MUST implement standard ADB transport communication (device pairing, authorization key handshake, and sync protocol stream).
- **FR-012**: `app-core` MUST provide a prioritized transfer manager with chunked verification, sliding-window speed calculation, and persistent checkpoint WAL.
- **FR-013**: `app-core` MUST implement 3-tier PKI certificate chain validation (Root CA -> Developer Cert -> Plugin Package Digest) using Ed25519 cryptography.
- **FR-014**: `app-core` MUST enforce manifest-declared capabilities (`Capabilities`), halting unauthorized filesystem, network, or process execution attempts.
- **FR-015**: Platform UI (`products/ttzip/apple`) MUST separate into pure presentation views, interacting with `app-core` and `core` via asynchronous streams.
- **FR-016**: Client row models (`DiskItemInfo`) MUST precompute and store `displayName` during instantiation, prohibiting POSIX I/O inside `Equatable::==`.
- **FR-017**: Audio player views MUST isolate 60Hz playback progression to leaf timeline cursors; waveform paths MUST be rasterized offscreen or cached via `drawingGroup()`.
- **FR-018**: Client MUST physically purge all 18 identified abandoned view components (including `UniversalOmnibarView`, `LiquidGlassOmnibar`, `FolderCompositionPieChartView`, and `Fluid/` renderer).
- **FR-019**: Build pipeline MUST physically remove `core/scripts/benchmark_archive/`, `core/sdk/jvm/.../ttzip_engine.kt` (32,001 LOC), fake `org.junit` mocks, and unmaintained SDK templates.
- **FR-020**: C-ABI bridge (`CTTZipBridge.h`) MUST replace the 9KB `TTZipPathSanitizationResult` stack structure with standard pointer-length buffers or UniFFI safe strings.

---

## 4. Key Entities

- **`ArchiveEntry`**: Canonical immutable descriptor of an archived item (path, compressed size, uncompressed size, CRC32/Blake3, attributes, encryption method).
- **`MediaInspection`**: Standard metadata and render pipeline descriptor returned by `infra/ttmedia` (MIME type, thumbnail provider, dimensions, duration, text/sheet stream).
- **`DeviceDescriptor`**: Abstract physical or wireless endpoint managed by `app-core` (UUID, connection transport, product name, battery state, storage partitions).
- **`TransferJob`**: Atomically scheduled file movement task (Job ID, source URI, destination URI, total bytes, transferred bytes, checksum, pause/resume checkpoint).
- **`PluginManifest`**: Cryptographically signed descriptor of a plugin bundle (Plugin ID, semantic version, developer ID, certificate chain, requested capability scopes).

---

## 5. Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Microkernel peak resident memory during 100,000-file archive traversal drops from >250MB to <32MB.
- **SC-002**: Main thread disk I/O wait during Miller Column fast navigation drops to 0.0ms.
- **SC-003**: UI scrolling frame rate maintains solid 120 FPS on Apple Silicon ProMotion displays across 50,000 items.
- **SC-004**: Audio playback and waveform visualization reduces client CPU utilization from 30%~60% to <2.0%.
- **SC-005**: MTP / ADB file transfer metrics reflect 100% measured physical bytes (sliding window 500ms), eliminating all hardcoded fallback values.
- **SC-006**: Total codebase size is reduced by >45,000 lines of dead, duplicated, or synthetic AI slop files while passing all unit and integration tests.
- **SC-007**: Every single Rust and Swift file complies with the Monorepo Single-File LOC Gate ($\le 800$ LOC, target mean $\le 350$ LOC).
- **SC-008**: Full hermetic build passes `bazel test -c opt //...` with 0 errors and 0 compiler warnings.

---

## 6. Assumptions & Non-Goals

### Assumptions
- macOS 13.0+ (Ventura, Sonoma, Sequoia) is the primary client deployment target for `products/ttzip/apple`.
- `infra/ttmpv` provides reliable Metal / libmpv C-bindings and is imported hermetically via Bazel and SPM.
- Rust toolchain 1.84+ and Swift 6 with strict concurrency checking enabled.

### Non-Goals
- We will NOT implement a custom software-based Office rendering engine from scratch; `ttmedia-document` provides structured cell extraction and clean Markdown/HTML views.
- We will NOT support legacy Android 4.x ADB protocols; only modern ADB v4+ and USB MTP 1.1 are supported.

---

## 7. Clarified Architectural Decisions

- **Decision 1 (Topology)**: `products/ttzip/app-core` is established as an independent Rust crate physically decoupled from `products/ttzip/core`. Each exports independent UniFFI bindings ensuring strict architectural orthogonality.
- **Decision 2 (Execution Strategy)**: Immediate physical purge as Step 1. All 45,000+ lines of synthetic benchmark scripts, 32,001-line Kotlin monolith, fake JUnit code, and 18 dead UI components are removed prior to new scaffold generation.
- **Decision 3 (In-Place Edit Scope)**: Progressive in-place editing. Phase 1 delivers zero-delay editing and WAL mutation write-back for text, source code, Markdown, and CSV/TSV data; Office documents, PDFs, and audio/video files focus on high-performance offscreen read-only preview and streaming.

