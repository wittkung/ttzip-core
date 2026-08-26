# Phase 0 Research: 180-architecture-streamlining-and-core-headless-purity

## Research Item R001: 7z Engine Layering & Dummy Header Removal
- **Decision**: Remove the redundant `SevenZipParallelExtractor` and `SevenZipParallelWriter` onion layers, routing `NativeSevenZipEngine` directly to `SevenZipCAdapter` (`ttzip_rust_create_archive` / `ttzip_rust_extract_archive`), and rewrite `SevenZipHeaderReader` to call `ttzip_rust_scan_entries` directly.
- **Rationale**: 
  - Eliminates 4 layers of redundant forwarding and restores real entry metadata parsing in place of `archive_content_0` mock placeholders.
  - Ensures accurate progress reporting and proper passing of filtering parameters (`skipMacJunk`).
- **Alternatives Considered**: 
  - *Keep existing onion wrappers*: Adds unnecessary stack depth, maintenance overhead, and parameter loss.
- **Source**: 
  - `Sources/TTZipCore/SevenZip/NativeSevenZipEngine.swift:L82-113`
  - `Sources/TTZipCore/SevenZip/SevenZipHeaderReader.swift:L52-66`

---

## Research Item R002: Standards Compliance & Magic Signature Delegation
- **Decision**: Thin out `StandardsComplianceChecker.swift` and `ArchiveMagicSignatureScanner.swift` by delegating directly to Rust `ttzip-glue::standards` via `ttzip_rust_detect_format_file`, `ttzip_rust_detect_format_buffer`, and `ttzip_rust_check_compliance_file`.
- **Rationale**: 
  - Eliminates over 3,200 LOC of duplicated pure-Swift parsing and format validation rules that had drifted from the Rust core.
  - Unifies multi-anchor magic scanning (Sector 16 for ISO, offset 257 for Tar, SFX offsets) under single high-performance Safe Rust implementation.
- **Alternatives Considered**: 
  - *Maintain dual Swift and Rust standards engines*: High risk of behavioral drift and format discrepancy between GUI and CLI.
- **Source**: 
  - `Sources/TTZipCore/Standards/StandardsComplianceChecker.swift`
  - `Sources/TTZipCore/Standards/ArchiveMagicSignatureScanner.swift`
  - `rust/ttzip-glue/src/standards/ffi.rs`

---

## Research Item R003: Headless TTZipCore Purity & FileClipboardStore Relocation
- **Decision**: Move `FileClipboardStore.swift` from `Sources/TTZipCore/Services/` to `Sources/TTZipApp/Services/` and ensure zero AppKit or SwiftUI imports remain in `Sources/TTZipCore`.
- **Rationale**: 
  - `TTZipCore` is the headless core engine library. AppKit and SwiftUI dependencies break cross-platform compilation on Linux and Windows and pollute CLI/Bench targets.
- **Alternatives Considered**: 
  - *Add `#if canImport(AppKit)` conditional blocks*: Still conceptually misplaces presentation/pasteboard logic in the core headless domain.
- **Source**: 
  - `Sources/TTZipCore/Services/FileClipboardStore.swift:L8-15`

---

## Research Item R004: Intermediate Temp File Elimination in Composite Tar Streams
- **Decision**: Refactor `NativeBrotliEngine.swift` composite TAR handling to use Rust streaming pipe encoders (`ttzip_rust_create_tar_brotli_streaming`), eliminating intermediate disk `.tar` file writes.
- **Rationale**: 
  - Eliminates $2\times$ disk write/read amplification on `.tar.br` and `.tar.zst` operations, improving performance and protecting SSD endurance.
- **Alternatives Considered**: 
  - *Allocating full in-memory Tar buffer in Swift*: Risk of OOM on large archives. Rust streaming pipe bounds memory to 4MB.
- **Source**: 
  - `Sources/TTZipCore/Brotli/NativeBrotliEngine.swift:L140-170`
