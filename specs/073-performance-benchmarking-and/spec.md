# Feature Specification: 073-performance-benchmarking-and-readme-reconstruction

**Feature Branch**: `073-performance-benchmarking-and`

**Created**: 2026-08-18

**Status**: Draft

**Input**: User description: "详细调研我们自己的项目，并且好好的去跑，就是设计一下我们怎么去展示我们的性能，这个要非常的专业。然后我们去跑的非常具体的数据，然后去进行这个比较。可以再把不同的比较和在当前版本下的都做一个文档，然后放到我们的 docs 里面，然后在这个 readme 里面去引用，而且好好的调研一下我们这个项目。要我们的 read me 写得很不专业，我们要非常好地去重写它 /speckit-specify ，而且我们相关的开源和商业的这个权限和协议，都有一些变更，你要好好去看"

---

## Executive Overview

TTZip is an ultra-high-performance native compression and archiving engine for macOS 14+, architected with 100% in-process C11 static bindings, Apple Silicon SIMD / PMULL hardware vectorization, and Swift 6 strict concurrency. 

This specification establishes a three-pillar overhaul:
1. **Grounded Empirical Benchmarking & Exhaustive Documentation**: Run physical monotonic benchmarks across all 16 supported formats against industry-standard competitors (Apple native ditto, 7-Zip 7zz, GNU/BSD tar, pigz, zstd, xz, lz4, brotli, Keka CLI equivalents) across diverse workloads (massive small files, log texts, high-entropy payloads, large 500MB streams). Author an exhaustive, peer-reviewable performance whitepaper at `docs/PERFORMANCE.md` with complete mathematical and environmental transparency.
2. **Top-Tier Professional `README.md` Reconstruction**: Completely redesign and author a world-class, professional `README.md` reflecting modern systems engineering standards. Integrate high-impact benchmark comparison cards, full 16-format capabilities, comprehensive CLI documentation (`compress`, `extract`, `list`, `test`, `bench`, `inspect`, `health`, `man`, `completion`), Homebrew and binary installation flows, macOS Desktop UI capabilities, in-depth architectural highlights, and upstream open-source contributions.
3. **Authoritative Legal, Licensing & Commercial Terms Alignment**: Eliminate licensing ambiguities (such as outdated badge metadata) and clearly codify the Source-Available / Anti-Copycat model alongside Enterprise Commercial Licensing across `README.md`, `LICENSE`, `ACKNOWLEDGEMENTS.md`, and documentation.

---

## Clarifications

### Session 2026-08-18
- Q: Which specific workloads and competitor baselines must be captured in the physical benchmarking suite? → A: All 16 supported formats across Massive Small Files (100 files), Real Log Text (10MB), High-Entropy Payload (100MB), and Large Stream (500MB), compared against Apple Native ditto, 7-Zip (7zz multithreaded), GNU/BSD tar, pigz, zstd, xz, lz4, and brotli.
- Q: Where should the full performance whitepaper and raw benchmark datasets reside? → A: Primary comprehensive whitepaper at `docs/PERFORMANCE.md`, machine-readable and dated historical run reports under `docs/benchmarks/`, and executive comparative summary cards in `README.md`.
- Q: How should the licensing terms and commercial policies be standardized across the repo? → A: TTZip Source-Available & Anti-Copycat Public License for personal/CLI/research use, strict ban on third-party app store redistribution and white-label copycats, and a dedicated Enterprise Commercial Licensing channel for commercial products/pipelines.
- Q: Which CLI subcommands and capabilities must be comprehensively documented in README.md? → A: All 9 subcommands (`compress`, `extract`, `list`, `test`, `bench`, `inspect`, `health`, `man`, `completion`), UNIX pipe streaming (`stdin`/`stdout`), and Homebrew Tap installation (`brew install wittkung/tap/ttzip-cli`).

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Prospective User & Open-Source Evaluator (Priority: P1)

As a macOS software engineer or systems administrator browsing the TTZip repository, I want to immediately understand the software's unique value proposition, supported format matrix, hardware acceleration mechanisms, and installation commands so that I can evaluate and install TTZip within 60 seconds.

**Why this priority**: The `README.md` is the primary entry point for 100% of developers, evaluators, and adopters. It must communicate technical authority, clarity, and precision instantly.

**Independent Test**: Can be verified by a clean review of the restructured `README.md`, testing installation commands (`brew install wittkung/tap/ttzip-cli`, `swift build`, direct download links), and confirming zero broken links or inaccurate descriptions.

**Acceptance Scenarios**:
1. **Given** a developer visiting the repository root, **When** they view `README.md`, **Then** they see an informative hero header, verified badges (accurate license, Swift 6, macOS 14+, Apple Silicon), core technical highlights, full 16-format matrix, quick-start installation, and comprehensive CLI command references.
2. **Given** a user looking for command line syntax, **When** they read the CLI section in `README.md`, **Then** they can copy and execute working commands for archiving, extraction, pipe streaming, format inspection, health checks, and shell completion.

