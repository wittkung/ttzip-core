# Implementation Tasks: Universal File Manager & Dual-Core Architecture

**Branch**: `228-universal-file-manager-dual-core-architecture`  
**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)  
**Status**: Ready for Implementation  

---

## Dependencies & Phase Completion Order

```
[Phase 1: Setup & Dead Asset Purge]
       │
       ├──► [Phase 2: Archive Microkernel Hardening (US1)]
       │           │
       │           └──► [Phase 3: Media & Document Kit (US2)]
       │
       └──► [Phase 4: Cross-Platform Application Core (US3)]
                   │
                   └──► [Phase 5: Industrial 3-Tier PKI (US4)]
                               │
                               ▼
       [Phase 6: Client 120Hz Zero-Drop Fluid Performance (US5)]
                               │
                               ▼
       [Phase 7: Full Monorepo Gate Audits & Quickstart Verification]
```

---

## Phase 1: Setup & Immediate Dead Asset Purge

*Goal: Purge 45,000+ lines of synthetic AI slop, fake test frameworks, and dead components to establish a pristine codebase.*

- [x] [TASK-001] [P1] [Setup] Physically delete synthetic benchmark scripts directory `products/ttzip/core/scripts/benchmark_archive/`
- [x] [TASK-002] [P1] [Setup] Physically delete 32,001-line Kotlin monolith `products/ttzip/core/sdk/jvm/src/main/kotlin/com/ttzip/uniffi/ttzip_engine/ttzip_engine.kt`
- [x] [TASK-003] [P1] [Setup] Physically delete fake JUnit test package `products/ttzip/core/sdk/jvm/src/test/java/org/junit/`
- [x] [TASK-004] [P1] [Setup] Physically delete unmaintained and broken SDK/example directories `products/ttzip/core/sdk/` (dart, dotnet, go, node) and `products/ttzip/core/examples/` (c, cpp, go, dotnet, jvm)
- [x] [TASK-005] [P1] [Setup] Physically delete orphaned scripts `products/ttzip/core/scripts/gc_target.py` and `products/ttzip/core/scripts/upstream_report_gen.py`
- [x] [TASK-006] [P1] [Setup] Physically delete 18 abandoned client view components and pseudophysics fluid engine (`products/ttzip/apple/Sources/TTZipUI/Views/Fluid/`, `UniversalOmnibarView*.swift`, `LiquidGlassOmnibar.swift`, `SpotlightSearchCapsuleView.swift`, `SegmentedBreadcrumbCapsuleView.swift`, `AndroidMillerView.swift`, `FolderCompositionPieChartView.swift`, `CompressionProInspectorPanel.swift`, etc.)

---

## Phase 2: User Story 1 - Archive Microkernel Hardening (`products/ttzip/core`)

*Goal: Establish pure-Rust archive microkernel with zero-copy VFS, consolidated format types, and constant-factor memory efficiency.*

- [x] [TASK-007] [P1] [US1] Consolidate format types into a single `ArchiveFormat` enum in `products/ttzip/core/rust/crates/ttzip-types/src/archive_format.rs`, deleting `TTZipArchiveFormat` and `DetectedFormat`
- [x] [TASK-008] [P1] [US1] Merge duplicate TAR implementations into `products/ttzip/core/rust/crates/ttzip-archive/src/tar/`, deleting `crates/ttzip-archive/src/archive/tar/` and renaming raw block to `RawTarHeader`
- [x] [TASK-009] [P1] [US1] Replace `Vec::drain` in LZMA2 sliding dictionary with `Lzma2RingDict` in `products/ttzip/core/rust/crates/ttzip-codecs/src/lzma2/decoder/dict.rs`
- [x] [TASK-010] [P1] [US1] Fix WAL mutation slice truncation defect for non-original pieces in `products/ttzip/core/rust/crates/ttzip-archive/src/archive/wal_mutation.rs`
- [x] [TASK-011] [P1] [US1] Purge all `catch_unwind` blocks across microkernel crates that conflict with `panic = "abort"` in `Cargo.toml`
- [x] [TASK-012] [P1] [US1] Evict 11 zero-size strategy structs and delete `products/ttzip/core/rust/crates/ttzip-archive/src/archive/zero_vtable_dispatch.rs`
- [x] [TASK-013] [P1] [US1] Evict duplicate dead DSL module `products/ttzip/core/rust/crates/ttzip-archive/src/fs/filter_dsl/`
- [x] [TASK-014] [P1] [US1] Eliminate secondary POSIX mmap in `products/ttzip/core/Sources/TTZipCore/UniFfiMmapReader+ZeroCopy.swift`, sharing Rust mapped buffer safely
- [x] [TASK-015] [P1] [US1] Flatten 10-layer compression facade round-trip: decouple `TTZipEngineFacade` and `CompressCommand` to directly invoke `ArchiveWriter`

---

## Phase 3: User Story 2 - Universal Media & Rich Document Kit (`infra/ttmedia`)

*Goal: Establish `infra/ttmedia` as a Monorepo-level kit subsuming `infra/ttmpv` for zero-copy inspection, rendering, and in-place editing.*

