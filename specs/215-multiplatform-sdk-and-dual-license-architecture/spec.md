# Feature Specification: Multiplatform SDK, Dual-Licensing & Repository Topology Architecture

**Feature**: `215-multiplatform-sdk-and-dual-license-architecture`  
**Classification**: `[Full SDD]` (Defines C-ABI boundary, multi-language SDK interfaces, legal dual-licensing, GPLv3 client architecture, CLI contracts, repository topology decoupling, and zero-cloud local CI/CD)  
**Status**: `SPECIFIED` (Fully Consolidated with Upstream Governance)  

---

## 1. Executive Summary & Problem Statement

### 1.1 Context
TTZip's architecture spans two fundamentally distinct software layers:
1. **Underlying Infrastructure & SDKs (`ttzip-core`)**: Safe Rust compression engines (`ttzip-engine`), standard C11 FFI ABI (`ttzip-glue`), standalone POSIX CLI (`ttzip-cli`), and multi-language SDK bindings (Swift 6 `TTZipCore`, Python `pyo3`, Java/Kotlin, Node.js).
2. **End-User Client Applications**: The Apple ecosystem native application (`ttzip-apple` supporting macOS AppKit/SwiftUI, QuickLook, FinderSync, and upcoming iOS/iPadOS/visionOS targets), with future roadmap support for Windows (`ttzip-windows`), Android (`ttzip-android`), and Linux (`ttzip-linux`).

To ensure that external developers can freely embed the core compression engine while rigorously preventing malicious third parties from repackaging and selling the complete GUI desktop/mobile products, a clear **tiered licensing and multi-repository topology** is established:
- **Core Engine & SDKs (`ttzip-core`)**: Permissive **`BSD-3-Clause OR Apache-2.0` Dual-License** (enables universal commercial & open-source embedding).
- **Client Applications (`ttzip-apple`, etc.)**: **`GPLv3` License** + strict Trademark / Trade Dress policy (mandates 100% source disclosure for any derivative client and prohibits unauthorized app store sales).
- **Upstream Open-Source Governance**: Automated harvesting of all third-party licenses (libdeflate, Zstd, libarchive, LZ4, XZ) into `ACKNOWLEDGEMENTS.md`, `docs/THIRD_PARTY_LICENSES.md`, and `Acknowledgements.plist` with automated CI verification.

---

