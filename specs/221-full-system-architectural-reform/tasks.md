# Tasks: Full System Architectural Reform & Defect Remediation

**Feature**: `221-full-system-architectural-reform`  
**Status**: In Progress  

---

## Phase 1: Foundational & Rust Workspace Governance

- [ ] T001 Clean up dead duplicate directories in `rust/ttzip-glue/src/` and update `rust/ttzip-glue/src/lib.rs`
- [ ] T002 Update `rust/ttzip-glue/Cargo.toml` with `hex = "0.4"` dev-dependency
- [ ] T003 Merge `rust/ttzip-engine/src/bench/` into `rust/ttzip-engine/src/benchmark/` and update re-exports in `rust/ttzip-engine/src/lib.rs`
- [ ] T004 [P] Verify `cargo test --manifest-path rust/Cargo.toml` compiles cleanly

---

## Phase 2: Rust Core Engine Fast-Path & Streaming Pipeline

- [ ] T005 Connect pure Rust ZIP, TAR, 7z engines in `rust/ttzip-engine/src/archive/unified/create.rs` and `extract.rs`
- [ ] T006 Implement batch selective extraction in `rust/ttzip-engine/src/archive/unified/extract_single.rs` and C-ABI export in `unified.rs`
- [ ] T007 Connect `VirtualMultiVolumeReader` in `rust/ttzip-engine/src/archive/unified/inspect.rs`
- [ ] T008 [P] Update C-ABI headers in `Sources/CTTZipBridge/include/ttzip_rust_glue.h`
- [ ] T009 Compile and package Rust staticlib into `Vendor/TTZipVendor.xcframework` via `./scripts/build_rust.sh --release`

---

## Phase 3: Swift 6 Core Engine Safety, Transactions & Concurrency

- [ ] T010 Implement `DifferentialExtractTransaction` in `Sources/TTZipCore/Commands/ExtractCommand.swift`
- [ ] T011 Implement `AtomicCompressTransaction` in `Sources/TTZipCore/Commands/CompressCommand.swift`
- [ ] T012 Fix `ArchiveExtractor.swift` single-file extraction and batch selective extraction
- [ ] T013 Update `ArchiveReader.swift` to eliminate physical `.001` split volume concatenation
- [ ] T014 Harden `SecureBytes.swift` `init(utf8String:)` to prevent heap memory leakage
- [ ] T015 Remove full-path lock interning in `ArchiveEntry.swift` & `ArchiveEntryMetadata.swift` with `ArchiveMimeMapper`
- [ ] T016 Create `NativeComputeDispatcher.swift` and deprecate `boostCurrentThreadPriority()` in `PlatformHardware.swift`
- [ ] T017 Extract `ConcurrencyBridge.swift` and delete `MemoryPagePool.swift`

---

## Phase 4: AppKit/SwiftUI Presentation Layer & VFS Search Optimization

- [ ] T018 Implement `RustVfsSession.swift` and update `RustVfsBridge.swift` for persistent tree lifetime
- [ ] T019 Implement `ArchiveOutlineItem.swift` and update `NativeArchiveOutlineView+Delegates.swift`
- [ ] T020 Update `ArchiveTreeStore.swift` to use persistent `RustVfsSession` for instant search

---

## Phase 5: Verification, Benchmarking & Gate Closure

- [ ] T021 Run `swift test` and verify 100% test pass rate
- [ ] T022 Run `./scripts/lint_loc_gate.sh` to enforce single-file LOC limits
- [ ] T023 Run `./scripts/run_local_ci_gate.sh` and ensure all quality gates pass