- [x] [TASK-016] [P1] [US2] Scaffold `infra/ttmedia` kit directory and configure Bazel targets in `infra/ttmedia/BUILD.bazel`
- [x] [TASK-017] [P1] [US2] Implement `infra/ttmedia/av/` wiring `//infra/ttmpv` as audio/video playback subsystem with authentic PCM waveform rasterizer
- [x] [TASK-018] [P1] [US2] Migrate and slim down document parsers (XLSX, CSV, DOCX, PDF, EPUB) from `ttzip-core` into `infra/ttmedia/document/`
- [x] [TASK-019] [P1] [US2] Migrate Tree-sitter code syntax tokenizer and AST generator from `ttzip-core` into `infra/ttmedia/syntax/`
- [x] [TASK-020] [P1] [US2] Migrate gigapixel tile pyramid downsampler into `infra/ttmedia/image/` with Metal shaders
- [x] [TASK-021] [P1] [US2] Implement `InPlaceDelta` generation protocol in `infra/ttmedia` for text, Markdown, and CSV, binding to microkernel WAL mutations

---

## Phase 4: User Story 3 - Cross-Platform Application Core (`products/ttzip/app-core`)

*Goal: Create headless Rust application microkernel managing real USB MTP, wireless ADB, and prioritized transfer pipelines.*

- [x] [TASK-022] [P1] [US3] Scaffold `products/ttzip/app-core/` Rust crate with `Cargo.toml` and UniFFI configuration
- [x] [TASK-023] [P1] [US3] Implement genuine USB MTP 1.1 protocol state machine over `libusb` in `products/ttzip/app-core/src/device/mtp/`
- [x] [TASK-024] [P1] [US3] Implement standard ADB transport and authorization handshake in `products/ttzip/app-core/src/device/adb/`
- [x] [TASK-025] [P1] [US3] Implement prioritized transfer coordinator with 500ms sliding-window throughput calculator in `products/ttzip/app-core/src/transfer/`
- [x] [TASK-026] [P1] [US3] Implement multi-tab workspace session manager and unified JSON favorites in `products/ttzip/app-core/src/workspace/`
- [x] [TASK-027] [P1] [US3] Generate and verify UniFFI Swift bindings for `app-core` under `products/ttzip/app-core/src/uniffi_api/`

---

## Phase 5: User Story 4 - Industrial 3-Tier PKI Plugin Runtime

*Goal: Implement authentic Ed25519 root-chain certificate validation and capability sandbox enforcement.*

- [x] [TASK-028] [P2] [US4] Implement 3-tier certificate validator (Root CA -> Dev Cert -> Package Digest) in `products/ttzip/app-core/src/plugin/pki.rs`
- [x] [TASK-029] [P2] [US4] Implement manifest parser, JSON Schema validation, and capability enforcement guards in `products/ttzip/app-core/src/plugin/manifest.rs`
- [x] [TASK-030] [P2] [US4] Fix selective extraction bug in `products/ttzip/apple/Sources/TTZipApp/Services/TTZipHostContextImpl.swift` to honor `entries` filter parameter

---

## Phase 6: User Story 5 - macOS Client 120Hz Zero-Drop Fluid Performance

*Goal: Eliminate all main-thread I/O bottlenecks and high-frequency invalidations across SwiftUI/AppKit views.*

- [x] [TASK-031] [P2] [US5] Convert `displayName` in `DiskItemInfo.swift` to a stored property, eradicating POSIX filesystem I/O inside `MillerColumnItemRowView.==`
- [x] [TASK-032] [P2] [US5] Move `MediaPreviewFactory.detectTypeAsync` off `@MainActor` onto detached cooperative background actor
- [x] [TASK-033] [P2] [US5] Isolate 60Hz playback progression in `UnifiedAudioPlayerView.swift` to leaf cursor subviews, rasterizing waveforms offscreen
- [x] [TASK-034] [P2] [US5] Refactor `OmnibarTextField.swift` to use bidirectional binding, removing asynchronous main-actor focus stealing
- [x] [TASK-035] [P2] [US5] Throttle `DockProgressManager.swift` IPC to 2Hz, reusing `DockProgressTileView` instances
- [x] [TASK-036] [P2] [US5] Wire client views to `products/ttzip/app-core` UniFFI bindings and `infra/ttmedia`

---

## Phase 7: Polish & Full Monorepo Gate Audits

*Goal: Guarantee zero compiler warnings, 100% LOC gate compliance, and end-to-end integration verification.*

- [x] [TASK-037] [P3] [Audit] Enforce Monorepo Single-File LOC Gate ($\le 800$ LOC) via `products/ttzip/core/scripts/lint_loc_gate.sh`
- [x] [TASK-038] [P3] [Audit] Verify Rust microkernel and app-core compile cleanly under `cargo clippy --workspace --all-targets -- -D warnings`
- [x] [TASK-039] [P3] [Audit] Verify Swift packages build cleanly under `swift build` and `swift test`
- [x] [TASK-040] [P3] [Audit] Execute full Monorepo Bazel 8 build and test: `bazel test -c opt //products/ttzip/... //infra/ttmedia/...`
- [x] [TASK-041] [P3] [Audit] Execute quickstart scenarios in `quickstart.md`
