# Feature Specification: 015 100% Pure UniFFI Architecture & Total C-ABI Legacy Decommissioning

- **Feature Directory**: `specs/015-glue-and-bridge-architecture-evolution`
- **Classification**: `[Full SDD]`
- **Status**: `Specified`
- **Created**: 2026-08-25
- **Author**: Antigravity AI & TTZip Architectural Governance Team

---

## 1. Executive Summary & Problem Statement

TTZip currently suffers from **dual-bridge architectural schizophrenia**:
1. **Legacy Manual C-ABI Layer (`CTTZipBridge`, `ttzip_rust_glue.h`, `CUnsafeBufferAdapter`, `core/rust/ttzip-engine/src/ffi/`)**:
   - Relies on manual raw pointer manipulation (`UnsafePointer<CChar>`, `UnsafeMutableRawPointer`), `user_data` pointer casting, `strlen`, `strdup` loops, and recursive stack traversal.
   - Requires manual synchronization between C headers, Rust `extern "C"` functions, Swift wrappers, and multiple SDK header copies.
   - Bypasses Swift 6 strict concurrency safety via `@unchecked Sendable` and is prone to memory leaks, dangling pointer dereferences, and stack overflow crashes.
2. **Modern Mozilla UniFFI Scaffolding Layer (`ttzip_engine.swift`, `core/rust/ttzip-engine/src/uniffi_api/`)**:
   - Implements declarative, compile-time verified, memory-safe, and auto-generated cross-language bindings.
   - Generates native Swift 6 `Sendable` types, typed errors (`throws TTZipError`), callback protocols (`ProgressHandler`), and atomic object handles (`CancellationToken`, `UniFFIVfsTree`).

```
[ Current Hybrid / Divergent Architecture - FLAWED ]
Swift / SDKs ──┬──> Hand-Rolled C-ABI (`CTTZipBridge` / raw pointers / unsafe allocs) ──> Rust
               └──> Mozilla UniFFI (`uniffi_api` / typed scaffolding / i18n)         ──> Rust

[ Target 100% Pure UniFFI Architecture - SINGLE SOURCE OF TRUTH ]
Swift UI / SDKs ────> Mozilla UniFFI Engine Scaffolding ────> Rust Microkernel Core
                      (Zero Manual C-ABI, Zero Raw Pointers, Zero Unsafe Casts)
```

**Architectural Decision**:
**100% decommission and delete the hand-rolled C-ABI glue layer (`CTTZipBridge`, `ttzip_rust_glue.h`, `CUnsafeBufferAdapter`, `core/rust/src/ffi/`). Complete migration of all archiving, extraction, inspection, VFS tree navigation, audio waveform analysis, integrity verification, and repair APIs to Mozilla UniFFI.**

---

## 2. Fundamental Paradigm Shifts

```
+-----------------------------------------------------------------------------------+
|                        100% Pure UniFFI Paradigm Shift                            |
+-----------------------------------------------------------------------------------+
| 1. Complete C-ABI Decommissioning | Delete CTTZipBridge, ttzip_rust_glue.h, and  |
|                                   | all manual extern "C" raw pointer functions   |
| 2. Universal UniFFI Engine Model  | Expose all operations via UniFFI Record,      |
|                                   | Enum, Object, and Callback Interface IDLs     |
| 3. Zero Unsafe Pointer Operations | Eliminate UnsafePointer, UnsafeRawPointer,    |
|                                   | strdup, strlen, withCString, and memory casts |
| 4. Native Swift 6 Sendable Types  | UniFFI automatically generates Sendable,      |
|                                   | data-race-free Swift 6 structs and classes   |
| 5. Unified Multi-Language Source  | Single Rust source of truth generates Swift,  |
|                                   | Python, Kotlin/Java, Go, Dart, and C# SDKs   |
+-----------------------------------------------------------------------------------+
```

### Paradigm 1: Complete Elimination of Raw Pointers & Unsafe Buffers
By relying entirely on UniFFI, Swift and SDK clients pass standard high-level types (`[String]`, `Data`, `UInt64`, enums). UniFFI's RustBuffer handles serialization/deserialization with compile-time memory guarantees, completely removing stack recursion risks and heap `strdup` allocation overhead.

### Paradigm 2: First-Class Callback Interfaces & Atomic Cancellation
Progress tracking and cancellation use UniFFI's `#[uniffi::export(callback_interface)] trait ProgressHandler` and `#[derive(uniffi::Object)] struct CancellationToken`. No `void* user_data` boxing, no `Unmanaged` dereferences, and no manual pointer lifecycle management.

