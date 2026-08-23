# Feature Specification: CLI Test System, Full Coverage, and Compression/Decompression Standards Professionalization

**Feature Branch**: `070-cli-test-system-standards-professionalization`  
**Created**: 2026-08-17  
**Status**: Draft  
**Input**: Comprehensive comparative gap analysis and modernization of `ttzip-cli` test systems, coverage depth, ergonomics, systematics, and industrial-grade professional standards adherence across all 16 supported compression and archive formats (`/speckit-specify`).

---

## 1. Executive Summary & Industry Benchmark Gap Analysis

An exhaustive audit of `ttzip-cli`'s testing infrastructure and standards conformance against world-class industrial tools (`libarchive`/`bsdtar`, `7-Zip`/`7zz`, `zstd`, `libdeflate`, `ripgrep`, `ouch`, and GNU tools) establishes that while TTZip possesses unmatched physical throughput on Apple Silicon (28+ GB/s), its testing and standards architecture has concrete gaps across four pillars:

```
┌─────────────────────────────────────────────────────────────────────────────────────────────────┐
│                      TTZip CLI Testing & Standards Modernization Matrix                         │
├──────────────────────────┬─────────────────────────────┬────────────────────────────────────────┤
│ Dimension                │ Market Benchmark (`bsdtar`, │ Target TTZip Professional Architecture │
│                          │ `7z`, `zstd`, `libdeflate`) │ (Feature 070 Specification)            │
├──────────────────────────┼─────────────────────────────┼────────────────────────────────────────┤
│ 1. Testing System &      │ Independent C test runners, │ Hybrid XCTest + Standalone C Oracles,  │
│    Oracles               │ crash-first fuzzing, golden │ Golden UU Corpus, Fault Injection      │
│                          │ bit-exact oracles, ASan/    │ Harness, Deterministic Seed Matrix,    │
│                          │ UBSan sanitizers in CI.     │ PTY TTY Interactive Automation Suite.  │
├──────────────────────────┼─────────────────────────────┼────────────────────────────────────────┤
│ 2. Coverage & Edge-Case  │ 95%+ branch coverage,       │ 16-format boundary fuzzing, malformed  │
│    Robustness            │ zip-bomb defense, truncated │ header fault injection, Zip64/Pax      │
│                          │ stream recovery, TOCTOU/    │ boundary tests, zero-allocation memory │
│                          │ symlink attack immunity.    │ leak verification, SIGINT/PIPE exits.  │
├──────────────────────────┼─────────────────────────────┼────────────────────────────────────────┤
│ 3. Ergonomics &          │ Reproducible 1-command runs,│ `ttzip-cli test --standard <fmt>`,     │
│    Usability             │ hex diff diagnostics, zero- │ `--differential <oracle>`, clean diffs │
│                          │ dependency fixtures, clear  │ with byte-level dumps, programmatic    │
│                          │ machine-readable test logs. │ deterministic test corpus generator.   │
├──────────────────────────┼─────────────────────────────┼────────────────────────────────────────┤
│ 4. Systematics & Format  │ Strict RFC/ISO/POSIX format │ Unified Format Standards Registry,     │
│    Standards Adherence   │ compliance, Extra Field IDs,│ APPNOTE/POSIX/RFC/ISO 16-format parity,│
│                          │ Pax/UStar/WinZip AE parity. │ Cross-Oracle Bi-directional Assurance. │
└──────────────────────────┴─────────────────────────────┴────────────────────────────────────────┘
```

---

## 2. Clarifications

### Session 2026-08-17
- Q: What is the primary execution model for differential oracle comparison testing? → A: A dedicated Swift test harness (`DifferentialOracleTestHarness`) executing round-trip archiving and extraction across TTZip and system/installed reference engines (`/usr/bin/tar`, `bsdtar`, `/usr/bin/unzip`), asserting 100% bit-exact SHA-256 payload parity, symlink target preservation, and POSIX permission mode equality.
- Q: How will malformed input security and robustness testing be driven? → A: A programmatic mutation engine (`MalformedStreamFuzzEngine`) using deterministic pseudo-random seeds to inject bit flips in magic headers, truncated compression payloads, invalid dictionary sizes, and malicious path traversal vectors (`../../`), asserting that TTZip always exits cleanly with descriptive error codes and zero process panics.
- Q: How are format standards definitions formalized in production code? → A: A centralized `ArchiveFormatStandardSpec` catalog in `TTZipCore/Standards/ArchiveFormatStandardSpec.swift` providing strongly typed metadata (RFC/ISO numbers, magic byte sequences, extra field IDs, header specs, and compliance validator routines) for all 16 supported formats.

---

TTZip supports 16 primary formats and 24 container variants. The table below specifies the official international standards, specifications, and conformance requirements governing each format in TTZip:

