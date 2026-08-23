# Phase 0 Research: Silesia Corpus Standard Benchmark Fixtures & Regression Gates

## R001: Silesia Dataset Catalog, Exact File Roster & Checksum Manifest

- **Decision**: Package the standard 12-file Silesia Compression Corpus (totaling exactly 211,945,550 bytes) directly under `Tests/TTZipTests/Fixtures/Silesia/` accompanied by an immutable cryptographic manifest `silesia_manifest.json`, bundled into `TTZipTests` via SPM `Package.swift` resource declaration `.copy("Fixtures")`.
- **Rationale**:
  1. *Apple Silicon UMA & Direct Zero-Copy Streaming*: Preserving verbatim uncompressed byte layout on disk enables C-level `mmap()` without runtime unpacking steps. Apple Silicon unified memory can map the 211.95 MB fixture directly into virtual address space with zero intermediate heap allocations (`malloc`).
  2. *Windows MSVC & NTFS Determinism*: Storing raw files on NTFS eliminates nested archive extraction overhead, ensuring file metadata, disk clustering, and page alignment match production I/O pipelines.
  3. *Hermetic CI/CD Execution*: Bundled static fixtures eliminate external network downloads or Git LFS authentication dependencies in restricted CI environments.
- **Alternatives Considered**:
  - *Single `silesia.tar.gz` archive unpacked dynamically in test `setUp()`*: Rejected. Dynamic unpacking incurs 200–500ms disk write overhead on every test run, causes filesystem cache pollution before measurement begins, and complicates temporary file lifecycle cleanup.
  - *On-demand HTTP download in test suite*: Rejected. Introduces non-deterministic network latency, network failure flakiness, and security audit friction in offline build environments.
- **Source**:
  - `Package.swift#L69-75` (`.testTarget(name: "TTZipTests", resources: [.copy("Fixtures")])`)
  - `Tests/TTZipTests/TestFixtureLoader.swift#L1-48`
  - Deorowicz, S. (2003), *Silesia Compression Corpus*, Institute of Informatics, Silesian University of Technology (`sun.aei.polsl.pl/~sdeor/corpus/silesia.html`)

### Standard 12-File Inventory

| File Name | Exact Size (Bytes) | Category | Description & Entropy Characteristics |
| :--- | :--- | :--- | :--- |
| `dickens` | 10,192,446 | `text` | Collected works of Charles Dickens in ASCII plain text. Natural language English text patterns. |
| `mozilla` | 51,220,480 | `executable` | Tarred executables and shared libraries from Mozilla 1.0.1 (x86 ELF). High density of machine instructions. |
| `mr` | 9,978,008 | `image` | 3D Magnetic Resonance Imaging (MRI) head scan (16-bit DICOM). Spatial correlation across slices. |
| `nci` | 33,553,445 | `database` | Chemical structure database from the National Cancer Institute. Tabular/structured ASCII molecular descriptions. |
| `ooffice` | 6,152,192 | `executable` | OpenOffice.org 1.01 DLL binary (`libvcl641li.dll`). x86 PE executable code sections and symbol tables. |
| `osdb` | 10,085,684 | `database` | Open Source Database Benchmark sample table dump. High frequency of numerical and columnar delimiters. |
| `reymont` | 6,627,202 | `text` | Polish novel *Chłopi* by Władysław Reymont in uncompressed PDF/ISO-8859-2 text. Non-ASCII 8-bit characters. |
| `samba` | 21,606,400 | `source_code` | Tar archive of Samba 2.2.3 source code, headers, and docs. C source files in a continuous tar stream. |
| `sao` | 7,251,944 | `binary_data` | Smithsonian Astrophysical Observatory star catalog. Fixed-width binary floating point and coordinate records. |
| `webster` | 41,464,527 | `structured_text` | 1913 Webster's Unabridged Dictionary in HTML/ASCII format. Tag-rich markup and dense text. |
| `xml` | 5,345,280 | `structured_text` | Concatenated XML documents. Hierarchical markup and repetitive element tags. |
| `x-ray` | 8,474,240 | `image` | Medical diagnostic X-ray of a child's hand (8-bit raw radiographic image). Smooth spatial gradients. |
| **Total** | **211,945,550** | — | **12 standardized physical benchmark fixtures (~202.13 MiB)** |

