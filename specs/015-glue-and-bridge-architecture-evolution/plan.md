# Implementation Plan: 015 100% Pure UniFFI Architecture & Total C-ABI Decommissioning

- **Feature Directory**: `specs/015-glue-and-bridge-architecture-evolution`
- **Classification**: `[Full SDD]`
- **Status**: `Planning`
- **Created**: 2026-08-25
- **Author**: Antigravity AI & TTZip Architectural Governance Team

---

## 1. Technical Context & Scope of Changes

### 1.1 Complete Legacy Decommissioning & UniFFI Scaffolding Consolidation
This plan executes the complete removal of all legacy manual C-ABI code and achieves a 100% pure Mozilla UniFFI architecture:

1. **Rust Engine UniFFI Layer (`core/rust/ttzip-engine/src/uniffi_api/mod.rs`)**:
   - Expand UniFFI scaffolding to cover 100% of engine operations:
     - `create_archive`, `extract_archive`, `inspect_archive`, `extract_selected`.
     - `UniFFIVfsTree` object with in-memory windowed paging (`get_children`) and fuzzy search.
     - `CancellationToken` object for atomic Swift-Rust cancellation.
     - `ProgressHandler` callback interface for 60Hz throttled telemetry.
     - `extract_audio_waveform` / `extract_audio_waveform_from_memory`.
     - `repair_archive`, `verify_integrity`, and `recover_password`.
   - Purge `core/rust/ttzip-engine/src/ffi/` manual C-ABI exports.

2. **Package & Build Configuration Clean-up**:
   - Remove `CTTZipBridge` target from `core/Package.swift` and `apple/Package.swift`.
   - Remove all manual C header files (`ttzip_rust_glue.h`, `ttzip.h`).
   - Run `uniffi-bindgen generate` to produce updated, verified Swift bindings (`ttzip_engine.swift`).

3. **Swift Layer Pure-UniFFI Refactoring (`core/Sources/TTZipCore/`)**:
   - Delete `CUnsafeBufferAdapter.swift`, `ClosureBox.swift`, and manual `ProgressBridgeContext.swift`.
   - Refactor `ArchiveReader`, `ArchiveWriter`, `ArchiveExtractor`, `ArchiveSelectiveExtractor`, `RustVfsSession`, and `NativeMicrokernelBridge` to call generated UniFFI methods and types.
   - Refactor `TTZipEngineFacade` and `TTZipEngine` actor to use pure UniFFI objects.

4. **Multi-Language SDK Synchronization**:
   - Generate UniFFI bindings for Python, Kotlin/Java, Go, Dart, and .NET from the single authoritative Rust UniFFI source.

---

## 2. Constitution Check & Architectural Invariants

- **Zero-Subprocess Policy**: 100% compliant. All operations are strictly in-process native UniFFI foreign function calls.
- **Strict Single-File LOC Threshold ($\le 800$ LOC)**: All files in `core/Sources/TTZipCore/` and `core/rust/ttzip-engine/src/uniffi_api/` will maintain $\le 350$ LOC, strictly capped at 800 LOC.
- **Zero In-Tree Path Invariant**: 100% compliant.
- **Distribution-Centric CI**: `make test-out-of-tree-smoke` passes 100% across all multi-language SDKs.
- **Living & Executable Examples**: All SDK quickstart examples updated to pure UniFFI bindings.

---

## 3. Execution Phases

### Phase 0: Research & Benchmarking (`research.md`)
- [x] Analyze UniFFI RustBuffer transfer performance vs. manual C-ABI.
- [x] Design UniFFI native Callback Interface and atomic Cancellation Token models.
- [x] Establish single source of truth architecture across all language ecosystems.

### Phase 1: Design Artifacts (`data-model.md`, `contracts/`, `quickstart.md`)
- [x] Define UniFFI IDL records, enums, objects, and callback interfaces.
- [x] Validate JSON contract schemas using `lint-contracts.sh`.
- [x] Create end-to-end quickstart validation guide for pure UniFFI workflows.

### Phase 2: Rust UniFFI Engine Expansion (`core/rust/ttzip-engine/src/uniffi_api/`)
- Implement `TTZipEngineCore` with `create_archive`, `extract_archive`, `inspect_archive`, and `extract_selected`.
- Implement `ProgressHandler` callback interface and `CancellationToken` object.
- Implement `UniFFIVfsTree` object with `get_children`, `fuzzy_search`, and `get_stats`.
- Export audio waveform extraction and archive repair via UniFFI.

### Phase 3: Legacy C-ABI Purge & Build Manifest Clean-up
- Remove `core/Sources/CTTZipBridge/` and manual C headers.
- Remove `CTTZipBridge` target from `core/Package.swift` and `apple/Package.swift`.
- Purge `core/rust/ttzip-engine/src/ffi/` manual C-ABI exports.
- Run `uniffi-bindgen` to generate updated `ttzip_engine.swift`.

### Phase 4: Swift Core Migration to 100% Pure UniFFI
- Delete `CUnsafeBufferAdapter.swift`, `ClosureBox.swift`, and manual `ProgressBridgeContext.swift`.
- Refactor `ArchiveReader`, `ArchiveWriter`, `ArchiveExtractor`, `ArchiveSelectiveExtractor`, and `RustVfsSession` to use UniFFI APIs.
- Migrate `actor TTZipEngine` to use `TTZipEngineCore` with `withTaskCancellationHandler`.
- Ensure 100% Swift 6 strict concurrency compliance (`-strict-concurrency=complete`) with zero `@unchecked Sendable`.

### Phase 5: Multi-Language SDK Synchronization
- Synchronize Python, Kotlin/Java, Go, Dart, and .NET SDKs to UniFFI-generated bindings.

### Phase 6: Systemic Verification & Quality Gates
- Execute `swift test` across `core` and `apple`.
- Execute `cargo test --all` across Rust workspace.
- Run `make test-out-of-tree-smoke` to verify multi-language distribution.