---

### User Story 2 - Performance Engineer & Benchmark Auditor (Priority: P1)

As a performance engineer or tech lead evaluating compression throughput on Apple Silicon, I want to review rigorous, reproducible, physically measured benchmark data comparing TTZip against native and third-party competitors across realistic workloads, along with direct links to full methodology documents in `docs/`, so that I have verifiable proof of throughput and latency claims.

**Why this priority**: Performance claims without reproducible test suites, environmental parameters, and competitor baselines lack credibility. TTZip's core differentiator is raw physical throughput.

**Independent Test**: Can be verified by running `swift run ttzip-cli bench` and performance test suites on Apple Silicon, checking that recorded metrics match `docs/PERFORMANCE.md` and summarized tables in `README.md`.

**Acceptance Scenarios**:
1. **Given** a reader looking at performance comparisons in `README.md`, **When** they click the performance whitepaper link, **Then** they are navigated to `docs/PERFORMANCE.md` containing detailed tables, test hardware configurations (CPU, RAM, OS version), workload descriptions, and exact reproduction commands.
2. **Given** a developer executing `ttzip-cli bench -f all` or `swift test --filter XCTestPerformanceMeasureTests`, **When** the benchmark runs, **Then** the physical throughput meets or exceeds published floors without variance anomalies.

---

### User Story 3 - Enterprise Decision Maker & Commercial Adopter (Priority: P2)

As an enterprise technical leader or commercial software vendor, I want to clearly understand the licensing terms, permitted uses (personal, CLI, research, upstream contributions), strict prohibitions (no third-party app store publishing, no white-label copycats), and enterprise licensing paths so that our organization stays fully compliant.

**Why this priority**: Prevents unauthorized commercial abuse, white-label app store scraping, and provides a clear commercial licensing channel for enterprise users.

**Independent Test**: Can be verified by inspecting `README.md`, `LICENSE`, and `ACKNOWLEDGEMENTS.md` to ensure complete consistency, zero conflicting license declarations, and unambiguous legal boundaries.

**Acceptance Scenarios**:
1. **Given** an enterprise evaluator reviewing the license, **When** they read `README.md` and `LICENSE`, **Then** they find clear, unambiguous terms distinguishing free personal/CLI/audit use from prohibited public redistribution/app store hosting and commercial enterprise deployment.
2. **Given** an open-source maintainer, **When** they inspect `ACKNOWLEDGEMENTS.md`, **Then** all third-party libraries (libarchive, libdeflate, XZ Utils, zstd, LZ4, 7-Zip, libb2, uchardet, Sparkle) have accurate upstream authors, licenses, and reciprocal contribution notes.

---

### User Story 4 - Desktop Power User & Mac Enthusiast (Priority: P3)

As a macOS desktop user, I want to explore TTZip's GUI capabilities (native AppKit/SwiftUI glassmorphic interface, In-Archive QuickLook preview, Password Vault v4, Archive Metadata Inspector, and Health Check vulnerability scanner) through clear visual descriptions and documentation.

**Why this priority**: TTZip delivers both a standalone high-performance CLI tool and a native macOS GUI application; both must be professionally showcased.

**Independent Test**: Can be verified by checking that `README.md` and `docs/` showcase GUI features with accurate descriptions matching implemented capabilities in `TTZipApp`.

**Acceptance Scenarios**:
1. **Given** a Mac desktop user reading `README.md`, **When** they review GUI features, **Then** they see organized sections detailing In-Archive QuickLook, Mojibake auto-repair, Archive Inspector, and Password Vault.

---

## Edge Cases

- **Mixed Architecture Hardware**: How does benchmark documentation handle Apple Silicon vs Intel x86_64 performance? Documentation must explicitly state Apple Silicon hardware specifications (core counts, P/E distribution, NEON/PMULL) and note x86_64 baseline behaviors.
- **Competitor Multithreading Parity**: How do we ensure competitor benchmarks are fair? Competitor tools must be configured with maximum hardware multithreading (`-mmt=on`, `-T0`, `-p max`, `-n max`) to ensure true apples-to-apples competition.
- **License Badge Consistency**: What happens if third-party badge providers or cached READMEs display outdated licenses? All badges, footers, and license text in `README.md`, `Formula/ttzip-cli.rb`, and `docs/` must be synchronously aligned.
- **Documentation Link Integrity**: What happens if documentation files are renamed or moved? All relative links across `README.md`, `docs/`, `ACKNOWLEDGEMENTS.md`, and `CONTRIBUTING.md` must be checked for 100% path resolution.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001 [Empirical Benchmarking]**: System MUST execute physical benchmark suites covering all 16 supported formats (ZIP, 7Z, TAR, TAR.ZST, TAR.GZ, TAR.BZ2, TAR.XZ, WIM, DMG, LZ4, LZIP, LRZIP, AAR, ISO, BROTLI, SNAPPY) across standard workloads (Massive Small Files, Real Log Text, High-Entropy Binary, Large 500MB Data).
- **FR-002 [Competitor Delta Analysis]**: Benchmarking suite MUST capture and calculate throughput speedups and compression ratios against native macOS tools (`ditto`, `tar`, `zip`), 7-Zip (`7zz`), `pigz`, `zstd`, `xz`, `lz4`, and `brotli`.
- **FR-003 [Comprehensive Performance Whitepaper]**: System MUST generate `docs/PERFORMANCE.md` detailing:
  - Physical test environment (Apple Silicon CPU, memory, macOS version, compiler flags).
  - Exhaustive throughput and compression ratio tables across all 16 formats.
  - Hardware vectorization mechanics (PMULL CRC64/CRC32, AES-256 SIMD, SWAR match finding).
  - In-process static binding latency advantages vs external subprocess spawning.
  - Step-by-step reproduction guide with exact CLI commands.
