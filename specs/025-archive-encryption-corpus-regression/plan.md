# Implementation Plan: Corpus-Driven Archive Encryption Regression & Acceptance Suite

**Feature Branch**: `025-archive-encryption-corpus-regression`  
**Created**: 2026-08-15  
**Status**: Ready for Tasks (`@speckit-tasks`)  

---

## Technical Context

This feature introduces a comprehensive, corpus-driven encryption test harness directly aligned with and adopting libarchive's gold-standard encryption regression fixtures, establishing a resilient 3-tier encryption introspection architecture, and closing the gap on multi-format encryption validation across ZIP, 7Z, RAR4, and RAR5.

### Phase 0 Research Deliverables
- `- R001 [SUBAGENT:research] 《静态测试语料加载机制与打包格式选型》`: Resolved in [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/025-archive-encryption-corpus-regression/research.md#r001-静态测试语料加载机制与打包格式选型-fixture-loading--storage-strategy)
- `- R002 [SUBAGENT:research] 《WinZip AES 与 7z/RAR5 加密规范与认证校验机制》`: Resolved in [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/025-archive-encryption-corpus-regression/research.md#r002-winzip-aes-与-7zrar5-加密规范与认证校验机制-encryption-matrix--authentication-verification)
- `- R003 [SUBAGENT:research] 《三级加密状态自省与 Swift 错误模型对齐》`: Resolved in [research.md](file:///Users/kevintung/Documents/dev/TTZip/specs/025-archive-encryption-corpus-regression/research.md#r003-三级加密状态自省与-swift-错误模型对齐-3-tier-encryption-introspection--error-handling)

### Phase 1 Design Artifacts
- **Data Model**: [data-model.md](file:///Users/kevintung/Documents/dev/TTZip/specs/025-archive-encryption-corpus-regression/data-model.md)
- **Contracts**:
  - `ArchiveEntry` Schema: [archive_entry.schema.json](file:///Users/kevintung/Documents/dev/TTZip/specs/025-archive-encryption-corpus-regression/contracts/archive_entry.schema.json)
  - `ArchiveProbeRequest` Schema: [archive_probe_request.schema.json](file:///Users/kevintung/Documents/dev/TTZip/specs/025-archive-encryption-corpus-regression/contracts/archive_probe_request.schema.json)
  - `ArchiveProbeResponse` Schema: [archive_probe_response.schema.json](file:///Users/kevintung/Documents/dev/TTZip/specs/025-archive-encryption-corpus-regression/contracts/archive_probe_response.schema.json)
  - `ArchiveFixtureManifest` Schema: [archive_fixture_manifest.schema.json](file:///Users/kevintung/Documents/dev/TTZip/specs/025-archive-encryption-corpus-regression/contracts/archive_fixture_manifest.schema.json)
- **Quickstart Guide**: [quickstart.md](file:///Users/kevintung/Documents/dev/TTZip/specs/025-archive-encryption-corpus-regression/quickstart.md)

---

## Constitution Check & Architectural Invariants

| Constitutional Rule | Compliance Analysis | Status |
| :--- | :--- | :--- |
| **1. Frozen Files Protection** | All changes build on testing and service layer (`Tests/TTZipTests/`, `ArchiveReader`, `ArchiveEntry`). Zero modifications to frozen ZIP core engines (`ZipParallelExtractor`, `ZipCryptoEngine.swift`, `CTTZipBridge_Crypto.c`). | ✅ PASSED |
| **2. Hot-Path Zero-Cost Abstraction** | Corpus tests utilize `Bundle.module` direct file paths, passing directly to POSIX `open()`/`mmap()` in C bridge without intermediate `Data` heap allocations. | ✅ PASSED |
| **3. Hard Throughput Floors** | Verification test execution < 1000 ms aggregate; zero impact on existing 262-dimension benchmark suite (`XCTestPerformanceMeasureTests`). | ✅ PASSED |
| **4. Strict Logging Discipline** | Zero `print`/`printf` statements; all diagnostics route through `TTLogger`. | ✅ PASSED |
| **5. Zero Bare Objects Contract Gate** | All JSON Schemas in `contracts/` declare `"$schema": "http://json-schema.org/draft-07/schema#"` and contain zero bare objects. | ✅ PASSED |

---

## Planned Changes by Component

### Component 1: Build & Resource Infrastructure (`Package.swift`)
- **[MODIFY]** `Package.swift`: Configure `.copy("Fixtures")` in `TTZipTests` target to enable SPM bundle packaging of static archive fixtures.

### Component 2: Test Fixtures & Loader (`Tests/TTZipTests/`)
- **[NEW]** `Tests/TTZipTests/TestFixtureLoader.swift`: Standardized resource loader retrieving binary file URLs from `Bundle.module`.
- **[NEW]** `Tests/TTZipTests/Fixtures/Encrypted/*`: Static encrypted test corpus fixtures:
  - `zip_winzip_aes128_store.zip`
  - `zip_winzip_aes256_deflate.zip`
  - `zip_traditional_zipcrypto.zip`
  - `7z_aes256_data_encrypted.7z`
  - `7z_aes256_header_encrypted.7z`
  - `7z_aes256_partially_encrypted.7z`
  - `rar4_aes128_encrypted.rar`
  - `rar5_aes256_header_encrypted.rar`
  - `rar5_aes256_data_encrypted.rar`

### Component 3: Domain Models & 3-Tier Introspection (`Sources/TTZipCore/`)
- **[MODIFY]** `Sources/TTZipCore/ArchiveEntry.swift`: Add `isEncrypted`, `isDataEncrypted`, `isMetadataEncrypted`, `encryptionMethod` fields with backward-compatible initializers.
- **[MODIFY]** `Sources/TTZipCore/ArchiveReader.swift`: Add `probeEncryption(archivePath:)` and upgrade `ArchiveError` enum to include `passwordRequired(archivePath:tier:)` and `wrongPassword(archivePath:)`.
- **[MODIFY]** `Sources/CTTZipBridge/CTTZipBridge_Archive.c`: Bridge libarchive's `archive_entry_is_data_encrypted` and `archive_entry_is_metadata_encrypted` into C callback.

### Component 4: Test Suites (`Tests/TTZipTests/`)
- **[NEW]** `Tests/TTZipTests/ArchiveEncryptionCorpusTests.swift`: Comprehensive multi-format matrix decryption tests with SHA-256 validation.
- **[NEW]** `Tests/TTZipTests/ArchiveEncryptionIntrospectionTests.swift`: 3-tier encryption probing and header-defense tests.
- **[NEW]** `Tests/TTZipTests/ArchivePassphraseFallbackTests.swift`: Multi-candidate password fallback pipeline tests.

---

## Verification Plan

### Automated Tests
1. `swift test --filter ArchiveEncryptionCorpusTests`
2. `swift test --filter ArchiveEncryptionIntrospectionTests`
3. `swift test --filter ArchivePassphraseFallbackTests`
4. `swift test --filter XCTestPerformanceMeasureTests`
5. `./scripts/run_all_tests.sh`
