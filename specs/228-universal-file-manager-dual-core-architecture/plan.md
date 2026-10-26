# Implementation Plan: Universal File Manager & Dual-Core Architecture

**Branch**: `228-universal-file-manager-dual-core-architecture` | **Date**: 2026-09-26 | **Spec**: [spec.md](./spec.md)  
**Pipeline Mode**: [Full SDD]

---

## 1. Summary

Reconstruct TTZip from a single-focus archive utility into an industrial-grade universal file manager and multi-platform workspace. Establish a three-layer dual-core architecture:
1. Pure archive microkernel (`products/ttzip/core`);
2. Universal media & rich document kit (`infra/ttmedia`, superset of `infra/ttmpv`);
3. Cross-platform application core (`products/ttzip/app-core`, 100% Rust + UniFFI);
4. 120Hz zero-drop fluid presentation client (`products/ttzip/apple`).

Purge 45,000+ lines of synthetic AI slop, fake test frameworks, and dead components as Step 1.

---

## 2. Technical Context

- **Languages/Toolchains**: Rust 1.84+ (MSRV), Swift 6 (Strict Concurrency), C/C++ (C11/C++20 via Bazel `cc_library`).
- **Primary Dependencies**:
  - `infra/ttmpv` (libmpv + Metal rendering)
  - `libusb-1.0` (cross-platform USB/MTP)
  - `ed25519-dalek` / `sha2` (PKI security verification)
  - `tokio` (async I/O, device scheduling)
  - `uniffi` 0.28+ (safe cross-language FFI bindings)
- **Target Platforms**: macOS 13.0+ (Ventura, Sonoma, Sequoia; Apple Silicon & Intel x86_64); cross-platform Rust cores portable to Windows 11 & Linux.
- **Performance Budgets**:
  - Main thread I/O wait: **0.0ms**.
  - 100k-item archive VFS projection memory: **<32MB**.
  - ProMotion display scroll budget: **120 FPS solid**.
  - Audio playback CPU overhead: **<2.0%**.

---

## 3. Constitution Check & Architecture Gates

| Gate / Invariant | Status | Verification Strategy |
| :--- | :--- | :--- |
| **Google Piper Licensing** | PASS | Proprietary headers enforced; Apache-2.0 mock test code purged. |
| **Root Neutrality** | PASS | Root contains no single-language manifest; Bazel 8 rules manage targets. |
| **Single-File LOC Gate** | PASS | Strict $\le 800$ LOC limit enforced via `scripts/lint_loc_gate.sh`. |
| **Zero-Warning Hard Gate** | PASS | `-warnings-as-errors` and `cargo clippy -- -D warnings` enforced. |
| **Decoupled Extraction** | PASS | `infra/ttmedia` sub-engines maintain 0 circular references. |

---

## 4. Project Structure & Code Layout

```
metastudyline/source/
├── products/
│   └── ttzip/
│       ├── core/                      # Pure Archive Microkernel
│       │   ├── rust/crates/
│       │   │   ├── ttzip-archive/     # VFS, TAR, ZIP, 7Z, WAL mutation
│       │   │   ├── ttzip-codecs/      # LZMA2 (RingDict), Deflate, Zstd
│       │   │   └── ttzip-types/       # SSOT ArchiveFormat
│       │   └── Sources/TTZipCore/     # Slim Swift facade & Reader/Writer
│       ├── app-core/                  # NEW: Cross-Platform Application Core
│       │   ├── Cargo.toml
│       │   └── src/
│       │       ├── device/            # Real USB MTP & ADB protocols
│       │       ├── transfer/          # Sliding-window transfer coordinator
│       │       ├── plugin/            # 3-tier PKI certificate validator
│       │       └── uniffi_api/        # Swift / C# / C++ bindings
│       └── apple/                     # Thin Presentation Client (SwiftUI / AppKit)
│           └── Sources/
│               ├── TTZipApp/          # UI views, windows, sheets
│               ├── TTZipUI/           # Reusable design tokens & components
│               └── TTZipPreviewKit/   # Preview connectors
└── infra/
    ├── ttmedia/                       # NEW: Universal Media & Document Kit
    │   ├── BUILD.bazel
    │   ├── av/                        # Subsystem wrapping //infra/ttmpv
    │   ├── document/                  # Office (Excel/CSV/DOCX) & PDF & Ebook
    │   ├── syntax/                    # Tree-sitter AST & code tokenization
    │   └── image/                     # Tile pyramids & Metal rendering
    └── ttmpv/                         # Existing high-throughput mpv engine
```

---

## 5. Execution Phases

### Phase 1: Physical Purge of Dead Assets (-45,000+ LOC)
- Remove `core/scripts/benchmark_archive/` (all fake A/B and metric scripts).
- Remove `core/sdk/jvm/.../ttzip_engine.kt` (32,001 LOC) and fake `org.junit` mocks.
- Remove client-side dead view components (18 files including `UniversalOmnibarView` and `Fluid/`).
- Remove obsolete and broken C/C++/Go SDK and example directories.

### Phase 2: Microkernel Hardening (`products/ttzip/core`)
- Consolidate duplicate TAR modules and unify `ArchiveFormat` to a single enum.
- Replace `Vec::drain` in LZMA2 dictionary with `Lzma2RingDict`.
- Purge all `catch_unwind` wrappers incompatible with `panic = "abort"`.
- Evict all non-archive media parsers from `ttzip-core`.

### Phase 3: Infrastructure Kit Scaffold (`infra/ttmedia`)
- Initialize `infra/ttmedia` with sub-module structure.
- Wire `ttmedia-av` directly to `//infra/ttmpv`.
- Port clean document, syntax, and image engines into `infra/ttmedia`.

### Phase 4: Application Core Scaffold (`products/ttzip/app-core`)
- Initialize `products/ttzip/app-core` Rust crate.
- Implement real USB MTP and ADB communication pipelines via `libusb`.
- Implement 3-tier Ed25519 PKI certificate chain validator and manifest capability guard.
- Generate and verify UniFFI Swift bindings.

### Phase 5: Client Performance & Binding Realignment (`products/ttzip/apple`)
- Wire UI views to `app-core` and `ttmedia`.
- Cache `displayName` in `DiskItemInfo` (0.0ms main-thread I/O).
- Offscreen-rasterize waveforms and isolate 60Hz timeline cursor progression.
- Run full gate audits: Bazel test, Clippy, and LOC gate.