- **FR-004 [README.md Overhaul]**: System MUST reconstruct `README.md` into a top-tier open-source document featuring:
  - Professional typography, badge bar, and engineering philosophy.
  - Full-matrix format support table (16 formats with compression, extraction, and QuickLook support status).
  - Benchmark exhibition cards and comparative tables with direct links to `docs/PERFORMANCE.md`.
  - Comprehensive CLI usage manual covering all subcommands (`compress`, `extract`, `list`, `test`, `bench`, `inspect`, `health`, `man`, `completion`), pipe streaming (`stdin`/`stdout`), and flags.
  - Installation guide covering Homebrew Tap (`brew install wittkung/tap/ttzip-cli`), precompiled releases, Mac App Store, and source compilation.
  - Desktop GUI features showcase (QuickLook, Charset repair, Password Vault v4, Inspector).
  - In-depth Architecture & Invariants section (Zero-cost abstraction, SIMD vectorization, Swift 6 concurrency).
  - Upstream contributions and community giving-back section.
  - Clear, legally precise License & Enterprise Commercial Licensing terms.
- **FR-005 [Legal & Licensing Synchronization]**: System MUST synchronize and verify all licensing statements across `README.md`, `LICENSE`, `ACKNOWLEDGEMENTS.md`, and `Formula/ttzip-cli.rb`, ensuring zero contradictory statements (e.g., removing any erroneous `BSD-3-Clause` badge).
- **FR-006 [Documentation Cross-Referencing]**: System MUST audit and interconnect `docs/README.md`, `docs/PERFORMANCE.md`, `docs/competitor_benchmark_report.md`, `ARCHITECTURE.md`, `CONTRIBUTING.md`, and `ACKNOWLEDGEMENTS.md`.

---

### Key Entities

- **BenchmarkMetricRecord**: Structured measurement record containing format, compression level, encryption mode, input workload type, input size, compressed size, compression ratio, packing throughput (MB/s), extraction throughput (MB/s), competitor baseline throughput, and acceleration factor ($\Delta\%$).
- **FormatCapabilityEntry**: Matrix entry defining format extension, MIME type, compression capability, decompression capability, penetration/QuickLook capability, underlying C engine, and hardware acceleration features.
- **LicenseSpecification**: Definitive legal structure defining permitted non-commercial uses, forbidden public distribution/app store publishing actions, trademark protection, and commercial licensing guidelines.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Physical benchmark runs yield 100% complete data points across all 16 supported formats and workloads with zero missing cells in `docs/PERFORMANCE.md`.
- **SC-002**: `README.md` delivers a comprehensive, professional documentation suite spanning all required sections with 100% valid, verified internal and external markdown links.
- **SC-003**: 100% of CLI subcommands documented in `README.md` are tested and executable via copy-paste.
- **SC-004**: Legal and licensing terms across all repository documents (`README.md`, `LICENSE`, `ACKNOWLEDGEMENTS.md`, `Formula/ttzip-cli.rb`) are 100% consistent with zero conflicting license declarations.
- **SC-005**: All unit tests, regression tests, and performance gates in `./scripts/run_local_ci_gate.sh` pass with 100% green status.

---

## Assumptions

- Benchmarks will be executed on the physical host Apple Silicon machine running macOS Sonoma / Sequoia under single-user load conditions to ensure monotonic timing accuracy.
- Competitor binaries (`7zz`, `pigz`, `zstd`, `xz`, `lz4`, `brotli`, `ditto`, `tar`) are available or simulated via standard test harness suites in `Tests/TTZipTests/`.
- Homebrew formula reference points to `wittkung/tap/ttzip-cli` as established in Feature 072.
- The project retains its dual-distribution architecture (MAS Sandbox + Direct Sparkle) and Source-Available Anti-Copycat License with Commercial Enterprise options.
