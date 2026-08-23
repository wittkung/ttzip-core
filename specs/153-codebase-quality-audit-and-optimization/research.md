# Research & Architectural Audit Report: Codebase Quality Audit and Optimization

**Feature Branch**: `153-codebase-quality-audit-and-optimization`
**Created**: 2026-08-20
**Spec**: [spec.md](./spec.md)

---

## Technical Context & Baseline

- **Language / Runtime**: Swift 6.0 (`swift-tools-version: 6.0`), C11 (`-O3 -Wall -Wextra -Wvla -Wformat=2`).
- **Target Platform**: macOS 14.0+ (ARM64 Apple Silicon NEON & PMULL + x86_64).
- **Core Architecture**: In-process static C library bindings (`CTTZipBridge` -> `Vendor/libTTZipVendor.a` + `TTZipVendor.xcframework`). Zero CLI subprocess execution on hot paths.
- **Logging Subsystem**: Centralized `TTLogger` with thread-safe `NSLock` synchronization, privacy levels, and log emission filtering.
- **Localization Subsystem**: Type-safe `L10n` enum namespaces across 17 groups (295 keys) backed by 7 language catalogs (`En`, `ZhHans`, `ZhHant`, `Ja`, `De`, `Es`, `Fr`).

---

## Research Items

### R001: Localization Catalog Key Structure & Error Mapping Alignment

#### Problem Statement
Investigate key alignment between `Sources/TTZipCore/Localization/` (`LocaleCatalog+*.swift`, `LocaleKey.swift`, `ArchiveError+L10n.swift`), identify missing or mismatched keys, and resolve discrepancies in archive error mapping.

#### Findings
1. **Catalog Structure**: 7 language catalogs (`LocaleCatalog+En.swift`, `LocaleCatalog+ZhHans.swift`, `LocaleCatalog+ZhHant.swift`, `LocaleCatalog+Ja.swift`, `LocaleCatalog+De.swift`, `LocaleCatalog+Es.swift`, `LocaleCatalog+Fr.swift`) each hold 296 key-value pairs.
2. **Key Hierarchy**: `LocaleKey.swift` defines 295 strongly typed enum cases across 17 namespaces (`Common`, `Sidebar`, `Explorer`, `Compress`, `Extract`, `Benchmark`, `Presets`, `Vault`, `Settings`, `Queue`, `Preview`, `Menu`, `Dialogs`, `Errors`, `Units`, `CLI`, `Notification`).
3. **Error Mapping Discrepancy**:
   - In `ArchiveError+L10n.swift`, `.readFailed(code:)` was mapped to `L10n.Errors.corruptedHeader` ("Archive header magic check failed or corrupted.") instead of `L10n.Errors.readError` ("Failed to read data from source stream.").
   - `ArchiveError.errorDescription` in `ArchiveReader.swift:24-49` returned static English strings instead of calling into the localized `localizedDescription()`.

#### Decision
- Map `ArchiveError.readFailed` to `L10n.Errors.readError`.
- Update `ArchiveError.errorDescription` in `ArchiveReader.swift` to delegate to `localizedDescription()`, unifying error presentation across SwiftUI, AppKit, and CLI.
- Maintain 100% key parity across all 7 catalogs for `L10n.Errors`.

#### Rationale
- `L10n.Errors.readError` represents stream and IO read failures accurately without diagnosing a false header magic failure.
- Delegating `errorDescription` to `localizedDescription()` provides a single source of truth for error string formatting across both Foundation `LocalizedError` protocol consumers and explicit localization managers.

#### Alternatives Considered
- *Alternative*: Introduce an isolated `L10n.Archive` namespace specifically for archive engine errors.
  - *Reason for Rejection*: Redundant with existing `L10n.Errors` namespace; creates unnecessary catalog fragmentation and duplicates 8+ keys across 7 catalog files.

#### Source
- `Sources/TTZipCore/Localization/Extensions/ArchiveError+L10n.swift:10-39`
- `Sources/TTZipCore/Localization/LocaleKey.swift:331-346, 387-403`
- `Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+En.swift:269-282`
- `Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+ZhHans.swift:269-282`
- `Sources/TTZipCore/Localization/Catalogs/LocaleCatalog+De.swift:269-282`
- `Sources/TTZipCore/ArchiveReader.swift:12-50`
- `Tests/TTZipTests/LocalizationIntegrityTests.swift:17-28, 80-98`