| Format ID | Primary Governing Standards / Specifications | Conformance & Feature Specifications |
| :--- | :--- | :--- |
| **ZIP** | **PKWARE APPNOTE.TXT v6.3.10**<br>• PKWARE Zip64 Extensions<br>• WinZip AES Encryption Spec AE-1 / AE-2<br>• Info-ZIP UNIX Extra Fields (0x5455, 0x5855, 0x7875)<br>• Unicode Filename Extra Field (0x7075) | • Central Directory & Local Header parity<br>• Extended timestamps & POSIX UID/GID preservation<br>• AES-128/256 PBKDF2/HMAC authentication<br>• Zip64 4GB+ threshold & 65,535+ entries handling |
| **7Z** | **7-Zip Format Specification (Igor Pavlov)**<br>• LZMA / LZMA2 / Fast-LZMA2 Streams<br>• AES-256-CBC + SHA-256 19-cycle KDF<br>• BCJ / BCJ2 ARM/x86 Call Conversion Filters<br>• Multi-volume Spanning (.7z.001..N) | • `kHeader` and `kEncodedHeader` parsing<br>• Solid block compression & dictionary resets<br>• Non-echo password derivation & hardware NEON AES<br>• Seamless multi-part split joining & extraction |
| **TAR / PAX** | **POSIX.1-1988 (UStar)**<br>**POSIX.1-2001 (Pax Extended Headers)**<br>• GNU Tar Extensions (LongLink, LongName)<br>• GNU Sparse Format v0.0, v0.1, v1.0<br>• Pax Sparse Format v0.0, v0.1 | • 512-byte header block alignment & octal checksums<br>• Unlimited pathname length via Pax `path=` / `linkpath=`<br>• Nanosecond mtime/atime/ctime resolution (`mtime=`)<br>• Sparse file zero-block detection and reconstruction |
| **GZIP** | **RFC 1952 (GZIP File Format v4.3)**<br>**RFC 1951 (DEFLATE Compressed Data Format)** | • 10-byte fixed header (ID1/ID2 = 0x1F, 0x8B, CM=8)<br>• FEXTRA, FNAME, FCOMMENT, FHCRC flag parsing<br>• CRC-32 and ISIZE 32-bit modulo trailer verification |
| **ZSTD** | **RFC 8878 (Zstandard Compression & Media Type)**<br>• Zstandard Frame Header (Magic 0xFD2FB528)<br>• Skippable Frames (0x184D2A50..0x184D2A5F)<br>• Block Headers (Raw, RLE, Compressed) | • Magic header & Frame Content Size (FCS) decoding<br>• Multi-frame concatenated stream decompression<br>• Content XXH64 / CRC checksum verification<br>• Direct I/O zero-copy streaming passes |
| **BZIP2** | **Julian Seward Bzip2 Format Specification**<br>• Burrows-Wheeler Transform (BWT) Block Coding<br>• Move-to-Front (MTF) & Huffman Coding<br>• Block CRC-32 and Stream Combined CRC-32 | • Header magic `BZh1`..`BZh9` (100k..900k block sizes)<br>• 48-bit block magic `0x314159265359` validation<br>• Stream end magic `0x177245385090` & combined CRC |
| **XZ** | **The .xz File Format Specification v1.0.4**<br>• Stream Header & Footer (Magic `0xFD`, `7zXZ`)<br>• Check Types: None (0), CRC32 (1), CRC64 (4), SHA-256 (10)<br>• Variable Length Integers (VLI) & Index Records | • Stream flags and Backward Size integrity<br>• LZMA2 + BCJ ARM64 filter pipeline execution<br>• Multithreaded block decoding with index verification |
| **LZ4** | **LZ4 Frame Format Specification v1.6.2**<br>• LZ4 Block Format Specification<br>• Magic `0x184D2204`, FLG/BD descriptor bytes | • Independent vs Linked block decoding<br>• Header Checksum (XXH32) & Content Checksum<br>• Raw block compression/decompression API |
| **BROTLI** | **RFC 7932 (Brotli Compressed Data Format)** | • WBITS sliding window size (10..24 bits)<br>• Static dictionary & Huffman meta-block decoding<br>• Zero-allocation micro-stream transformations |
| **LZIP** | **Lzip Format Specification (Antonio Diaz Diaz)** | • 6-byte header: `LZIP` (Magic `0x4C, 0x5A, 0x49, 0x50`), version 1<br>• Coded dictionary size (4 KiB .. 512 MiB)<br>• 20-byte trailer: CRC-32, Data Size, Member Size |
| **LRZIP** | **Long Range ZIP Specification (Con Kolivas)** | • RZIP / LRZIP rzip-header + LZMA/Zstandard/Bzip2<br>• Sliding large window (up to RAM size) chunking |
| **WIM** | **Microsoft Windows Imaging (WIM) Specification** | • Fixed 208-byte header (Magic `MSWIM\0\0\0`)<br>• Resource Hash Table & XML Directory Metadata<br>• LZX / XPRESS / LZMS compression chunk decoding |
| **DMG** | **Apple Disk Image (UDIF) Specification** | • 512-byte `koly` trailer at file end (Magic `koly`)<br>• Embedded XML Property List (plist) partition catalog<br>• UDZO (zlib), UDBZ (bzip2), ULFO (lzfse) block chunks |
| **ISO** | **ISO 9660:1988 / ECMA-119**<br>• Joliet Unicode Extensions (UCS-2 Level 1..3)<br>• Rock Ridge Interchange Protocol (IEEE P1282)<br>• El Torito Bootable CD Specification | • Primary Volume Descriptor (PVD) & Supplementary (SVD)<br>• UTF-16BE Joliet long filename tree decoding<br>• POSIX permissions, symlinks, UID/GID from Rock Ridge |
| **AAR** | **Apple Archive Format Specification** | • Apple Archive `AA01` / `PBKDF2` encrypted envelopes<br>• LZFSE / LZVN / RAW chunk stream decoding<br>• Extended attributes (xattr), ACLs, and clone trees |
| **SNAPPY** | **Google Snappy Framing Format Description** | • Stream identifier chunk `0xff 0x06 0x00 0x00 sNaPpY`<br>• Compressed / uncompressed chunks with CRC32C masking |
| **RAR** | **RAR 4.x & RAR 5.0 Technical Specifications** | • RAR5 Magic `0x52 0x61 0x72 0x21 0x1A 0x07 0x01 0x00`<br>• Variable-length header fields & Blake2sp checksums<br>• AES-256 PBKDF2 HMAC-SHA256 encrypted headers |

