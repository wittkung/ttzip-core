# Implementation Plan: 7z Solid 解压与 ARM64 密码加速 (Feature 167)

**Feature ID**: `167-7z-solid-stream-decoder-and-aes256-neon-acceleration`  
**Created**: 2026-08-21  
**Status**: Ready for Tasks  

---

## 1. Technical Context & Constitution Check

### 1.1 Technical Context
- **Toolchain**: Apple Clang ARM64 (`-O3 -march=armv8-a+crypto`), C11 (`stdatomic.h`, `<arm_neon.h>`), CMake 3.20+.
- **Core Cryptographic & Microarchitectural Invariants**:
  - Precompute 15 inverse decryption keys for 14-round AES-256-CBC.
  - 8-way parallel ACLE unrolled vector decryption kernel (`vaesdq_u8` + `vaesimcq_u8`).
  - In-register hardware SHA-256 block compressor for 7z KDF loop.
  - 3-phase Solid stream extraction (`discard -> extract -> early termination`).
  - Zero heap allocation churn in inner loops, `explicit_bzero` memory sanitization.

### 1.2 Constitution Check
- [x] **Zero Cloud Quota**: 100% local Apple Silicon execution.
- [x] **Algorithm Non-Invention**: Standard FIPS 197 AES-256 and FIPS 180-4 SHA-256.
- [x] **Safe Fallback**: Pure C scalar fallback on non-ARM64 architectures.
- [x] **Zero Bare Objects & Schema Strictness**: JSON schema contract in `contracts/7z-crypto-solid-schema.json`.

---

## 2. Phase 0 & Phase 1 Artifacts Index

- [x] **Phase 0 Research**: [`research.md`](research.md)
  - `- R001 [SUBAGENT:research] 《ARM64 ACLE Crypto AES-256-CBC 8-Way 向量解密流水线》`
  - `- R002 [SUBAGENT:research] 《ARM64 硬件 SHA-256 KDF 寄存器级密钥派生加速》`
  - `- R003 [SUBAGENT:research] 《7z Solid 块流式跳过与单条目提取契约》`
- [x] **Phase 1 Data Model**: [`data-model.md`](data-model.md)
- [x] **Phase 1 Contract**: [`contracts/7z-crypto-solid-schema.json`](contracts/7z-crypto-solid-schema.json)
- [x] **Phase 1 Quickstart**: [`quickstart.md`](quickstart.md)

---

## 3. Component Breakdown & Planned Changes

### Component 1: Hardware AES-256 ACLE Engine (`ttzip_7z_crypto_neon.c`)
- [MODIFY] `Sources/CTTZipBridge/include/ttzip_7z_crypto_neon.h`: Add 8-way unrolled decrypt signatures.
- [MODIFY] `Sources/CTTZipBridge/ttzip_7z_crypto_neon.c`: Implement 8-way `vaesdq_u8` decryption pipeline with precomputed inverse round keys.

### Component 2: Hardware SHA-256 KDF Engine (`ttzip_7z_kdf_arm64.c`)
- [MODIFY] `Sources/CTTZipBridge/include/ttzip_7z_kdf_arm64.h`: Declare hardware-accelerated KDF function.
- [MODIFY] `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c`: Implement in-register hardware block transform.

### Component 3: 7z Solid Stream Selective Extraction (`ttzip_7z_block_decoder.c` & `CTTZipBridge_7zSolid.c`)
- [MODIFY] `Sources/CTTZipBridge/include/ttzip_7z_block_decoder.h`: Add `ttzip_7z_decode_solid_entry_stream`.
- [MODIFY] `Sources/CTTZipBridge/ttzip_7z_block_decoder.c`: Implement 3-phase stream extractor.
- [MODIFY] `Sources/CTTZipBridge/CTTZipBridge_7zSolid.c`: Wire single entry streaming API.

### Component 4: C11 Unit Tests & A/B Verification
- [NEW] `Tests/c/test_7z_crypto_neon.c`: NIST test vectors and 7z encrypted block tests.
- [MODIFY] `CMakeLists.txt` and `Tests/c/test_main.c`: Register `7z_crypto_neon` suite.

---

## 4. Verification Plan

1. **Unit Test Execution**:
   - `cmake --build build --target ttzip_c_test_runner && ./build/ttzip_c_test_runner 7z_crypto_neon`
2. **All 26 Test Suites**:
   - `./build/ttzip_c_test_runner all`
3. **Statistical A/B Validation**:
   - `./scripts/benchmark_ab.sh HEAD WIP --runs 5`