---

### R002: Logging Hygiene & Four Systemic Engineering Invariants Audit

#### Problem Statement
Scan `Sources/` and `Sources/CTTZipBridge/` for bare print/printf logging, evaluate adherence to the Four Systemic Engineering Invariants (Zero-Memory Assumption, Bounds-First, Invariant-First, Oracle-First), and audit pointer allocation arithmetic for integer overflow vulnerabilities.

#### Findings
1. **Logging Scan**:
   - One bare `print` statement was identified in `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift:139`:
     `print("[WARNING] ExtremeBlockWriter: blockIdx \(blockIdx) failed compression, using uncompressed fallback (len: \(currentChunkSize))")`
   - C bridge files in `Sources/CTTZipBridge/*.c` have zero bare `printf`, `fprintf`, `puts`, or `NSLog` calls; errors are formatted strictly via `snprintf`/`vsnprintf` into caller buffers.
2. **Sensitive Memory Clearing**:
   - `Sources/CTTZipBridge/include/CTTZipCommon.h:98-113` implements `ttzip_secure_zero` using `memset_s` (Apple/C11), `explicit_bzero` (Linux), and a compiler memory barrier (`__asm__ __volatile__("" : : "r"(ptr) : "memory")`) to eliminate Dead-Store Elimination (DSE) risk.
   - Crypto contexts across `CTTZipBridge_Crypto.c`, `CTTZipBridge_ZipWrite.c`, `CTTZipExtract.c`, `ttzip_7z_kdf_arm64.c`, and `ttzip_lzma2_enc_native.c` strictly call `ttzip_secure_zero`.
3. **Magic Lifecycle Sentinels**:
   - `TTZIP_STRUCT_MAGIC` (`0x545A4950U`) and `TTZIP_POISON_FREE` (`0xDEADBEEFU`) are correctly initialized upon allocation and overwritten on deallocation in `CTTZipStreamCoder.c`, `CTTZipSuperChunk.c`, `CTTZipVLMeta.c`, and `ttzip_tar_native.h`.
4. **Allocation Size Overflow Checking**:
   - Multi-element allocations `malloc(sizeof(T) * count)` in `CTTZipExtract.c:269` (`total_entries * sizeof(ttzip_parsed_entry_t)`), `CTTZipBridge_7zSolid.c:106` (`num_files * sizeof(uint64_t)`), and `CTTZipBridge_Crypto.c:556` can be hardened using `ttzip_mul_overflow` to prevent heap buffer overflow on corrupted archive headers.

#### Decision
1. Replace the bare `print` in `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift:139` with `TTLogger.shared.warning(...)`.
2. Harden dynamic array allocations in `CTTZipExtract.c`, `CTTZipBridge_7zSolid.c`, and `CTTZipBridge_Crypto.c` with `ttzip_mul_overflow` bounds checks.
3. Validate all 5 targets against Swift 6.0 compiler warnings and enforce strict invariant conformance.

#### Rationale
- Centralized logging via `TTLogger` guarantees level filtering, telemetry capture, and thread safety.
- Hardening multi-element array allocations with overflow checks prevents integer wrap-around attacks from hostile headers.

#### Alternatives Considered
- *Alternative*: Wrap bare `print` in `#if DEBUG` instead of migrating to `TTLogger`.
  - *Reason for Rejection*: Violates logging constitution; silences telemetry in release builds rather than allowing structured diagnostic collection.

#### Source
- `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift:139`
- `Sources/TTZipCore/Utilities/Logger.swift:12-60`
- `Sources/CTTZipBridge/include/CTTZipCommon.h:40-67, 98-113`
- `Sources/CTTZipBridge/CTTZipBridge_Crypto.c:296-300, 481-496, 556`
- `Sources/CTTZipBridge/CTTZipExtract.c:189, 269`
- `Sources/CTTZipBridge/CTTZipBridge_7zSolid.c:106`
- `Package.swift:1-112`
