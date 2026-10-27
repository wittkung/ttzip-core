# Phase 0 Research & Technology Synthesis

**Feature**: `073-performance-benchmarking-and-readme-reconstruction`
**Date**: 2026-08-18
**Status**: Completed

---

## Research Item R001: Physical Performance Benchmarking & Competitor Exhibition Architecture

### Decision
Adopt an empirical **"Dual-Layer Whitepaper + Nanosecond Monotonic Timing + 4-Workload Industrial Coverage + Max-Multithread Competitor Parity + Hardware Vector Deconstruction"** framework:
1. **Measurement Standard**: Use nanosecond monotonic clock timing (`PlatformMonotonicTimer` / `mach_absolute_time()`), warm-up iterations, 5-pass median and peak recording, and 100% byte-accurate checksum verification (CRC32/SHA-256).
2. **Environment Transparency Matrix**: Full disclosure of hardware topology (Apple Silicon M-Series CPU cores, P/E ratio, unified memory, APFS 16KB page size, macOS version and build number, compiler optimization `-O -whole-module-optimization`).
3. **4-Dimensional Industrial Workloads**:
   - Massive Small Files (10MB / 100~500 files): I/O traversal and metadata bound.
   - Realistic Log Text (10MB / 50MB): Dictionary match-finding and LZ/DEFLATE compression bound.
   - High-Entropy Binary Payload (100MB): Entropy detection, fallback bypass, and raw stream throughput.
   - Large Stream (500MB): Zero-copy `mmap`, SIMD acceleration, and memory bandwidth bound.
4. **Competitor Parity**: Benchmark against 10+ standard tools (Apple `ditto`, 7-Zip `7zz`, GNU/BSD `tar`, `zstd`, `pigz`, `xz`, `lz4`, `brotli`) with competitors configured at maximum hardware multithreading (`-mmt=on`, `-T0`, `-p max`).
5. **Hardware Vector Breakdown**: Publish dedicated acceleration metrics for ARM64 PMULL CRC64 (`vmull_p64` reaching 48.1 GB/s, 35.5x speedup) and ARM NEON AES-256 / SHA-256 KDF.

### Rationale
- Monotonic hardware timers eliminate NTP and OS scheduler timing jitter.
- Giving competitors maximum multithreading guarantees zero "strawman" benchmark accusations, providing unshakeable industry credibility.
- 4-dimensional workloads expose real-world performance across I/O traversal, dictionary scanning, and memory bandwidth, not just synthetic best cases.
- Hardware vector breakdown explains the physical mechanism behind TTZip's performance.

### Alternatives Considered
- *Single Synthetic Benchmark (e.g. only XCTest measureBlock)*: Rejected because it cannot benchmark external competitor CLI processes under identical memory-mapped stream conditions and lacks reproducible CLI output.
- *Single Fixed Corpus (e.g. only Silesia)*: Rejected because single-file collections do not measure APFS concurrent directory traversal or multi-hundred megabyte memory-mapped throughput.

