# Phase 0 Technical Research: CLI Test System, Full Coverage, and Standards Professionalization

**Feature**: `070-cli-test-system-standards-professionalization`  
**Date**: 2026-08-17  
**Status**: Completed  

---

## Research Item 1: Unified International Compression & Archive Format Specification Architecture

- **Decision**:
  Implement a centralized, immutable `ArchiveFormatStandardSpec` catalog and registry (`Sources/TTZipCore/Standards/ArchiveFormatStandardSpec.swift`) providing strongly typed metadata for all 16 supported formats. The specification includes:
  1. Governing standard citations (RFC, ISO, POSIX, PKWARE, Apple, Google, Igor Pavlov).
  2. Multi-anchor magic signature scanning (`.head(offset)`, `.sector(index, offset)` for ISO 9660 `CD001`, `.tail(offset)` for DMG `koly`, `.tarOffset(257)` for UStar).
  3. Standard Extra Field parsing and validation (`0x5455` Extended Timestamp, `0x7075` Unicode Path, `0x7875` Info-ZIP UNIX, `0x0001` Zip64, `0x9901` WinZip AES).
  4. 3-Tier encryption classification and multi-volume spanning metadata.
- **Rationale**:
  Format definitions were previously scattered across `ArchiveCompressionTypes.swift`, `ArchiveHeaderMagicHandler.swift`, `SevenZipHeaderReader.swift`, and `ZstdHeaderReader.swift`. Centralizing into `ArchiveFormatStandardSpec` decouples format standards from specific compression engines, provides zero-allocation microsecond signature scanning via `UnsafeRawBufferPointer`, and preserves isolation from the frozen ZIP engine core.
- **Alternatives Considered**:
  - *Extending `ArchiveCompressionFormat` enum directly*: Rejected because bloating the base UI-bound enum violates Single Responsibility Principle and creates heavy coupling between UI and format standard verification logic.
  - *Relying solely on C-level `libarchive` format detection*: Rejected because ISO, DMG, Apple Archive, Snappy, and LRZIP are handled by different modules and C-boundary FFI calls incur unnecessary allocation overhead compared to native Swift `UnsafeRawBufferPointer` scanning.
- **Source**:
  - PKWARE APPNOTE.TXT v6.3.10: `https://pkware.cachefly.net/webdocs/APPNOTE/APPNOTE-6.3.10.TXT`
  - IETF RFC 8878 (Zstandard): `https://datatracker.ietf.org/doc/html/rfc8878`
  - IETF RFC 1952 (GZIP) & RFC 1951 (DEFLATE): `https://datatracker.ietf.org/doc/html/rfc1952`
  - IETF RFC 7932 (Brotli): `https://datatracker.ietf.org/doc/html/rfc7932`
  - `Sources/TTZipCore/ArchiveCompressionTypes.swift`
  - `Sources/TTZipCore/ChainOfResponsibility/ArchiveHeaderMagicHandler.swift`
  - `Sources/TTZipCore/Zip/ZipCentralDirectoryReader.swift`

---

## Research Item 2: Differential Oracle Comparison Testing Harness

- **Decision**:
  Implement `DifferentialOracleTestHarness` (`Tests/TTZipTests/DifferentialOracleTestHarness.swift`) executing a 3-way bidirectional validation matrix:
  1. **TTZip-Compress ➔ Oracle-Extract**: TTZip creates the archive, reference tools (`/usr/bin/tar`, `bsdtar`, `/usr/bin/unzip`, `7zz`) extract and verify.
  2. **Oracle-Compress ➔ TTZip-Extract**: Reference tool creates the archive, TTZip extracts and verifies.
  3. **Cross-Oracle Manifest Verifier**: Evaluates a 5-dimension manifest (`FileTreeManifest`): SHA-256 data integrity, APFS normalized hierarchy, symlink/hardlink preservation, POSIX permission bits, and 16-byte hex diff diagnostics.
  4. Core macOS binaries (`/usr/bin/tar`, `/usr/bin/unzip`) are mandatory; extended tools (`bsdtar`, `7zz`) are dynamically discovered with `XCTSkip` fallback.
- **Rationale**:
  Comparing compressed archive binaries directly is impossible due to variable compressor metadata, timestamps, and Huffman coding headers. Comparing extracted filesystem manifests guarantees functional parity and zero functional regression against industry-standard tools.
- **Alternatives Considered**:
  - *External bash script harness*: Rejected because it cannot run natively inside `swift test`, lacks Swift assertion integration, and does not run inside IDE test navigators.
  - *Static fixture comparisons only*: Rejected because static fixtures only test decompression of legacy archives without exercising TTZip's compressors.
- **Source**:
  - `Tests/TTZipTests/SystemDifferentialTests.swift#L28-L118`
  - `Sources/TTZipCore/SubprocessExecutor.swift#L9-L73`
  - `Sources/TTZipCore/HashCalculator.swift#L62-L141`
  - `Tests/TTZipTests/TTZipAssertions.swift#L9-L146`

---

## Research Item 3: Programmatic Mutation & Malformed Stream Fuzzing Engine

