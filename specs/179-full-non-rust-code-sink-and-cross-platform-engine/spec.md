# Feature Specification: 179-full-non-rust-code-sink-and-cross-platform-engine

## 1. Executive Summary & Strategic Motivation
In response to the directive for a comprehensive, all-out non-Rust code audit and sink, this feature performs a total architectural consolidation. We classify all ~150 Swift files across `Sources/TTZipCore` and `Sources/TTZipBench` into:
1. **Platform/UI Anchors (Must Remain Swift)**:
   - macOS SwiftUI Views and ViewModels (`Sources/TTZipApp/`).
   - macOS FinderSync integration (`TTZipFinderSyncController.swift`, `FinderSyncActionRequest.swift`).
   - macOS Touch ID / Biometric Keychain (`TouchIDAuthenticator.swift`, `SecureCredentialResolver.swift`).
   - macOS QuickLook AppKit Provider (`TTZipQuickLookProvider.swift`, `QuickLookPreviewData.swift`).
2. **Core Domain & Engine Logic (100% Sunk to Safe Rust)**:
   - **Domain 1: Security & Path Defense**:
     - `PlatformPathSanitizer.swift`, `SecurityScanner.swift`, `PathPatternFilterEngine.swift`, `SmartExtractResolver.swift` ➔ Sunk to `rust/ttzip-glue/src/security/path_sanitizer.rs`.
   - **Domain 2: Charset Sniffing & Transcoding**:
     - `CharsetDetector.swift`, `CharsetDetectionStrategyProtocol.swift` ➔ Sunk to `rust/ttzip-glue/src/charset/` (Mozilla bigram statistics + `encoding_rs`).
   - **Domain 3: Streaming RS-FEC & Recovery Records**:
     - `ReedSolomonFEC.swift`, `ArchiveRecoveryRecordEngine.swift`, `RecoveryRecordPayload.swift` ➔ Sunk to `rust/ttzip-glue/src/crypto/rs_fec/` (Streaming Cauchy RS, 32B binary digest fix, zero UAF hazards).
   - **Domain 4: File System Recursive Traversal & Preallocation**:
     - `ZipDirectoryScanner.swift`, `FolderStatsCalculator.swift`, `DeepFileMetadataReader.swift`, `ArchiveDiskPreallocator.swift`, `PlatformFileSystem.swift` ➔ Sunk to `rust/ttzip-glue/src/fs/scanner.rs`.
   - **Domain 5: Differential Oracles & In-Memory Fuzzing**:
     - `FastHexDiffEngine.swift`, `MalformedStreamFuzzEngine.swift`, `DifferentialManifestScanner.swift`, `DifferentialManifestVerifier.swift` ➔ Sunk to `rust/ttzip-glue/src/testing/`.
   - **Domain 6: Platform Hardware & Memory Safety**:
     - `PlatformMemory.swift`, `PlatformHardware.swift`, `HardwareThermalCoordinator.swift`, `AppleSiliconTuner.swift` ➔ Sunk to `rust/ttzip-glue/src/platform/` (Compiler-barrier `zeroize` preventing Dead-Store Elimination, dynamic runtime CPUID topology).
   - **Domain 7: High-Level Facades & Design Patterns Thinning**:
     - `ArchiveOperationsFacade.swift`, `ArchiveBatchFacade.swift`, `ArchiveStreamingFacade.swift`, `BaseArchiveEngineTemplate.swift`, `ConcreteStates.swift`, `ConcreteVisitors.swift`, `ConcreteRepositories.swift` thinned to pure C-ABI delegations.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Total Cross-Platform Independence
- **Given** TTZip compiled for Linux (x86_64 / aarch64) or Windows
- **When** running any archive extraction, creation, verification, or charset transcoding
- **Then** 0 calls are made to Darwin-only APIs (CoreFoundation, sysctlbyname, mach_task_basic_info), achieving 100% portable native execution.

### User Scenario 2: Zero Memory Safety Hazards
- **Given** high-concurrency extraction or password recovery
- **When** processing millions of paths, keys, and data slices
- **Then** 0 dangling pointers (`withUnsafeBytes` escape), 0 dead-store key residue in RAM, and 0 unbounded allocations occur.

### User Scenario 3: Maximum Throughput & Ultra-Fast Walking
- **Given** a directory tree with 200,000 files and deep symlink structures
- **When** creating an archive
- **Then** directory scanning finishes in $<200\text{ms}$ with zero symlink loop hangs and full preservation of project dotfiles (`.gitignore`, `.env`).

---

## 3. Success Metrics
1. **Engine Sinking Completion**: 100% of core algorithms, security sanitizers, format sniffers, RS-FEC engines, directory walkers, and fuzzers live in Safe Rust.
2. **File Size Compliance**: 100% of first-party files in the workspace strictly maintained at $< 350\sim 500\text{ LOC}$.
3. **Zero Breaking Changes**: 100% pass rate across 200+ Rust tests, 872+ Swift tests, and 7/7 local CI stages.

---

## 4. Clarifications
- **Q1: How are CJK character encodings detected without CoreFoundation?**
  - **Decision**: Uses a Mozilla-grade bigram 2-byte statistical transition frequency model evaluating byte sequences against pre-calculated probability matrices for GB18030, Shift-JIS, Big5, EUC-KR, and Windows-1252, followed by `encoding_rs::Encoding::decode_without_bom_handling_and_fail_on_malformed_str`.
- **Q2: How does the parallel directory scanner handle symbolic links?**
  - **Decision**: Tracks `(dev_id, inode)` pairs in a concurrent `DashSet` or thread-local `HashSet`. If a directory symlink points to an already visited ancestor inode, it is skipped with a warning, preventing infinite recursion.
- **Q3: How are Swift design pattern wrappers maintained?**
  - **Decision**: Swift `ArchiveOperationsFacade`, `ArchiveBatchFacade`, `ArchiveVisitorProtocols`, and `ArchiveTemplateContext` are preserved as lightweight, public Swift APIs, but their implementations delegate directly to Rust C-ABI with 0 redundant pure-Swift business logic.

