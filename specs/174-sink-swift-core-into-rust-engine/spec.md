# Feature Specification: 174-sink-swift-core-into-rust-engine

## 1. Overview & Strategic Motivation
TTZip's long-term architectural goal is to **minimize Swift code to only what is strictly necessary for macOS native UI (SwiftUI) and OS integration (FinderSync, Keychain, QuickLook)**, while completely sinking all computation-heavy, stream processing, archive format parsing, cryptographic, error-correction, standards compliance, benchmarking, and differential testing logic down into a **high-performance, memory-safe, cross-platform Safe Rust core (`ttzip-glue` and `ttzip-tui`)**.

Following a comprehensive audit of all remaining non-Rust code (~40,000 LOC of Swift logic in `Sources/TTZipCore`, `Sources/TTZipCLI`, `Sources/TTZipBench`), this feature systematically implements the migration and sinking of these components into the Rust engine.

---

## 2. Scope of Down-Sinking to Rust

### Category 1: Archive Format Parsing & Container Packaging
- **ZIP Parser & Builder**: Central Directory, Zip64 Locator/EOCD, Local File Headers, ZipExtraField TLV parsing (`0x5455`, `0x7075`, `0x0001`, `0x9901`).
- **TAR Parser & Builder**: POSIX.1 ustar, GNU Tar, Pax extended header, octal checksum calculation, dual 512-byte zero block trailing.
- **7z & Stream Containers**: 7z SignatureHeader, Varint codec, Solid block seek tables, ISO 9660 Sector 16 volume descriptors, DMG UDIF koly trailers, MS-WIM headers.

### Category 2: Core Streaming Pipelines & Multi-Threaded Chunking
- **Streaming Adapters**: Unified `Read`/`Write`/`Seek` streaming state machines in Rust with zero-copy buffer handoff.
- **Parallel Chunked Pipelines**: Rayon-based multi-threaded block compression with work-stealing schedulers replacing Darwin GCD queues.
- **Codecs Unification**: Deflate (libdeflate), Zstandard (zstd), Fast-LZMA2 (FL2), LZ4, Snappy, LZFSE, Brotli, and Bzip2 native Rust integrations.

### Category 3: Checksums, Hashes & Cryptography
- **Hardware & SIMD Checksums**: Rayon parallel CRC32, CRC64, Adler32, and SHA-256 reductions.
- **Stream & Container Crypto**: PKZIP 3-Key Stream Cipher, WinZip AES-CTR/CBC, 7z SHA-256 KDF with automated memory zeroization (`zeroize::ZeroizeOnDrop`).
- **Error Correction & Self-Healing**: Galois Field $GF(2^8)$ Cauchy Reed-Solomon Forward Error Correction (FEC) and recovery record generation.

### Category 4: Standards Compliance & Magic Signature Sniffing
- **Format Magic Sniffer**: 16+ archive format magic detectors supporting Head, Tail, Sector (2048B), and TarOffset heuristics.
- **Standards Compliance Verifier**: Strict compliance rules for PKWARE APPNOTE, POSIX.1, RFC 1952, RFC 8878, 7-Zip, ISO 9660, and MS-WIM.

### Category 5: Differential Oracles, Fuzzing & Benchmarking
- **Differential Oracle Test Harness**: Cross-process test runners comparing TTZip against system utilities (`unzip`, `tar`, `7z`, `zstd`, `xz`).
- **Malformed Stream Fuzz Engine**: SplitMix64 PRNG-driven mutation operators with crash dump capture.
- **In-Memory & Pareto Benchmarking**: Zero-I/O memory throughput benchmarking, 7-Zip MIPS hardware scoring, and multi-objective Pareto frontier calculation.

### Category 6: Standalone Cross-Platform CLI & TUI
- **Unified `ttzip` Binary**: Full command dispatch (`create`, `extract`, `list`, `test`, `bench`, `verify`, `fuzz`, `tui`) running standalone on macOS, Linux, and Windows.

---

## 3. User Scenarios & Acceptance Criteria

### User Scenario 1: Standalone Cross-Platform CLI Execution
- **Given** a Linux (x86_64 / aarch64) or macOS system without Swift runtime installed
- **When** the user executes `ttzip` CLI commands (`ttzip create`, `ttzip extract`, `ttzip list`, `ttzip bench`, `ttzip verify`)
- **Then** all archive operations execute with 100% functionality, full multi-core acceleration, and zero Swift runtime dependency.

### User Scenario 2: Zero-Copy Maximum Throughput & Memory Safety
- **Given** large multi-gigabyte files or archives with thousands of entries
- **When** compression, extraction, or hash verification is performed
- **Then** memory usage remains bounded (<= 64MB streaming resident set size), Rayon utilizes 100% CPU multi-core efficiency, and all crypto keys are zeroized immediately on drop.

### User Scenario 3: Thin Swift UI Layer with Zero Regression
- **Given** the macOS native SwiftUI app and existing test suites
- **When** the app runs operations through the thin C-ABI bridge
- **Then** 100% of existing 860+ tests and 7/7 local CI stages pass with 0 warnings and 0 failures.

---

## 4. Success Metrics
1. **Rust Engine Self-Sufficiency**: All core parsing, compression, cryptography, and standards compliance run natively in Rust.
2. **Swift Layer Thinning**: Swift code reduced by >50%, focusing solely on SwiftUI views, state models, and macOS integration.
3. **Cross-Platform Compilation**: `rust/` workspace compiles cleanly on Linux and macOS targets (`aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-musl`).
4. **Zero Regression Gate**: 100% of unit tests, property tests, fuzz harnesses, and local CI stages pass with 0 warnings.

---

## 5. Clarifications
- **Q1: How will the thin Swift layer interact with the Rust engine after down-sinking?**
  - **Decision**: Swift calls high-level C-ABI functions exported by `ttzip-glue` (`ttzip_rust_*`) through the minimal bridge `Sources/CTTZipBridge/include/ttzip_rust_glue.h`. Swift wraps these in clean `async/await` and `ThrowingStream` APIs.
- **Q2: How is cross-platform build orchestration managed?**
  - **Decision**: The Rust workspace (`rust/`) is completely self-contained with its own `Cargo.toml`. On Linux/Windows, `cargo build --release` produces the standalone `ttzip` binary directly without requiring Xcode, SPM, or Swift toolchains. On macOS, SPM continues to link `libTTZipVendor.a` into `TTZipApp`.
- **Q3: What happens to existing Swift tests when their underlying logic moves to Rust?**
  - **Decision**: Swift tests continue to test the public Swift facade/adapters (verifying end-to-end integration and zero regression), while comprehensive new native Rust property tests (`proptest`), unit tests, and fuzzing harnesses are added to `rust/ttzip-glue` and `rust/ttzip-tui`.