## 2. Global Repository Topology & Licensing Matrix

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ 1. 核心基础设施仓库: wittkung/ttzip-core [BSD-3-Clause OR Apache-2.0 双协议开源]         │
├────────────────────────────────────────────────────────────────────────────────────────┤
│  - rust/ttzip-engine: 纯算法内核 (#![forbid(unsafe_code)], crates.io)                   │
│  - rust/ttzip-glue:   C-ABI FFI 导出层 (catch_unwind 异常安全)                          │
│  - rust/ttzip-cli:    19 个子命令 + --json NDJSON 机器流 + 终端 TUI (可执行文件 ttzip)   │
│  - Sources/CTTZipBridge: 标准 C11 ttzip.h 与 ttzip_rust_glue.h                          │
│  - Sources/TTZipCore:    Swift 6 原生 SDK (SPM Package, AsyncThrowingStream)            │
│  - Vendor/*-upstream: 官方源码工作树 (Git Subtree) 供差分基准验证与上游反哺             │
└───────────────────────────────────────────┬────────────────────────────────────────────┘
                                            │ SPM / C-ABI / 各语言包引入
        ┌───────────────────┬───────────────┴───┬───────────────────┬───────────────────┐
        ▼                   ▼                   ▼                   ▼                   ▼
【ttzip-apple】       【ttzip-windows】   【ttzip-android】   【ttzip-linux】      【Web/Wasm】
- macOS (AppKit)     - Windows 10/11     - Android Phone     - Linux 桌面         - 浏览器端
- iOS / iPadOS       - WinUI 3 / WPF     - Jetpack Compose   - GTK4 / Libadwaita  - Wasm 极速解压
- visionOS (SwiftUI) (C# / Rust)         (Kotlin)            (Rust / C++)         
[GPLv3 开源]         [GPLv3 开源]        [GPLv3 开源]        [GPLv3 开源]         [GPLv3 开源]
```

---

## 3. Phased Architecture & Scope Boundaries

### MVP Scope (Feature 215)
- [x] **Phase 0: Legal Migration, Upstream Governance & SPDX Replacement**: Transition core engine to `BSD-3-Clause OR Apache-2.0`, generate `LICENSE-BSD`, `LICENSE-APACHE`, `LICENSE-GPL`, `NOTICE`, run automated license harvesting, and batch update SPDX headers.
- [x] **Phase 1: Rust Workspace 3-Crate Restructuring**: Form `ttzip-engine` (`rlib`), `ttzip-glue` (`staticlib`/`cdylib`), and `ttzip-cli` (`bin` named `ttzip`).
- [x] **Phase 2: Public C-ABI Subset (`ttzip.h`)**: Export minimal semver-stable C11 SDK header ($\le 100\text{ LOC}$) alongside internal `ttzip_rust_glue.h`.
- [x] **Phase 3: Swift 6 SDK Consolidation (`TTZipCore`)**: Swift 6 Strict Concurrency compliance with `AsyncThrowingStream` progress pipelines.
- [x] **Phase 4: Standalone CLI Enhancements**: Add `--json` NDJSON streaming events and shell completion generators to the 19 CLI commands.
- [x] **Phase 5: Zero-Cloud-Cost Local CI/CD Pipeline**: Harden pre-push Git hooks enforcing test suites, license audits, regression gates, and single-file $\le 800\text{ LOC}$ ceiling.

### Milestone 2 Scope (Subsequent Features)
- **Feature 216**: Physical Repository Split (`ttzip-core` standalone repo vs. `ttzip-apple` multiplatform Xcode repo).
- **Feature 217**: Python PyO3 native SDK directly consuming `ttzip-engine`.
- **Feature 218**: Java 22+ FFM & Node.js `napi-rs` multi-language bindings.

---

## 4. Functional Requirements

### 4.1 Legal & Open-Source Governance
- **REQ-GOV-001**: `ttzip-core` MUST be licensed under `BSD-3-Clause OR Apache-2.0`.
- **REQ-GOV-002**: `ttzip-apple` and all client applications MUST be licensed under `GPL-3.0-or-later` with explicit Trademark and Trade Dress protection clauses reserving brand name "TTZip", official icons, and official App Store distribution rights.
- **REQ-GOV-003**: The root directory MUST maintain `LICENSE-BSD`, `LICENSE-APACHE`, `LICENSE-GPL`, and a standardized `NOTICE` file specifying copyright attribution and origin.
- **REQ-GOV-004**: All source files in the core engine, C-bridge, Swift SDK, and CLI MUST update their SPDX header to:
  `// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0`
  while client UI application files update to:
  `// SPDX-License-Identifier: GPL-3.0-or-later`
- **REQ-GOV-005 (Upstream Attribution & Harvester)**: `scripts/generate_acknowledgements.py` MUST automatically scan and harvest all upstream third-party licenses (libdeflate, zstd, libarchive, lz4, fast-lzma2, zopfli, lzfse, uchardet, Sparkle) into `ACKNOWLEDGEMENTS.md`, `docs/THIRD_PARTY_LICENSES.md`, and `Acknowledgements.plist` for GUI about boxes.

### 4.2 Rust 3-Crate Workspace Architecture
- **REQ-ENG-001**: The Rust workspace MUST contain 3 distinct crates:
  1. `ttzip-engine` (`rlib`): Pure Rust compression/decompression kernels (ZIP, 7z, TAR, GZ, BZ2, XZ, ZSTD) with `#![forbid(unsafe_code)]` on format decoders.
  2. `ttzip-glue` (`staticlib`, `cdylib`): FFI wrapper exporting C11 symbols with `std::panic::catch_unwind` boundaries.
  3. `ttzip-cli` (`bin` named `ttzip`): Standalone binary packaging 19 CLI subcommands and Ratatui TUI.
- **REQ-ENG-002**: All FFI entry points in `ttzip-glue` MUST return `TTZipStatus` enum values and prevent uncaught panics from crossing the ABI boundary.

### 4.3 Public C-ABI Subset (`ttzip.h`) & Internal Bridge (`ttzip_rust_glue.h`)
- **REQ-ABI-001**: A compact public C11 header `Sources/CTTZipBridge/include/ttzip.h` ($\le 100\text{ LOC}$) MUST be exposed for third-party consumers, wrapping archive creation, extraction, inspection, and buffer decompression.
- **REQ-ABI-002**: The comprehensive `ttzip_rust_glue.h` (550 LOC) MUST remain available internally for `TTZipCore` and advanced tooling (VFS cache, worker pool, password recovery, hex diff).
- **REQ-ABI-003**: Progress callbacks MUST adhere to the signature:
  `typedef bool (*TTZipProgressCallback)(uint64_t processed_bytes, uint64_t total_bytes, const char *current_entry, void *user_data);`
  where returning `true` continues execution and `false` aborts.

### 4.4 Swift 6 SDK (`TTZipCore`)
- **REQ-SDK-001**: `TTZipCore` MUST provide a pure Swift 6 API with `Sendable` domain structures (`ArchiveCompressionOptions`, `ArchiveEntryMetadata`, `ArchiveProgress`).
- **REQ-SDK-002**: Extraction and compression facades MUST expose `AsyncThrowingStream` progress pipelines with cooperative cancellation.

### 4.5 Standalone CLI (`ttzip`)
- **REQ-CLI-001**: The CLI binary MUST be named `ttzip` and provide all 19 subcommands (`create`, `extract`, `list`, `tree`, `bench`, `check`, `repair`, `diff`, `split`, `hash`, `convert`, `recover`, `cat`, `comment`, `doctor`, `info`, `delete`, `lock`, `update`).
- **REQ-CLI-002**: The CLI MUST support `--json` emitting Newline-Delimited JSON (NDJSON) streaming events to `stdout`.
- **REQ-CLI-003**: The CLI MUST provide automated shell completion generators for `bash`, `zsh`, `fish`, and `powershell`.

### 4.6 Zero-Cloud-Cost Local CI/CD Pipeline
- **REQ-CI-001**: All verification MUST run locally via `scripts/install_local_git_hooks.sh` and `scripts/run_local_ci_gate.sh`.
- **REQ-CI-002**: The local pre-push hook MUST enforce the single-file $\le 800\text{ LOC}$ ceiling and execute all test stages with 0 cloud runner minutes used.

---

## 5. Non-Functional Requirements & Success Criteria

| Metric | Target | Verification Method |
| :--- | :--- | :--- |
| **FFI Call Overhead** | $< 100\text{ ns}$ per invocation | Microbenchmark suite |
| **Panic Leaks across FFI** | 0 panic leaks (100% caught) | Rust panic containment tests |
| **Single-File LOC Ceiling** | $\le 800$ lines per file | `scripts/lint_loc_gate.sh` |
| **Local CI Execution Time** | $\le 60\text{ s}$ on Apple Silicon / x86_64 | `scripts/run_local_ci_gate.sh` |
| **License Compliance** | 100% pass across all dependencies | `scripts/audit_licenses.py` |
| **Cloud Runner Cost** | $\$0.00$ | Local Git pre-push hook execution |
