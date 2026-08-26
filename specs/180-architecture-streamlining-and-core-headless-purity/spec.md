# Feature Specification: 180-architecture-streamlining-and-core-headless-purity

## 1. Executive Summary & Strategic Motivation
Following the fifth-round comprehensive audit across all remaining Swift and Rust source files in TTZip, several layers of architectural redundancy and platform boundary leakage remain:
1. **7z Engine Onion Layers & Mock Residue**:
   - `SevenZipParallelExtractor.swift` and `SevenZipParallelWriter.swift` act as redundant 30-line wrappers discarding parameters.
   - `SevenZipHeaderReader.swift` contains a legacy dummy stub (`archive_content_0`) instead of calling the fully featured Rust 7z TOC scanner.
2. **Standards & Magic Signature Duplication**:
   - Over 3,200 LOC of duplicated pure-Swift standards and magic checking exist across `StandardsComplianceChecker.swift`, `ArchiveMagicSignatureScanner.swift`, and extensions, duplicating the Rust `ttzip-glue::standards` engine.
3. **Piped Tar/Brotli/Zstd Stream & Zero Intermediate Disk I/O**:
   - Eliminating the $2\times$ disk I/O amplification in `NativeBrotliEngine.swift` (which wrote intermediate uncompressed `.tar` files to disk) by streaming directly through Rust memory pipes.
4. **TTZipCore Headless Purity**:
   - Relocating `FileClipboardStore.swift` out of `TTZipCore` (or removing its AppKit/SwiftUI imports) so `TTZipCore` remains 100% headless and cross-platform compilable on Linux and Windows.
5. **Design Pattern Framework Thinning**:
   - Streamlining `BaseArchiveEngineTemplate.swift`, `ConcreteStates.swift`, and `ConcreteVisitors.swift` to thin structures delegating directly to Rust C-ABIs.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Headless Cross-Platform Compilation
- **Given** TTZipCore built on Linux or in a headless daemon
- **When** compiling without AppKit / SwiftUI
- **Then** 0 missing module errors occur, guaranteeing 100% pure headless core portability.

### User Scenario 2: Instant 7z Solid Stream Extraction
- **Given** a 7z archive inspected or extracted
- **When** reading header descriptors or solid streams
- **Then** the engine reads authentic entries with zero mock fallbacks and extracts in-memory with zero disk write amplification.

### User Scenario 3: Unified Standards & Magic Sniffing
- **Given** any of 17 supported archive formats (Zip, 7z, Tar, Gz, Bz2, Xz, Zstd, ISO, DMG, Snappy, LZ4, etc.)
- **When** testing compliance or sniffing format
- **Then** Swift delegates directly to `ttzip-glue::standards`, guaranteeing 100% consistency across CLI, TUI, and GUI.

---

## 3. Success Metrics
1. **Headless Purity**: 0 `import AppKit` or `import SwiftUI` statements inside `Sources/TTZipCore`.
2. **Zero Code Duplication**: Standards and format inspection unified under Rust `standards::ffi`.
3. **LOC Compliance**: 100% of first-party source files kept under $< 350\sim 500\text{ LOC}$.
4. **Zero Regression**: 100% pass rate across 175+ Rust tests, 880+ Swift tests, and 7/7 local CI stages.

---

## 4. Clarifications
- **Q1: What happens to FileClipboardStore?**
  - **Decision**: `FileClipboardStore.swift` is moved to `Sources/TTZipApp/Services/` where AppKit and SwiftUI dependencies legitimately belong.
- **Q2: How are StandardsComplianceChecker and ArchiveMagicSignatureScanner bridged?**
  - **Decision**: They invoke `ttzip_rust_detect_format_file`, `ttzip_rust_detect_format_buffer`, and `ttzip_rust_check_compliance_file`, mapping returned C structs/JSON directly to Swift models.