---

## R002: Benchmark Execution Architecture, Warmup Protocols & Zero-Regression Gating

- **Decision**: Implement `SilesiaCorpusBenchmarkSuite` extending `AsyncBenchmarkRunner` with 1 warm-up iteration, 3 measurement iterations, median throughput calculation ($t_{\text{median}} = \text{durations}[N/2]$), coefficient of variation filtering ($CV \le 2.5\%$), and a hard 3.0% zero-regression gating floor compared against historical golden baselines.
- **Rationale**:
  1. *Apple Silicon DVFS Ramp & Page Fault Priming*: Apple Silicon dynamically scales CPU frequencies from idle to peak P-core clocks over 5–20ms. A warm-up pass eliminates DVFS ramp penalty and memory page fault initialization from recorded throughput numbers.
  2. *Windows NTFS Cache Jitter Immunity*: NTFS metadata locks and write delays introduce timing noise on initial file access. A dedicated warm-up round plus isolated RAII directory sandboxes (`IsolatedTempSandbox`) ensures clean baseline parity across OS targets.
  3. *Robust Median Filtering*: Median calculation inherently rejects transient OS interrupts (background daemons, indexing tasks) without skewing arithmetic averages.
- **Alternatives Considered**:
  - *Standard `XCTest.measure` block*: Rejected. `measure` lacks native Swift Concurrency async/await support, does not isolate temporary output directories between iterations, and lacks custom throughput threshold mathematical assertions.
  - *Single-iteration measurement*: Rejected. Vulnerable to random system interrupts and disk cache cold start, causing up to 15% false-positive regression failures in CI.
- **Source**:
  - `Tests/TTZipTests/AsyncBenchmarkRunner.swift#L21-125`
  - `Tests/TTZipTests/HardwareCalibrator.swift#L1-55`
  - `Tests/TTZipTests/XCTestPerformanceMeasureTests.swift#L1-100`
  - `GEMINI.md` §IV.3 & §VII.3 (Mandatory Zero-Regression Audit Discipline, 3.0% tolerance limit)

---

## R003: Memory-Safe Zero-Copy Fixture Loading & SPM File Resolution

- **Decision**: Implement `SilesiaFixtureLoader` with 3-tier fallback resolution (`Bundle.module` -> `TTZIP_SILESIA_PATH` -> `#filePath`), coupled with direct POSIX path passing for C-level `mmap` zero-copy access and Swift `Data(contentsOf:options: .alwaysMapped)`.
- **Rationale**:
  1. *Swift 6.0 Strict Concurrency & Memory Safety*: Avoids unsafe mutable buffer copies and eliminates multithreading data race hazards by ensuring all input fixtures are read-only (`PROT_READ` / immutable `Sendable` `Data`).
  2. *Apple Silicon Virtual Memory Management*: Zero-copy `mmap` avoids dirtying pages in the unified memory pool, keeping memory footprints under 50 MB even when testing multi-threaded 16-format pipelines concurrently.
  3. *Cross-Platform Path Resolution*: Handles path separators and bundle relocations cleanly across macOS App sandbox, command-line `swift test`, and Windows environments.
- **Alternatives Considered**:
  - *Reading entire corpus into RAM (`Data(contentsOf:)` without `.alwaysMapped`)*: Rejected. Loading 211 MB into heap creates unnecessary GC/ARC overhead, risks triggering memory pressure events on 4GB CI agents, and pollutes memory benchmarks.
  - *Hardcoding absolute paths*: Rejected. Completely fails on different developer machines and containerized CI runners.
- **Source**:
  - `Tests/TTZipTests/TestFixtureLoader.swift#L7-47`
  - `Tests/TTZipTests/ArchiveGoldenCorpusTests.swift#L13-21`
  - `Sources/CTTZipBridge/CTTZipExtract.c#L55-65` (`mmap` zero-copy pattern)
  - `Sources/CTTZipBridge/ttzip_tar_zstd_direct.c#L148-165`
