# Specification: Full System Architectural Reform & Defect Remediation

**Feature**: `221-full-system-architectural-reform`  
**Status**: Approved (Full SDD)  
**Author**: Antigravity CTO Agent  
**Date**: 2026-08-24  

---

## 1. Background & Executive Overview

A comprehensive architectural audit of TTZip identified 13 structural defects, performance bottlenecks, and technical debt across the Rust core, C-ABI bridge, Swift 6 domain layer, and AppKit/SwiftUI presentation layer:
1. **Engine Architecture Disconnect**: Rust pure ZIP/TAR/7z engines bypassed in favor of system `libarchive`.
2. **Workspace Redundancy**: `ttzip-glue/src` containing 17 dead duplicate directories and breaking `cargo test`.
3. **Module Duplication**: `bench/` vs `benchmark/` duplicate code.
4. **Memory Explosion in ZIP Compression**: All files buffered in RAM before compression, leading to $O(S_{\text{in}} + S_{\text{out}})$ memory usage.
5. **Selective Extraction Bugs**: `extractSingleFile` ignoring `entryPath` and `ArchiveSelectiveExtractor` allocating fixed 32MB buffers in an $O(N \cdot M)$ loop.
6. **Multi-Volume Inspection Disk Concatenation**: Physically joining 100GB split volumes to disk in `ArchiveReader.swift`.
7. **Destructive Extract Rollback**: `ExtractCommand` copying entire pre-existing target directories (e.g. 300GB `~/Downloads`) and deleting them on failure.
8. **Compress Rollback Inefficiency**: Synchronously copying files without APFS `clonefile()`.
9. **Swift 6 QoS Contamination**: `boostCurrentThreadPriority` modifying `pthread_set_qos_class_self_np` on shared cooperative thread pool.
10. **Global Lock Contention in Path Interning**: 300k+ lock acquisitions in `ArchiveEntryMetadataPool.internPath` on unique paths.
11. **VFS Search Memory Storm**: 100k `strdup`/`malloc` and tree rebuilds per keystroke in `RustVfsBridge.fuzzySearch`.
12. **NSOutlineView Struct Boxing**: `ArchiveTreeNode` struct causing `_SwiftValue` allocations and UI stutter.
13. **Security Memory Leaks & Dead Code**: `SecureBytes(utf8String:)` heap memory residue and unused `MemoryPagePool.swift`.

---

## 2. Requirements & Invariants

### 2.1 Functional Requirements
- **FR-01**: `ExtractCommand` must never copy or delete pre-existing destination directories or non-conflict files on rollback.
- **FR-02**: `ArchiveExtractor.extractSingleFile` must only decompress the specified entry, bypassing other entries.
- **FR-03**: `ArchiveReader.inspect` on multi-volume split archives (`.001`) must parse entries without writing any temporary files to disk.
- **FR-04**: ZIP archive compression must stream data through bounded buffers ($\le 64\text{MB}$ RAM) directly to disk.
- **FR-05**: Fast-path creation/extraction for ZIP, TAR, 7z must dispatch directly to pure Rust engines.
- **FR-06**: Searching in archive hierarchies must reuse persistent Rust `VfsTreeHandle` instances without per-keystroke heap allocation.
- **FR-07**: `NativeArchiveOutlineView` must maintain smooth 60 FPS rendering using reference-type nodes (`ArchiveOutlineItem: NSObject`).
- **FR-08**: `SecureBytes` must never leak plaintext password bytes to standard Swift heap memory before page locking.

### 2.2 Non-Functional Requirements
- **NFR-01**: Zero compiler warnings under strict Swift 6 and Rust flags.
- **NFR-02**: Single-file LOC limit $\le 800$ enforced across all modified/new files.
- **NFR-03**: `cargo test` and `swift test` 100% passing.
