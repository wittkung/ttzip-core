# Tasks: 7z Solid 解压与 ARM64 密码加速 (Feature 167)

**Feature ID**: `167-7z-solid-stream-decoder-and-aes256-neon-acceleration`  
**Created**: 2026-08-21  
**Status**: Ready for Implementation  

---

## Phase 1: Setup & Crypto Headers

- [x] T001 Update `Sources/CTTZipBridge/include/ttzip_7z_crypto_neon.h` and `Sources/CTTZipBridge/include/ttzip_7z_kdf_arm64.h` with enhanced 8-way AES and hardware KDF prototypes
- [x] T002 Update `Sources/CTTZipBridge/include/ttzip_7z_block_decoder.h` declaring `ttzip_7z_decode_solid_entry_stream`

---

## Phase 2: User Story 1 (P1) - ARM64 ACLE 8-Way AES-256-CBC Decryption

- [x] T003 [P] [US1] Implement precomputed inverse decryption round keys in `Sources/CTTZipBridge/ttzip_7z_crypto_neon.c`
- [x] T004 [P] [US1] Implement 8-way unrolled vector loop (`vaesdq_u8` + `vaesimcq_u8`) with 128-byte cache line alignment in `Sources/CTTZipBridge/ttzip_7z_crypto_neon.c`

---

## Phase 3: User Story 2 (P2) - ARM64 Hardware SHA-256 KDF Engine

- [x] T005 [P] [US2] Implement in-register hardware SHA-256 block compressor using `vsha256h/su` in `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c`
- [x] T006 [P] [US2] Implement loop invariant hoisting and memory sanitization in `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c`

---

## Phase 4: User Story 3 (P3) - 7z Solid Stream Selective Decoder

- [x] T007 [P] [US3] Implement 3-phase streaming execution (`discard -> extract -> early termination`) in `Sources/CTTZipBridge/ttzip_7z_block_decoder.c`
- [x] T008 [P] [US3] Wire selective extraction in `Sources/CTTZipBridge/CTTZipBridge_7zSolid.c`

---

## Phase 5: Verification & Statistical Worktree A/B Gate

- [x] T009 [US1] Create `Tests/c/test_7z_crypto_neon.c` with NIST AES-256 test vectors and KDF checks
- [x] T010 [US1] Register `7z_crypto_neon` in `CMakeLists.txt` and `Tests/c/test_main.c`
- [x] T011 [US1] Run `./build/ttzip_c_test_runner all` (all 26 test suites pass)
- [x] T012 [US1] Run `./scripts/benchmark_ab.sh HEAD WIP --runs 5` and verify `PASSED_NO_REGRESSION`