- **Decision**:
  Implement `MalformedStreamFuzzEngine` (`Sources/TTZipCore/Security/MalformedStreamFuzzEngine.swift`) utilizing:
  1. `DeterministicPRNG: RandomNumberGenerator` with explicit 64-bit seed initialization to guarantee zero flakiness across runs.
  2. Composable mutation operators: `.corruptMagic`, `.corruptCRC`, `.truncateStream`, `.injectZipSlipPath`, `.oversizeHeader`, `.invalidDictSize`.
  3. Crash-First file persistence: writes corrupted buffers to an isolated sandbox file prior to decoder invocation, ensuring immediate forensic reproducers if an unexpected abort occurs.
  4. Assertions verifying that C parsers return negative `ttzip_error_t` status codes (`TTZIP_ERR_CORRUPT_HEADER = -4`, `TTZIP_ERR_SECURITY_VIOLATION = -30`) and Swift wrappers throw typed `ArchiveError.invalidFormat` without panicking or triggering AddressSanitizer leaks.
- **Rationale**:
  Fuzzing against deep format structures prevents memory safety bugs, buffer overflows, and security vulnerabilities (Zip Slip path traversal, infinite loops on malformed LZMA2/Zstd streams).
- **Alternatives Considered**:
  - *LLVM libFuzzer (`-fsanitize=fuzzer`)*: Rejected as the primary CI regression driver because it requires special compiler harnesses, is non-deterministic by default, and cannot be invoked within standard `swift test` runs.
- **Source**:
  - `Sources/CTTZipBridge/CTTZipDiagnostics.c`
  - `Sources/CTTZipBridge/include/CTTZipCommon.h`
  - `Sources/TTZipCore/Platform/PlatformPathSanitizer.swift`
  - `Sources/TTZipCore/Facades/ArchiveSecurityFacade.swift`

---

## Research Item 4: High-Usability Diagnostic Test Output & Formatted Hex Diff Engine

- **Decision**:
  Enhance `FastHexDiffEngine` with 64-byte SIMD chunk hopping (zero-allocation on passing tests) and lazy task-local diagnostic messages. Upon assertion failure, format 16-byte aligned side-by-side hex and ASCII dumps highlighting diverging bytes in bold ANSI colors.
  Extend `ttzip-cli test` with `--standard <format>`, `--differential <oracle>`, `--fuzz`, `--tier <0..5>`, `--json` (single-line NDJSON telemetry), and TTY colored status badges (`[PASS]`, `[FAIL]`, `[DIFF]`, `[FUZZ]`).
- **Rationale**:
  Line-based diffs are useless for binary archives. Hex diffs with aligned 16-byte windows and color highlighting allow developers to locate corruption offsets instantly. Structured NDJSON telemetry allows automated CI ingestion.
- **Alternatives Considered**:
  - *Spawning external `xxd` / `hexdump`*: Rejected because spawning sub-processes violates in-process architectural invariants and fails in MAS sandboxes.
- **Source**:
  - `Sources/TTZipCore/Testing/FastHexDiffEngine.swift`
  - `Sources/TTZipCore/Testing/DiagnosticContext.swift`
  - `Sources/TTZipCore/CLI/TerminalRenderEngine.swift`
  - `Sources/TTZipCLI/TestCommand.swift`

---

## Research Item 5: Deep Architecture Audit of libarchive Test System & Standards Conformance

- **Decision**:
  Adopt libarchive's premier testing patterns into TTZip:
  1. **Monotonic Negative Error Ordering**: Align error propagation with libarchive's convention (`ARCHIVE_FATAL (-30) < ARCHIVE_FAILED (-25) < ARCHIVE_WARN (-20) < ARCHIVE_RETRY (-10) < ARCHIVE_OK (0)`), where `err_combine = min(a, b)` automatically prioritizes the most severe error.
  2. **Golden `.uu` Corpus Integration**: Use `LibarchiveUUDecoder.swift` to load libarchive's historical 90+ `.uu` golden archive fixtures in `LibarchiveGoldenCorpusTests.swift`.
  3. **High-Precision Assertions**: Extend `TTZipAssertions.swift` to mirror libarchive's `test_common.h` (`assertEqualIntA`, `assertEqualMem`, `assertFileMode`, `assertFileExists`, `assertEmptyFile`, `assertIsDir`, `assertIsSymlink`, `assertIsHardlink`).
  4. **CLI Stdio and Protection Conformance**: Adopt bsdtar's test methodologies (`test_option_O_upper.c`, `test_option_exclude.c`, `test_strip_components.c`, `test_option_k.c`, `test_option_U_upper.c`) into `CLIPOSIXStandardTests.swift`.
- **Rationale**:
  libarchive represents the gold standard for archive library testing developed over 20+ years. Leveraging its `.uu` corpus and assertion design gives TTZip instant compatibility verification against thousands of real-world historical archives and edge cases.
- **Alternatives Considered**:
  - *Relying only on synthetic newly created archives*: Rejected because synthetic encoders miss legacy vendor quirks, corrupt headers, and historical CVE edge cases captured in libarchive's `.uu` corpus.
- **Source**:
  - `Vendor/libarchive-upstream/libarchive/test/test.h`
  - `Vendor/libarchive-upstream/libarchive/test/test_fuzz.c`
  - `Vendor/libarchive-upstream/tar/test/test.h`
  - `Vendor/libarchive-upstream/test_utils/test_common.h`
  - `Vendor/libarchive-upstream/libarchive/archive.h`
  - `Vendor/libarchive-upstream/libarchive/archive_private.h`
  - `Tests/TTZipTests/LibarchiveGoldenCorpusTests.swift`
  - `Tests/TTZipTests/LibarchiveUUDecoder.swift`
