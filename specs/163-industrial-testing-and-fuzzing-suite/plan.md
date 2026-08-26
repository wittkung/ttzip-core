# Implementation Plan: 工业级极端边界、安全漏洞与元数据测试体系 (Feature 163)

**Feature ID**: `163-industrial-testing-and-fuzzing-suite`  
**Created**: 2026-08-20  
**Status**: Ready for Tasks  

---

## 1. Technical Context & Constitution Check

### 1.1 Technical Context
- **Target Architecture**: Apple Silicon ARM64, macOS APFS, Apple Clang C11 with AddressSanitizer & UndefinedBehaviorSanitizer.
- **Core Focus**:
  - CVE & Malformed Bitstream Defense (`CVE-2002-0059`, `CVE-2005-1849`, `CVE-2018-25032`, `GH-382`, `CVE-2022-37434`).
  - Backward Compatibility with 1990-2000s Legacy Tools (PKZIP 2.04g, 7-Zip 4.20, GNU Tar longlink).
  - macOS APFS `xattr` (quarantine) and 1GB sparse file physical hole preservation.
  - In-Memory Clang LibFuzzer zero-allocation harness (`LLVMFuzzerTestOneInput`).

### 1.2 Constitution Check
- [x] **Zero Cloud Quota / 100% Local**: All test runners and fuzzer harnesses execute completely offline.
- [x] **Strict Native Library Dominance**: Focus 100% on C glue layers, boundary checks, metadata restoration, and fuzzing harnesses without modifying core third-party codec implementations.
- [x] **Zero Bare Objects & Schema Strictness**: JSON telemetry contract (`contracts/security-fuzz-schema.json`) enforces strict draft-07 types.
- [x] **Zero-Regression 5-Gate Pipeline**: All new safety suites integrated into Gate 1 of `scripts/run_optimization_gate.sh` with total run time kept under 5 seconds.

---

## 2. Phase 0 & Phase 1 Artifacts Index

- [x] **Phase 0 Research**: [`research.md`](research.md)
  - `- R001 [SUBAGENT:research] 《zlib-ng 历史 CVE 与畸变包防御架构》`
  - `- R002 [SUBAGENT:research] 《libarchive 跨年代归档格式向后兼容性测试》`
  - `- R003 [SUBAGENT:research] 《macOS APFS 扩展属性 (xattr) 与 1GB 稀疏空洞文件往返》`
  - `- R004 [SUBAGENT:research] 《LLVM LibFuzzer Harness 与格式字典架构》`
- [x] **Phase 1 Data Model**: [`data-model.md`](data-model.md)
- [x] **Phase 1 Contract**: [`contracts/security-fuzz-schema.json`](contracts/security-fuzz-schema.json)
- [x] **Phase 1 Quickstart**: [`quickstart.md`](quickstart.md)

---

## 3. Component Breakdown & Planned Changes

### Component 1: CVE Defense & Malformed Bitstream Suite (`tests/`)
- [NEW] `tests/fixtures/cve/`: Curated directory with malformed binary fixtures.
- [NEW] `tests/c/test_cve_regressions.c`: C11 test suite verifying graceful rejection of CVE-2002-0059, CVE-2005-1849, CVE-2018-25032, GH-382, and synthetic Huffman corruptions.

### Component 2: Historical Archive Backward Compatibility Suite (`tests/`)
- [NEW] `tests/fixtures/compat/`: Curated legacy archive test files (PKZIP, 7z legacy, GNU Tar longlink).
- [NEW] `tests/c/test_compat_archives.c`: C11 test suite verifying 100% extraction fidelity on non-standard archives.

### Component 3: macOS APFS Extended Attributes & Sparse Files (`tests/`)
- [NEW] `tests/c/test_fs_metadata.c`: C11 test suite testing `com.apple.quarantine`, custom xattrs, and 1GB APFS sparse hole roundtrips.

### Component 4: Clang LibFuzzer Harness & Dictionary (`tests/fuzz/`)
- [NEW] `tests/fuzz/fuzz_extract_engine.c`: In-memory `LLVMFuzzerTestOneInput` harness.
- [NEW] `tests/fuzz/ttzip_archive.dict`: Comprehensive token dictionary for ZIP/TAR/GZ/ZSTD/7Z.

### Component 5: Build System & Gate Integration
- [MODIFY] `CMakeLists.txt`: Register new test suites in `ttzip_c_test_runner` and add `ttzip_fuzzer` target.
- [MODIFY] `tests/c/test_main.c`: Add `cve_regressions`, `compat_archives`, and `fs_metadata` suite runners.
- [MODIFY] `scripts/run_optimization_gate.sh`: Verify all new suites pass within 5s gate.

---

## 4. Verification Plan

1. **Unit Test Suite**:
   - `cmake --build build && ./build/ttzip_c_test_runner all` (must execute all 24 suites in <100ms).
2. **Fuzzer Verification**:
   - `cmake --build build --target ttzip_fuzzer && ./build/ttzip_fuzzer -max_total_time=3 tests/fixtures/cve/`
3. **5-Gate Optimization Pipeline**:
   - `./scripts/run_optimization_gate.sh --bail --json build/gate_report.json`