---

## 3. User Scenarios & Testing *(mandatory)*

### User Story 1 - Standards Conformance & Format Registry Validation (Priority: P1)

As a systems engineer or packaging maintainer, I want `ttzip-cli` to validate archive conformance against official RFC, ISO, and POSIX specifications (e.g. PKWARE APPNOTE, POSIX Pax, RFC 8878, RFC 1952), so that archives produced by TTZip are 100% interoperable with standard system tools worldwide.

**Why this priority**: Format standards compliance is the bedrock of interoperability and enterprise reliability.

**Independent Test**: Can be tested independently by running `ttzip-cli test --standard zip bundle.zip` and asserting full RFC/APPNOTE field compliance against the standard schema.

**Acceptance Scenarios**:
1. **Given** a generated ZIP archive, **When** validating standard conformance, **Then** all Extra Field headers conform strictly to APPNOTE.TXT and Info-ZIP ID definitions (0x5455 for timestamp, 0x7075 for UTF-8 path).
2. **Given** a TAR archive with path length > 100 bytes or nanosecond timestamps, **When** created by `ttzip-cli archive out.tar src/`, **Then** the archive writes standard POSIX.1-2001 Pax Extended Header records (`path=`, `mtime=`, `atime=`) readable by GNU tar and bsdtar.
3. **Given** a Zstandard compressed file, **When** inspecting with standard verification, **Then** Frame Header descriptors, Window Size, and XXH64 content checksums conform 100% to RFC 8878.

---

### User Story 2 - Differential Oracle Comparison Testing (Priority: P1)

As a quality assurance engineer or core developer, I want an automated differential test harness that tests TTZip's archiving, extraction, and listing outputs against native golden reference implementations (`bsdtar`, `7zz`, `unzip`, `pigz`, `zstd`, `xz`), so that any divergence in file content, permissions, or metadata is caught immediately.

**Why this priority**: Differential testing against authoritative reference engines is the only definitive proof of zero functional drift.

**Independent Test**: Can be tested independently by running `ttzip-cli test --differential bsdtar archive.tar.zst` and asserting bit-exact directory parity between TTZip and `bsdtar`.

**Acceptance Scenarios**:
1. **Given** a complex directory tree with symlinks, deep nesting, and empty directories, **When** packed by TTZip and unpacked by `bsdtar` (and vice-versa), **Then** all file hashes, permissions, and symlink targets match 100%.
2. **Given** an encrypted archive created by `7zz` with AES-256, **When** extracted by `ttzip-cli`, **Then** extraction succeeds with identical file checksums.
3. **Given** a corrupted stream with invalid CRC, **When** tested with both TTZip and reference tools, **Then** both fail with matching error classifications.

---

### User Story 3 - Crash-First Malformed Stream & Security Fuzzing (Priority: P2)

As a security engineer, I want `ttzip-cli`'s C and Swift engines to be hardened against malicious, malformed, and adversarial inputs (Zip Slip path traversal, Zip Bombs, corrupted magic headers, truncated payload buffers, integer overflow attacks), so that `ttzip-cli` never crashes, loops infinitely, or leaks memory.

