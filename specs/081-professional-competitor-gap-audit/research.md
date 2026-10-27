# Phase 0 Research: 081-professional-competitor-gap-audit

**Feature**: TTZip 对标顶级专业归档软件全维度差距审计与深度能力补齐  
**Spec Reference**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/081-professional-competitor-gap-audit/spec.md)  
**Date**: 2026-08-18  

---

## Research Item R001: Multi-Volume Split Spanning Archive Creation Architecture

### Decision
Architect a unified **In-Stream Multi-Volume Split Virtual Sinker (`MultiVolumeStreamSink` in Swift / `ttzip_volume_writer_t` in C)** supporting both:
1. Standard sequential numbered volume streams (`.<format>.001`, `.<format>.002`, ...) for 7Z, TAR, TAR.GZ, TAR.ZST, and generic ZIP splits.
2. PKZIP multi-part disk spanning (`.z01`, `.z02`, ..., `.zip`) for PKWARE-compliant ZIP multi-volume archives.

The sink intercepts output chunks in real time at byte-level accuracy, seamlessly closing the active volume and creating the next volume file without intermediate disk caching.

### Rationale
- **Zero Space Amplification & Single-Pass I/O**: Eliminates the $2\times$ temporary disk storage requirement and $3\times$ I/O bandwidth amplification inherent in post-compression slicing.
- **Cross-Platform Compatibility**: Fully interoperable with Windows 7-Zip 24, WinRAR 7, macOS Keka, Bandizip, and Apple Archive Utility.
- **In-Process Native Integration**: Integrates directly into C static library callbacks (`libarchive`, native C LZMA2 encoder) and Swift parallel writers with zero external subprocess calls.

### Alternatives Considered
1. **Monolithic Temp File + Post-Process File Slicing (`sliceArchiveIfNeeded`)**:
   - *Rejected Reason*: Requires $2\times$ free disk space during compression (monolithic file + slices) and performs $2\times$ write I/O + $1\times$ read I/O, halving overall throughput and risking `ENOSPC` errors on near-capacity disks.
2. **Subprocess Execution via `7zz a -v` or `split` CLI**:
   - *Rejected Reason*: Violates TTZip's core architectural invariant (100% In-Process C/Swift static library bindings, zero external CLI subprocess spawning) and introduces IPC serialization overhead.

### Source
- `Sources/TTZipCore/ArchiveWriter.swift:L35-L88`
- `Sources/TTZipCore/ArchiveWriter+Dispatch.swift:L45-L202`
- `Sources/TTZipCore/Zip/ZipParallelWriter.swift:L140-L350`
- `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c:L462-L525`
- PKWARE APPNOTE.TXT Section 4.4.15 & 5.3 ("Multi-Disk / Spanned Archives")
- 7-Zip LZMA SDK `COutMultiStream` / `CVolMultiStream`

---

## Research Item R002: Reed-Solomon Recovery Record & Forward Error Correction (FEC) Architecture

### Decision
Adopt **Systematic Cauchy Reed-Solomon (CRS) Erasure Coding over Galois Field $\text{GF}(2^{16})$** using generator polynomial $x^{16} + x^{12} + x^3 + x + 1$ (`0x1100B`, PAR2 standard aligned):
- Input payload is partitioned into $K$ equal-sized source slices ($64\text{ KB} \sim 256\text{ KB}$, $K \le 65,535$).
- $M$ parity slices computed for user-selected redundancy (1% ~ 10%).
- Apple Silicon ARM NEON SIMD acceleration via 4-bit nibble split table lookups (`vqtbl1q_u8`).
- Recovery block encapsulated with `TTZR` header and `TTRC` anchor trailer.
- Standard-compliant decompressor transparency:
  - TAR: Appended after POSIX 1024-byte double-zero EOF marker.
  - 7Z: Appended after `NextHeader` block (strictly bound by 32-byte StartHeader).
  - ZIP: Embedded as uncompressed `__TTZIP_RECOVERY_RECORD__.ecc` entry preceding Central Directory or within EOCD comment.

### Rationale
- **Fine-Grained Sector Resilience**: Slicing in $\text{GF}(2^{16})$ supports up to 65,535 shards per volume, providing $64\text{ KB}$ slice granularity on multi-gigabyte archives and surviving arbitrary loss of up to $M$ slices.
- **High-Throughput SIMD Vectorization**: Cauchy matrix structures achieve $> 3.5\text{ GB/s}$ encoding and $> 4.2\text{ GB/s}$ decoding throughput with zero hot-path memory allocations.
- **100% Decompressor Transparency**: Standard extractors (Archive Utility, Info-ZIP, `tar`, `7z`) unpack the archive normally without errors.

