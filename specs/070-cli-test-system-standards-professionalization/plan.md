# Implementation Plan: CLI Test System, Full Coverage, and Standards Professionalization

**Feature Branch**: `070-cli-test-system-standards-professionalization`  
**Created**: 2026-08-17  
**Status**: In Progress  

---

## 1. Technical Context & Constitution Check

### Technical Context
- **Language & Runtime**: Swift 6.0 (`swift-tools-version: 6.0`) with strict concurrency checking + POSIX C11.
- **Platform Target**: macOS 14.0+ (Sonoma), Apple Silicon native with Intel fallback.
- **Core Principles**:
  - 100% In-Process C static library bindings (`CTTZipBridge`), zero subprocess invocations in core decompression or inspection paths.
  - Zero heap allocation on hot signature and Extra Field scanning paths via `UnsafeRawBufferPointer`.
  - Strict preservation of the frozen ZIP engine core (`zip-engine-freeze.md`).
  - Bit-exact interoperability with standard tools (`bsdtar`, `7zz`, `/usr/bin/unzip`, `/usr/bin/tar`).

### Constitution Check
- [x] **Zero-Cost Abstraction**: Fast-path signature scanner and Extra Field parser operate directly on raw buffer pointers with 64-byte SIMD chunking.
- [x] **Frozen Engine Integrity**: Zero modifications to frozen ZIP files (`ZipParallelExtractor.swift`, `ZipParallelWriter.swift`, `ZipCryptoEngine.swift`, `CTTZipExtract.c`).
- [x] **Performance Invariant Floor**: All tests execute with zero performance degradation; `XCTestPerformanceMeasureTests` remains green.
- [x] **Memory Safety & Zero Leaks**: C handles strictly paired with `free`/`munmap`, verified under AddressSanitizer.
- [x] **Volatile Password Zeroing**: Sensitive key material zeroed via `ttzip_secure_zero` / volatile pointers.

---

## 2. Phase 0: Research Items

- R001 [SUBAGENT:research] 《统一国际标准规范契约建模与多锚点魔数嗅探》: Investigation of governing RFC/ISO/POSIX specifications for all 16 formats and multi-anchor signature scanning (`research.md#research-item-1`).
- R002 [SUBAGENT:research] 《双向差分预言机测试套件架构设计》: Investigation of `DifferentialOracleTestHarness` comparing TTZip against native reference tools (`research.md#research-item-2`).
- R003 [SUBAGENT:research] 《确定性变异注入与恶意流安全模糊测试引擎》: Investigation of `MalformedStreamFuzzEngine` with reproducible seeds and crash-first persistence (`research.md#research-item-3`).
- R004 [SUBAGENT:research] 《零堆分配 HexDiff 引擎与 CLI 测试子命令体验》: Investigation of `FastHexDiffEngine` with 64-byte chunk hopping and NDJSON telemetry (`research.md#research-item-4`).
- R005 [SUBAGENT:research] 《libarchive 核心测试系统、黄金语料库与断言体系深度审计》: In-depth audit of libarchive's test harness, `.uu` corpus, error severity ordering, and CLI options (`research.md#research-item-5`).

---

## 3. Phase 1: Design Artifacts

- **Data Models**: `data-model.md` — Formal data models for `ArchiveFormatStandardSpec`, `ArchiveMagicSignature`, `ZipExtraFieldRecord`, `DifferentialTestReport`, and `FuzzMutationConfig`.
- **Contracts**:
  - `contracts/standards_spec.json`: JSON Schema (Draft-07) for format standards definitions.
  - `contracts/test_telemetry.json`: JSON Schema (Draft-07) for CLI test telemetry and NDJSON events.
  - `contracts/fuzz_spec.json`: JSON Schema (Draft-07) for fuzzing mutation configurations and crash reproducers.
- **Validation Guide**: `quickstart.md` — Step-by-step verification commands with expected outputs and failure diagnostics.

---

## 4. Component Changes & Architecture Breakdown

### 4.1 TTZipCore Layer (`Sources/TTZipCore/`)
- `Standards/ArchiveFormatStandardSpec.swift`: Unified catalog of all 16 formats with RFC/ISO citations, MIME types, UTIs, and multi-anchor magic signatures.
- `Standards/ArchiveMagicSignatureScanner.swift`: Multi-anchor scanner supporting `.head`, `.sector(16)` (ISO), `.tail(512)` (DMG `koly`), and `.tarOffset(257)` (UStar).
- `Standards/ZipExtraFieldParser.swift`: Zero-allocation TLV parser for Extra Fields (`0x5455`, `0x7075`, `0x7875`, `0x0001`, `0x9901`).
- `Security/MalformedStreamFuzzEngine.swift`: Deterministic PRNG mutation engine with crash-first persistence and error code assertion.
- `Testing/FastHexDiffEngine.swift`: Enhanced SIMD 64-byte chunk hopping with 16-byte aligned ANSI visual diffs.
- `Testing/DifferentialOracleTestHarness.swift`: 3-way bidirectional validation harness with 5-dimension manifest verification.

### 4.2 CTTZipBridge Layer (`Sources/CTTZipBridge/`)
- `CTTZipDiagnostics.c` & `include/CTTZipDiagnostics.h`: Add standard negative error severity ordering helper `ttzip_err_combine(err1, err2)`.
- `CTTZipUtils.c` & `include/CTTZipUtils.h`: Expose multi-anchor magic buffer verification primitives.

### 4.3 TTZipCLI Layer (`Sources/TTZipCLI/` & `Sources/TTZipCore/CLI/`)
- `CLIOptions.swift` & `CLICommandSpec.swift`: Add `--standard <format>`, `--differential <oracle>`, `--fuzz`, `--tier <0..5>`.
- `POSIXCLIArgumentParser.swift`: Parse extended test arguments.
- `CLICommandRouter.swift`: Route `ttzip-cli test` flags to `TestCommand`.
- `TestCommand.swift`: Execute standards verification, differential oracle runs, or mutation fuzzing suites with NDJSON output.

### 4.4 Test Suite Layer (`Tests/TTZipTests/`)
- `ArchiveStandardsComplianceTests.swift`: Comprehensive compliance tests for all 16 formats against `ArchiveFormatStandardSpec`.
- `DifferentialOracleTests.swift`: Bidirectional round-trip tests against `/usr/bin/tar`, `bsdtar`, `/usr/bin/unzip`, `7zz`.
- `ArchiveMutationFuzzTests.swift`: Deterministic mutation fuzzing test suite (50+ cases) verifying memory safety under ASan.
- `LibarchiveGoldenCorpusTests.swift`: Verify decompression of libarchive's `.uu` golden archive suite.