### Paradigm 3: In-Memory Persistent VFS Tree via UniFFI Objects
The Rust VFS tree is exported directly as a UniFFI Object (`UniFFIVfsTree`). Swift and SDKs hold an `Arc<UniFFIVfsTree>` reference, enabling $O(1)$ child lookups, fuzzy searching, and metadata querying across boundaries without intermediate serialization.

---

## 3. User Stories & Acceptance Criteria

### User Story 1: 100% Memory-Safe Archiving & Extraction via UniFFI
- **As a** developer using TTZipCore,
- **I want** all compression, extraction, and inspection calls to go through UniFFI typed interfaces,
- **So that** there are zero unsafe pointer dereferences, zero manual memory allocations, and zero stack exhaustion risks.
- **Acceptance Criteria**:
  - `CTTZipBridge` target is completely removed from `Package.swift`.
  - All operations in `ArchiveReader`, `ArchiveWriter`, `ArchiveExtractor`, and `TTZipEngine` execute via UniFFI scaffolding.
  - Zero compiler warnings under Swift 6 strict concurrency (`-strict-concurrency=complete`).

### User Story 2: Persistent VFS Tree Object Lifetime & Search
- **As a** user browsing large archive hierarchies,
- **I want** archive inspection to return an immutable `UniFFIVfsTree` object,
- **So that** directory navigation and search are executed in $< 0.5\text{ms}$ directly in Rust memory.
- **Acceptance Criteria**:
  - `UniFFIVfsTree.build(entries, rootName)` builds the in-memory tree.
  - `tree.getChildren(dirNodeId, offset, limit)` returns paginated `UniFFIVfsMatch` / `UniFFIEntryMetadata` records in $< 0.5\text{ms}$.
  - `tree.fuzzySearch(query)` executes directly in Rust memory without Swift re-allocations.

### User Story 3: Clean Multi-Language SDK Bindings from UniFFI
- **As an** SDK developer on Python, Kotlin/Java, Go, Dart, or C#,
- **I want** bindings to be generated directly from `core/rust/ttzip-engine/src/uniffi_api/`,
- **So that** all languages have 100% feature parity, identical error types, and zero out-of-sync C header files.
- **Acceptance Criteria**:
  - Python SDK consumes UniFFI Python bindings.
  - JVM SDK consumes UniFFI Kotlin/Java bindings.
  - Out-of-tree smoke tests pass across all language ecosystems.

---

## 4. Functional Requirements

- **FR-001**: System MUST remove `CTTZipBridge` and all legacy hand-rolled C header files (`ttzip_rust_glue.h`, `ttzip.h`).
- **FR-002**: System MUST expand `core/rust/ttzip-engine/src/uniffi_api/mod.rs` to cover 100% of engine functionality: compression, extraction, selective extraction, inspection, VFS tree operations, audio waveforms, integrity checks, and repair.
- **FR-003**: System MUST export `ProgressHandler` callback trait and `CancellationToken` object via UniFFI.
- **FR-004**: System MUST refactor `ArchiveReader`, `ArchiveWriter`, `ArchiveExtractor`, and `ArchiveSelectiveExtractor` to call UniFFI APIs directly.
- **FR-005**: System MUST eliminate `CUnsafeBufferAdapter`, `ClosureBox`, and raw pointer `ProgressBridgeContext`.
- **FR-006**: System MUST update `core/Package.swift` and `apple/Package.swift` to remove `CTTZipBridge` dependencies.
- **FR-007**: System MUST synchronize Python, JVM, Go, Dart, and .NET SDKs to consume UniFFI-generated bindings.
- **FR-008**: System MUST enforce single-file size thresholds ($\le 800$ LOC, target $\le 350$ LOC) across all updated source files.

---

## 5. Non-Functional Requirements & Governance Guardrails

1. **Zero-Subprocess Policy**: All SDKs and bridges interact strictly in-process via UniFFI direct foreign function calls.
2. **Strict Single-File LOC Threshold ($\le 800$ LOC)**: All files in `core/Sources/TTZipCore/` and `core/rust/ttzip-engine/src/uniffi_api/` must strictly observe $\le 800$ LOC.
3. **Swift 6 Strict Concurrency**: Full compliance with `-strict-concurrency=complete`, zero `@unchecked Sendable` workarounds.
4. **Distribution-Centric CI**: `make test-out-of-tree-smoke` must pass 100% in isolated temporary environments.