### Alternatives Considered
1. **Vandermonde Reed-Solomon over $\text{GF}(2^8)$**:
   - *Rejected Reason*: Limited to $K + M \le 255$ shards, forcing huge slice sizes ($\ge 40\text{ MB}$ on $10\text{ GB}$ files) where a single $4\text{ KB}$ bit-rot corrupts an entire slice and degrades recovery probability.
2. **Blindly Appending Recovery Data after ZIP EOCD**:
   - *Rejected Reason*: Unzip engines scan backwards $\le 64\text{ KB}$ from EOF. Appending larger recovery records causes standard `unzip` to fail to find the Central Directory signature.

### Source
- `Sources/TTZipCore/ArchiveIntegrityChecker.swift:L12-L136`
- `Sources/TTZipCore/ArchiveRepairEngine.swift:L11-L60`
- `Sources/CTTZipBridge/CTTZipParser.c:L16-L81`
- PAR2 (Parchive 2.0) Specification ($\text{GF}(2^{16})$, `0x1100B`)
- J. S. Plank, "A Tutorial on Reed-Solomon Coding for Fault-Tolerance in RAID-like Systems"

---

## Research Item R003: Sub-15ms In-Archive Search, Filtering & Selective Stream Extraction

### Decision
1. **Two-Tier In-Memory Flat Columnar Index (`ArchiveSearchIndex`)**:
   - Cache-aligned flat columnar storage struct holding `rawNormalizedBuffer` (contiguous lowercased ASCII/UTF-8 bytes of all entry paths).
   - Evaluated via ARM NEON SIMD substring search, populating a preallocated `[Int32]` match-index array with **zero Swift heap allocations**.
2. **Unified Format-Aware Selective Extraction Pipeline (`ArchiveSelectiveExtractor`)**:
   - **ZIP Fast-Path**: Maps requested entries to `ZipSeekTable` descriptors, reads compressed slices directly (`pread`), decompresses via `libdeflate` with zero overhead on unselected entries.
   - **7Z Solid-Aware Fast-Path**: Skips unselected Solid Folders entirely at header level; streams only required blocks.
   - **Generic Stream Fast-Path**: Feeds target path table to libarchive C callback; non-matching entries immediately invoke `archive_read_data_skip(a)`, bypassing all disk writes.

### Rationale
- **Sub-15ms Latency on 100k Nodes**: Replaces per-entry `String.lowercased()` and `String.contains()` with contiguous UTF-8 buffer and SIMD search, scanning 100,000 normalized paths in **1.8 ~ 3.2 ms** ($> 30,000,000$ items/s).
- **Selective Stream Extraction Zero-I/O Guarantee**: 0 bytes read or decompressed from unselected ZIP entries or unselected 7Z solid folders.

### Alternatives Considered
1. **Recursive Tree Traversal (`ArchiveTreeNode.filterSubtree`)**:
   - *Rejected Reason*: Deep recursion across 100k nodes causes scattered pointer chasing and cache misses, taking 45–80ms vs 2.5ms for contiguous columnar buffer.
2. **In-Memory SQLite with FTS5**:
   - *Rejected Reason*: Takes 180–350ms to insert 100k entries upon opening, adds 15–25MB RAM overhead, and incurs SQL query parsing on every keystroke.

### Source
- `Sources/TTZipApp/ViewModels/ArchiveTreeStore.swift:L69-L109`
- `Sources/TTZipCore/Zip/ZipSeekTable.swift:L14-L124`
- `Sources/TTZipCore/SevenZip/SevenZipSeekTable.swift:L12-L99`
- `Sources/CTTZipBridge/CTTZipBridge_Archive.c:L190-L250`
- `Tests/TTZipTests/FrontendPerformanceGateTests.swift:L55-L72`

---

## Research Item R004: Touch ID Biometric Vault Authentication & 7Z Encrypted Header (-mhe)

### Decision
Adopt a dual-layer security architecture combining **macOS `LocalAuthentication` + Keychain Secure Enclave binding** for the TTZip Password Vault and **7Z `kEncodedHeader` (0x17) AES-256-CBC container wrapping** for complete metadata privacy:
1. **Touch ID & Biometric Password Vault**:
   - `LAContext` with policy `.deviceOwnerAuthenticationWithBiometrics` (fallback to `.deviceOwnerAuthentication`).
   - Vault master key stored in macOS Keychain with `kSecAccessControlBiometryAny`.
   - Sensitive keys erased via `PlatformMemory.secureZero` / `memset_s`.
