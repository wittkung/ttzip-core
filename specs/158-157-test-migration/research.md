# Research & Technical Decisions: Full 22-File Swift Test Migration to C11

**Feature**: `158-157-test-migration`  
**Date**: 2026-08-20  
**Status**: Completed  

---

## 1. Five Microkernel Test Clusters & Target C Architectures

### Cluster 1: Hardware Checksums & Vector Oracles (`test_adler_crc64.c`)
- **Migrated Swift Files (2)**: `HardwareChecksumTests.swift`, `CRC64HardwareTests.swift`
- **Target C Subsystem**: `CTTZipChecksum.h`, `CTTZipAdler32Neon.h`, `ttzip_crc64.h`
- **Decision**: Implement `tests/c/test_adler_crc64.c` testing:
  - Adler-32 RFC 1950 golden vectors, NMAX=5552 modulo boundary rollover.
  - CRC64-XZ standard vectors (`0x6c40df5f0b497347` for `"123456789"`), 0..15 byte misalignment matrices.
- **Rationale**: Direct vector intrinsics testing in C prevents Swift ARC and copy overhead, ensuring sub-100µs verification.
- **Alternatives Considered**: Keep Swift wrapper oracles (rejected due to slow differential loops).
- **Source**: `Sources/CTTZipBridge/include/CTTZipChecksum.h`, `Tests/TTZipTests/HardwareChecksumTests.swift`.

---

### Cluster 2: Shannon Entropy & Dynamic Chunking (`test_entropy_evaluator.c`)
- **Migrated Swift Files (3)**: `ArchiveEntropyEvaluatorTests.swift`, `EntropyAdaptiveExtremeRoutingTests.swift`, `EntropyTieredChunkingEngineTests.swift`
- **Target C Subsystem**: `CTTZipBridge.h` (`ttzip_estimate_buffer_entropy`), `CTTZipQuantumPipeline.h` (`ttzip_quantum_calc_entropy_neon`), `CTTZipStreamCoder.h` (`ttzip_probe_entropy_and_compressibility`, `ttzip_calculate_adaptive_block_size`)
- **Decision**: Implement `tests/c/test_entropy_evaluator.c` testing:
  - 8.0-scale Shannon entropy on uniform ASCII, low-entropy zero-fill, and high-entropy /dev/urandom bytes.
  - Dynamic chunk sizing: entropy > 7.65 $\rightarrow$ bypass store mode; entropy < 3.0 $\rightarrow$ large 1MB block mode.
- **Rationale**: Mathematical validation of entropy is purely numerical and belongs in C11.
- **Source**: `Sources/CTTZipBridge/include/CTTZipBridge.h`, `Tests/TTZipTests/ArchiveEntropyEvaluatorTests.swift`.

---

### Cluster 3: Match Finders & Deflate Pipelines (`test_matchfinder_advanced.c`)
- **Migrated Swift Files (4)**: `FastMatchFinderTests.swift`, `HuffmanBitstreamOptimizationTests.swift`, `AdaptiveBlockSplitTests.swift`, `CrossBlockDeflateDictionaryTests.swift`
- **Target C Subsystem**: `ttzip_lzma_hc4_neon.h`, `ttzip_adaptive_block_split.h`, `ttzip_ring_dict.h`
- **Decision**: Implement `tests/c/test_matchfinder_advanced.c` testing:
  - Hash chain collision resolution and match finding across 32KB windows.
  - Canonical Huffman bitstream packing without byte boundary leakage.
  - 32KB cross-block history preconditioning for multi-threaded Deflate.
- **Rationale**: Ring dictionary memory movement is zero-heap and should be checked with ASan.
- **Source**: `Sources/CTTZipBridge/include/ttzip_lzma_hc4_neon.h`, `Tests/TTZipTests/FastMatchFinderTests.swift`.

---

### Cluster 4: Blosc2 Slicing, SuperChunk & Plugin Registry (`test_blosc_slicing.c`)
- **Migrated Swift Files (8)**: `Blosc2LazySlicingTests.swift`, `Blosc2SpecialValueTests.swift`, `Blosc2SuperChunkTests.swift`, `Blosc2PluginRegistryTests.swift`, `Blosc2AdvancedArchitecturesTests.swift`, `Blosc2ArchitectureAbsorptionTests.swift`, `Blosc2HeuristicTunerTests.swift`, `Blosc2ExhaustiveComparativeTests.swift`
- **Target C Subsystem**: `CTTZipBitGroom.h`, `CTTZipSuperChunk.h`, `ttzip_blosclz.h`
- **Decision**: Implement `tests/c/test_blosc_slicing.c` testing:
  - Sub-chunk slice extraction (`ttzip_schunk_get_slice_buffer`).
  - Constant run-length chunk tagging with `(1ULL << 63)` MSB tag.
  - Multi-threaded SuperChunk dictionary training and appending.
- **Rationale**: Slicing structures are native C memory layouts.
- **Source**: `Sources/CTTZipBridge/include/CTTZipSuperChunk.h`, `Tests/TTZipTests/Blosc2SuperChunkTests.swift`.

---

### Cluster 5: 7z KDF Crypto, LZ4 Native VFS & Fuzzing (`test_crypto_lz4_snappy.c`)
- **Migrated Swift Files (5)**: `Libarchive7zEncryptionTests.swift`, `SnappySecurityAndFuzzingTests.swift`, `LZ4DeepIntegrationAndVFSTests.swift`, `NativeDeflateEngineTests.swift`, `ChunkedDeflateStreamingTests.swift`
- **Target C Subsystem**: `ttzip_7z_kdf_arm64.h`, `CTTZipBridge_Snappy.h`, `CTTZipBridge_ZipChunkedStream.h`, `lz4.h`
- **Decision**: Implement `tests/c/test_crypto_lz4_snappy.c` testing:
  - ARMv8 SHA-256 KDF key derivation cycles and session management.
  - Snappy security fuzzing (truncated varints, invalid chunk types, buffer overrun protection).
  - Chunked deflate stream buffering.
- **Rationale**: Validates defensive bounds and memory safety directly under AddressSanitizer.
- **Source**: `Sources/CTTZipBridge/include/ttzip_7z_kdf_arm64.h`, `Tests/TTZipTests/SnappySecurityAndFuzzingTests.swift`.
