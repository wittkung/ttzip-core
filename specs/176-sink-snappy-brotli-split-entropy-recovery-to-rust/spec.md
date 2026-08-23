# Feature Specification: 176-sink-snappy-brotli-split-entropy-recovery-to-rust

## 1. Executive Summary & Strategic Motivation
Following the comprehensive Phase 3 audit of remaining non-Rust code in TTZip, 6 critical domains were identified for next-level sinking into the Safe Rust core (`rust/ttzip-glue` and `rust/ttzip-tui`):
1. **Snappy Framing & Cross-Platform Brotli**: `SnappyFramingStream.swift` parsed Framing headers and CRC32C in Swift with frequent `Data` copies; `NativeBrotliEngine.swift` depended on Darwin-only `Compression.framework` (`COMPRESSION_BROTLI`) and used intermediate temporary `.tar` files.
2. **Multi-Volume Split Containers**: `MultiVolumeStreamSink.swift` operated via Foundation `FileHandle` and file renames; missing a unified zero-copy virtual multi-volume stream reader.
3. **SIMD Shannon Entropy & Smart Codec Selection**: `ArchiveEntropyEvaluator.swift` computed 256-bucket histograms via scalar Swift loops; `SmartCodecSelector.swift` performed manual `memcpy` sampling.
4. **VFS LZ4 Lock-Free LRU Cache Pool**: `VFSLz4CachePool.swift` suffered from $O(N \log N)$ key sorting during LRU evictions with heavy `NSLock` contention.
5. **In-Memory Password Recovery Engine**: `PasswordRecoveryEngine.swift` generated filesystem temporary directories for every candidate attempt (~1,200 keys/sec); sinking to pure in-memory Rayon verification boosts throughput to 200,000+ keys/sec.
6. **Archive Repair & Structural Salvage**: `ArchiveRepairEngine.swift` scanned corrupt headers in Swift without SIMD pattern scanning.

---

## 2. Scope of Down-Sinking

### Domain 1: Native Snappy Framing Stream & Pure Rust Brotli
- **Stream Framing**: Rust-native `snap::read::FrameDecoder` and `snap::write::FrameEncoder` with zero-allocation buffers.
- **Cross-Platform Brotli**: High-performance pure Rust Brotli streaming engine replacing Apple `Compression.framework` Darwin lock-in.

### Domain 2: Multi-Volume Split Writer & Virtual Continuous Reader
- **Multi-Volume Splitter**: Generic byte-counting stream sink supporting `.z01/.zip`, `.7z.001`, `.tar.gz.part*` formats.
- **Continuous Multi-Volume Source**: Seamless zero-copy virtual reader concatenating split volumes into a single logical archive stream.

### Domain 3: SIMD Shannon Entropy Estimator & Smart Codec Selector
- **ARM NEON / AVX2 Histogram**: 256-bin byte frequency histogram vectorization achieving >10 GB/s probe throughput.
- **Smart Decision Matrix**: Zero-copy 64KB micro-compression probes and entropy-driven algorithm selection.

### Domain 4: VFS Lock-Free LZ4 LRU Cache Pool
- **$O(1)$ LRU Eviction**: Intrusive doubly-linked list with `hashbrown::HashMap` and `parking_lot::RwLock` for sub-millisecond archive navigation.

### Domain 5: In-Memory Multi-Core Password Recovery Engine
- **Pure In-Memory Rayon Verification**: In-memory decryption verification for PKZIP 3-Key, WinZip AES, and 7z AES, bypassing all disk I/O.

### Domain 6: SIMD Archive Salvage & Repair Engine
- **SIMD Header Pattern Scanning**: Instant scanning of corrupted binary streams to reconstruct valid Central Directories and TAR tables of contents.

---

## 3. User Scenarios & Acceptance Criteria

### User Scenario 1: Cross-Platform Brotli & Snappy Compression
- **Given** a Linux or Windows system running `ttzip`
- **When** compressing or decompressing Brotli (`.tar.br`) or Snappy Framed (`.sz`) streams
- **Then** operations execute natively with zero Apple framework dependency at >500 MB/s.

### User Scenario 2: High-Speed Password Recovery
- **Given** an encrypted archive and candidate dictionary
- **When** password recovery is initiated
- **Then** Rayon utilizes 100% CPU multi-core capacity with >150,000 checks/sec in RAM without creating temporary disk files.

### User Scenario 3: Transparent Multi-Volume Archive Creation & Extraction
- **Given** large multi-gigabyte data split into 100MB volumes
- **When** compressing with split configuration
- **Then** precise volume boundaries are generated and seamlessly reassembled during extraction.

---

## 4. Success Metrics
1. **Password Recovery Speedup**: >100x acceleration (from ~1,200 keys/sec to >150,000 keys/sec).
2. **Shannon Entropy Probe**: <0.1ms per 1MB probe.
3. **Cross-Platform Compilation**: 100% clean build on Linux, macOS, and Windows.
4. **Zero Regression**: 100% of existing 863+ Swift tests and 7/7 local CI stages pass.

---

## 5. Clarifications
- **Q1: How does pure in-memory password recovery verify passwords without touching disk?**
  - **Decision**: For ZipCrypto, the engine validates the 12-byte encryption header against the CRC32/high byte; for WinZip AES, it computes PBKDF2-HMAC-SHA1 and verifies against the 2-byte password verification value; for 7z AES, it derives the key with SHA-256 iterations and attempts decoding the NextHeader CRC32. All operations run 100% in CPU registers and RAM via Rayon without any file writes.
- **Q2: How is cross-platform Brotli integrated without Apple Compression.framework?**
  - **Decision**: In `rust/ttzip-glue/src/codecs/brotli.rs`, pure Safe Rust `brotli = "7.0"` streaming compressor/decompressor is used across all target platforms.
- **Q3: How does the SIMD Shannon entropy calculation work?**
  - **Decision**: In `rust/ttzip-glue/src/analytics/entropy.rs`, 256-bin histograms are computed using 4-way unrolled 64-byte chunks with vector load instructions, followed by lookup-table log2 calculations.