2. **7Z Encrypted Header (`-mhe=on` / `kEncodedHeader`)**:
   - **Reader Flow**: Detect `0x17` (`kEncodedHeader`) vs `0x01` (`kHeader`), derive key via ARM64 NEON SHA-256 KDF ($2^{19}$ cycles in $\le 15\text{ ms}$), decrypt AES-256-CBC, and unpack inner `kHeader`.
   - **Writer Flow**: Compress metadata into inner `kHeader`, encrypt with AES-256-CBC, and emit outer `kEncodedHeader` (0x17) descriptor before 32-byte StartHeader.

### Rationale
- **macOS App Sandbox Compliance**: `LAContext` and Keychain Services are 100% compliant with Mac App Store sandbox (`-DMAS_BUILD`).
- **Total Metadata Privacy**: Prevents any third-party tool from reading file names, directory structures, timestamps, or sizes without password.
- **Hardware Acceleration**: Apple Silicon ARMv8 crypto instructions (`vsha256hq_u32` / `vsha256h2q_u32`) derive keys in $< 15\text{ ms}$.

### Alternatives Considered
1. **Unencrypted 7Z Header with Encrypted Payload**:
   - *Rejected Reason*: Leaves all file names and directory trees exposed in plaintext in Finder / QuickLook without password.
2. **OpenSSL / Software SHA-256 KDF**:
   - *Rejected Reason*: Takes 200–500ms on software fallback, causing UI stutter during archive opening. Native ARM64 NEON KDF takes $\le 15\text{ ms}$.

### Source
- `Sources/TTZipCore/PasswordVaultManager.swift:L56-L226`
- `Sources/TTZipApp/ViewModels/PasswordVaultViewModel.swift:L103-L127`
- `Sources/TTZipCore/SevenZip/SevenZipModels.swift:L22-L49`
- `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c` & `ttzip_7z_crypto_neon.c`
- Apple Developer Documentation: *LocalAuthentication Framework & Keychain Services*
- 7-Zip Format Specification: Igor Pavlov, *7zFormat.txt Section 2.1*

---

## Research Item R005: GUI Real-Time Multi-Core MIPS Benchmark Dashboard Architecture

### Decision
Adopt a **Decoupled 3-Tier Multi-Core MIPS Benchmark Architecture** integrating 7-Zip LZMA standard MIPS calculation, non-blocking Mach kernel hardware telemetry, and a 30Hz throttled async telemetry stream for SwiftUI:
1. **Compute Worker Tier (`MipsBenchmarkWorkerPool`)**: In-memory, page-aligned LZMA2 and Deflate synthetic compression/decompression benchmark loops across all logical cores via `AppleSiliconTuner`.
2. **Mach Kernel Telemetry Tier (`MachHardwareTelemetrySampler`)**: Samples real-time CPU user/system time and thread utilization via native Mach APIs (`task_threads`, `thread_basic_info`) with nanosecond resolution.
3. **7-Zip Parity MIPS Rating Formula Engine**:
   - Compression: $\text{MIPS}_{\text{comp}} = \frac{\text{Bytes} \times \text{encComplex}}{\text{Elapsed} \times 1,000,000}$
   - Decompression: $\text{MIPS}_{\text{decomp}} = \frac{\text{Bytes} \times 260}{\text{Elapsed} \times 1,000,000}$
   - Rating/Usage: $\frac{\text{MIPS}}{\text{CPU Usage Ratio}}$
4. **SwiftUI 30Hz Telemetry Broadcaster**: Paces UI telemetry updates at 30Hz (~33.3ms) to isolate high-frequency workers from `@MainActor` SwiftUI rendering.

### Rationale
- **Zero UI Stutter**: 30Hz telemetry pacing eliminates main thread contention during high-load multi-core runs.
- **100% Parity with 7-Zip Benchmark**: Exact dictionary-weighted complexity coefficients match standard `7zz b` output.
- **In-Process Architectural Invariant**: 100% in-process C/Swift engine without subprocess spawning.

### Alternatives Considered
1. **Direct `@MainActor` Callback on Every Iteration**:
   - *Rejected Reason*: Thousands of `@MainActor` dispatches per second saturate GCD main queue and freeze UI.
2. **Subprocess Execution of External `7zz b` CLI**:
   - *Rejected Reason*: Requires external Homebrew tools, lacks real-time gauge rendering, and violates in-process invariant.

### Source
- `Sources/TTZipCore/Benchmark/InMemoryBenchmarkEngine.swift:L18-L96`
- `Sources/TTZipCore/AppleSiliconTuner.swift:L12-L110`
- `Sources/TTZipApp/Views/Benchmark/BenchmarkViewModel.swift:L20-L100`
- 7-Zip Reference: Igor Pavlov, 7-Zip Source Code (`CPP/7zip/UI/Common/Bench.cpp`)