### Source
- [`Sources/TTZipCore/Platform/PlatformMonotonicTimer.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Platform/PlatformMonotonicTimer.swift)
- [`Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner+Setup.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Benchmark/CompetitorBenchmarkRunner+Setup.swift)
- [`Sources/CTTZipBridge/ttzip_crc64.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_crc64.c)
- [`docs/competitor_benchmark_report.md`](file:///Users/kevintung/Documents/dev/TTZip/docs/competitor_benchmark_report.md)
- [`docs/benchmarks/benchmark_report_2026-08-15_071939.md`](file:///Users/kevintung/Documents/dev/TTZip/docs/benchmarks/benchmark_report_2026-08-15_071939.md)

---

## Research Item R002: Top-Tier Open-Source README Architecture & Content Blueprint

### Decision
Structure `README.md` following modern tier-1 systems engineering software projects (e.g., `ripgrep`, `uv`, `zstd`, `curl`), employing a **"CLI-First, GUI-Enhanced, Engine-Hardened"** narrative across 10 core sections:
1. **Hero & Badges**: Clean logo, authoritative tagline, verified badges (CI Passing, Swift 6 Strict Concurrency, macOS 14+ Sonoma, Apple Silicon NEON/PMULL, Homebrew Tap, Source-Available License).
2. **Key Highlights**: 100% in-process C bindings, 48 GB/s PMULL CRC, Swift 6 data-race freedom, UNIX pipe streaming.
3. **Quick Installation**: Homebrew Tap (`brew install wittkung/tap/ttzip-cli`), prebuilt universal binaries, Direct DMG, MAS.
4. **CLI 9-Command Suite**: Command quick reference table (`compress`, `extract`, `list`, `test`, `bench`, `inspect`, `health`, `man`, `completion`), options, and UNIX pipe streaming 1-liners (`stdin`/`stdout`).
5. **16-Format Full Matrix**: Formats, compression/decompression status, QuickLook support, multi-volume, specifications.
6. **macOS Native GUI Features**: In-Archive QuickLook preview, `uchardet` mojibake auto-repair, Password Vault v4, Archive Inspector & Health Check.
7. **Physical Monotonic Benchmark**: Key throughput table with Apple M-Series figures and links to `docs/PERFORMANCE.md`.
8. **Modular Architecture & Invariants**: 4-layer architecture, zero-cost hot paths, security invariants.
9. **Upstream Giving Back**: Attributions to libarchive, XZ Utils, libdeflate, zstd, and details on ARM64 upstream contributions.
10. **License & Community Model**: Clear Source-Available terms, strict anti-copycat and anti-plagiarism ban, Enterprise Commercial Licensing.

### Rationale
- Combining CLI and GUI prevents TTZip from being misjudged as either a simple GUI wrapper or an inaccessible CLI tool.
- Provides immediate copy-pasteable commands for developers and DevOps engineers.
- Connects high-level documentation to in-depth technical whitepapers (`docs/PERFORMANCE.md`, `ARCHITECTURE.md`).

### Alternatives Considered
- *CLI-Only README*: Rejected because it conceals powerful AppKit/SwiftUI features like QuickLook penetration and mojibake auto-repair.
- *GUI-App-Only README*: Rejected because it conceals UNIX pipe streaming, CLI subcommands, and CI/CD capabilities.
- *Monolithic All-in-One README*: Rejected because embedding full man pages and full test logs creates an unreadable document; linking to `docs/` maintains high signal-to-noise ratio.

### Source
- [`README.md`](file:///Users/kevintung/Documents/dev/TTZip/README.md)
- [`Formula/ttzip-cli.rb`](file:///Users/kevintung/Documents/dev/TTZip/Formula/ttzip-cli.rb)
- [`Sources/TTZipCLI/CLICommandRouter.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCLI/CLICommandRouter.swift)
- [`Sources/TTZipCore/Standards/ArchiveFormatStandardSpec.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Standards/ArchiveFormatStandardSpec.swift)
- [`ARCHITECTURE.md`](file:///Users/kevintung/Documents/dev/TTZip/ARCHITECTURE.md)

---

## Research Item R003: Licensing Model Synchronization & Legal Clarity

### Decision
Establish the **"TTZip Source-Available & Anti-Copycat Public License v1.0 (TTZip-SAL-1.0) + Enterprise Commercial License + Upstream Carve-Out"** framework and synchronously align all files in the repository:
1. **Accurate License Declaration**:
   - Update `README.md` badge from erroneous `BSD-3-Clause` to `Source-Available / Anti-Copycat`.
   - Update `Formula/ttzip-cli.rb` and `Formula/ttzip.rb` license attributes to accurately declare `:cannot_be_redistributed` or custom identifier.
   - Synchronize `SPDX-License-Identifier` headers across the codebase to `LicenseRef-TTZip-Source-Available-1.0`.
2. **Three-Tier Policy Matrix**:
   - **Community / Personal Tier (100% Free)**: Full source code transparency, local compiling, CLI and GUI usage for personal daily tasks, security audits, and pull requests.
   - **Enterprise Commercial Tier**: Commercial entities embedding TTZip into proprietary commercial products, paid cloud services, or corporate automated pipelines obtain an Enterprise Commercial License.
   - **Upstream Open-Source Carve-Out**: Optimization routines (ARM64 PMULL CRC, SWAR match finders) contributed back to upstream open-source projects (libarchive, XZ Utils, zstd, libdeflate) are explicitly licensed under the respective upstream project's permissive license (BSD-2, MIT, 0BSD).
3. **Strict Anti-Copycat & Anti-Parasitism Ban**:
   - Explicitly forbids any third party from uploading compiled binaries, rebranded forks, or wrappers to the Apple Mac App Store, Microsoft Store, Steam, Setapp, or any marketplace (free or paid).
   - Forbids white-labeling, traffic siphoning, ad embedding, and bundling.

### Rationale
- Eliminates legal confusion between the README badge and the root `LICENSE` file.
- Protects the project from malicious Mac App Store re-packagers while maintaining complete transparency for the open-source community.
- Ensures upstream contributions to foundational libraries are unencumbered by proprietary restrictions.

### Alternatives Considered
- *Pure BSD-3-Clause / MIT*: Rejected because it legally allows third parties to copy, rename, and publish paid/ad-supported clones on the Mac App Store.
- *GPL v3 / AGPL v3*: Rejected because it conflicts with Apple Mac App Store distribution terms and does not prevent third parties from distributing free clones that siphon project visibility.

### Source
- [`LICENSE`](file:///Users/kevintung/Documents/dev/TTZip/LICENSE)
- [`ACKNOWLEDGEMENTS.md`](file:///Users/kevintung/Documents/dev/TTZip/ACKNOWLEDGEMENTS.md)
- [`Formula/ttzip-cli.rb`](file:///Users/kevintung/Documents/dev/TTZip/Formula/ttzip-cli.rb)
- [`Sources/TTZipCore/CLI/CLIPackageManifest.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/CLI/CLIPackageManifest.swift)
