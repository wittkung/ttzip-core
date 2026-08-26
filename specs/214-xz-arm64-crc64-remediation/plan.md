# Implementation Plan: XZ PR #241 ARM64 CRC64 PMULL Remediation

**Feature**: `specs/214-xz-arm64-crc64-remediation`  
**Pipeline Level**: `[Full SDD]`  
**Target Repository**: `Vendor/worktrees/xz/pr2-arm64-crc64`

---

## Technical Context

- **Build Systems**: CMake (`CMakeLists.txt`) & GNU Autotools (`configure.ac`, `src/liblzma/check/Makefile.inc`).
- **Core Files**:
  - `src/liblzma/check/crc_common.h`
  - `src/liblzma/check/crc64_arm64.h`
  - `src/liblzma/check/crc64_fast.c`
- **Compiler Requirements**: GCC / Clang with `__attribute__((__target__("+crypto")))` or `-march=armv8-a+crypto`.

---

## Proposed Changes

### Phase 1: Build System Feature Probes & Options

1. **`CMakeLists.txt`**:
   - Add `XZ_ARM64_CRC64` option (default ON).
   - Probe `vmull_p64` compiling with `+crypto` target attribute via `check_c_source_compiles`.
   - On success, define `HAVE_ARM64_CRC64`.
   - Probe `HWCAP_PMULL` in `<sys/auxv.h>` via `check_symbol_exists` and add definition `HAVE_HWCAP_PMULL`.
2. **`configure.ac`**:
   - Add `--disable-arm64-crc64` argument.
   - Probe `vmull_p64` compiling with `+crypto` target attribute via `AC_LINK_IFELSE`.
   - On success, define `HAVE_ARM64_CRC64`.
   - Check declaration of `HWCAP_PMULL` in `<sys/auxv.h>` via `AC_CHECK_DECL` and define `HAVE_HWCAP_PMULL`.

### Phase 2: Macro Decoupling & Defensive Fallback

1. **`src/liblzma/check/crc_common.h`**:
   - Decouple `CRC_ARM64_RUNTIME_DETECTION` into `CRC32_ARM64_RUNTIME_DETECTION` (requiring `HAVE_HWCAP_CRC32` on Linux) and `CRC64_ARM64_RUNTIME_DETECTION` (requiring `HAVE_HWCAP_PMULL` on Linux).
   - Protect `CRC64_ARM64` activation with `HAVE_ARM64_CRC64` and `!defined(WORDS_BIGENDIAN)`.

### Phase 3: Attribute Hardening & Header Hygiene

1. **`src/liblzma/check/crc64_arm64.h`**:
   - Add `crc64_attr_target` to `keep_high_bytes`, `shift_left`, `shift_right`, `clmul_00`, `clmul_10`, `clmul_11`, `fold`, `fold_xor`.
   - Ensure `is_arch_extension_supported` matches upstream standards for Linux, FreeBSD, Darwin, and Windows.

---

## Verification Plan

### Automated Verification
- `cmake -B build-cmake -DXZ_ARM64_CRC64=ON && cmake --build build-cmake && ctest --test-dir build-cmake`
- `./autogen.sh && ./configure && make check`
- Mathematical verification: `build-cmake/tests/test_check`