**Why this priority**: Prevents security exploits and crash bugs when processing untrusted inputs from the internet.

**Independent Test**: Can be tested independently by feeding mutated corrupted archives and asserting clean error return codes with zero ASan memory leaks and zero process panics.

**Acceptance Scenarios**:
1. **Given** an archive containing malicious paths like `../../../../etc/passwd` or `C:\Windows\System32`, **When** extracting, **Then** `ttzip-cli` safely sanitizes or rejects the entry without writing outside destination directories.
2. **Given** a mutated archive with random byte flips in compression stream headers, **When** processed by `ttzip-cli`, **Then** the CLI returns non-zero exit code (`TTZIP_ERR_CORRUPT_HEADER` or `TTZIP_ERR_CRC_MISMATCH`) without segmentation faults or heap corruption.
3. **Given** a recursive Zip Bomb (42.zip or deeply nested compressed blocks), **When** inspected or extracted, **Then** expansion limits and memory quotas protect the system from memory exhaustion.

---

### User Story 4 - High-Usability Diagnostic Test Harness & Automated CI Matrix (Priority: P2)

As a developer contributing to TTZip, I want a single-command test suite with clear colored terminal output, detailed hex dump diagnostics on failure, and structured NDJSON test telemetry, so that I can diagnose and fix regressions within seconds.

**Why this priority**: Streamlines contributor workflows and provides immediate root-cause visibility during CI/CD failures.

**Independent Test**: Can be tested independently by running `swift test --filter StandardsTests` and verifying clear per-format compliance summaries and failure diagnostics.

**Acceptance Scenarios**:
1. **Given** a test failure in header decoding, **When** running the test suite, **Then** the diagnostic output prints expected vs actual byte values in formatted hex and ASCII side-by-side.
2. **Given** automated CI execution, **When** tests complete, **Then** summary metrics include pass rate, throughput vs historical baseline, and coverage per format.

---

## 4. Requirements & Technical Invariants *(mandatory)*

### Functional Requirements

- **FR-001**: Implement a unified `ArchiveFormatStandardSpec` cataloging all 16 formats with their governing RFC/ISO/POSIX specifications, magic numbers, header structures, and required metadata fields.
- **FR-002**: Implement a `StandardsComplianceChecker` capable of inspecting archives and reporting field-by-field standard adherence scores and warnings.
- **FR-003**: Implement differential testing harnesses comparing TTZip extraction and compression against system reference tools (`/usr/bin/tar`, `bsdtar`, `unzip`, `7zz`, `zstd`).
- **FR-004**: Implement a programmatic `MalformedArchiveGenerator` for fuzz testing (producing truncated streams, corrupted CRCs, invalid dictionary sizes, and oversized headers).
- **FR-005**: Provide standard Extra Field emitters for ZIP format (0x5455 Extended Timestamp, 0x7075 UTF-8 Path, 0x7875 Info-ZIP UNIX).
- **FR-006**: Ensure Pax Extended Header formatting strictly adheres to POSIX.1-2001 specification for TAR format.
- **FR-007**: Maintain zero heap allocation and zero copy on hot inspection/decompression paths across all compliance checkers.
- **FR-008**: Implement automated test telemetry and formatted hex diffs for all test assertions.

### Key Architectural Invariants

- **Invariant 1 (Zero Subprocess in Core Engine)**: All format parsing, verification, and standards checks must execute 100% in-process via C/Swift bindings.
- **Invariant 2 (Bit-Exact Standards Interoperability)**: Archives produced by TTZip must be decompressible by reference tools (`bsdtar`, `7z`, `unzip`, `gzip`, `zstd`, `xz`) without warnings.
- **Invariant 3 (Memory Safety & Zero Leak)**: All C bridge memory must be strictly bounded with `free`/`munmap` paired allocations, verified under AddressSanitizer (ASan).
- **Invariant 4 (Deterministic Testing)**: Test fixture generators must use deterministic pseudo-random seeds to guarantee 100% reproducible test executions.

---

## 5. Success Criteria *(mandatory)*

- **SC-001**: 100% of all 16 supported formats have documented standards specifications and automated compliance validation tests.
- **SC-002**: Differential cross-validation tests with reference tools achieve 100% pass rate across round-trip packing/unpacking.
- **SC-003**: Adversarial and malformed input test suite (50+ fuzzing cases) demonstrates 0 crashes, 0 unhandled panics, and 0 security escapes.
- **SC-004**: Test execution time for the full standards suite runs in under 3.0 seconds locally.
- **SC-005**: All existing throughput benchmarks and performance floors (`XCTestPerformanceMeasureTests`) remain green with zero regression ($\Delta \ge 0.0\%$).
