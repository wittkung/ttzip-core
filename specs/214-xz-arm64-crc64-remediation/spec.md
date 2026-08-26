# Feature Specification: XZ PR #241 ARM64 CRC64 PMULL Upstream Remediation & Build System Hardening

**Feature Directory**: `specs/214-xz-arm64-crc64-remediation`  
**Pipeline Level**: `[Full SDD]`  
**Created**: 2026-08-23  
**Status**: Draft  
**Target Repository**: `tukaani-project/xz` (Worktree: `Vendor/worktrees/xz/pr2-arm64-crc64`)

---

## Motivation & Background

XZ Utils PR #241 implements hardware-accelerated CRC64 on ARM64 processors using the polynomial multiplication instruction (`vmull_p64` / PMULL). While the mathematical core (polynomial folding over $\text{GF}(2)[x]$, 4-way vector unrolling, and Barrett reduction) is validated and achieves 47 GB/s on Apple Silicon, a comprehensive audit revealed critical build-system and compiler-attribute defects that prevent clean upstream merging:

1. **Linux ARM64 Build Interruption**: On generic Linux AArch64 systems without `-march=+crypto`, runtime detection is enabled via `HAVE_HWCAP_CRC32`, but `is_arch_extension_supported()` checks `HAVE_HWCAP_PMULL`, which is never probed or defined in `configure.ac` or `CMakeLists.txt`. This triggers `#error Runtime detection method unavailable.` during compilation.
2. **Missing Feature Detection Gates**: Unlike CRC32 which has explicit compiler probes (`HAVE_ARM64_CRC32`) and configuration flags (`--disable-arm64-crc32` / `XZ_ARM64_CRC32`), CRC64 PMULL has no compiler capability checks in Autotools or CMake.
3. **Compiler Target Attribute Inconsistencies**: Helper `static inline` functions in `crc64_arm64.h` lack `crc64_attr_target`, risking GCC target option mismatch failures during inlining under non-crypto base architectures.

This specification defines the complete remediation required to make PR #241 100% production-ready for upstream merge.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Linux ARM64 Clean Compilation & Runtime Fallback (Priority: P1)

As a Linux package maintainer building XZ on AArch64 (e.g. Debian, Fedora, Alpine), I need the build system to correctly detect `HWCAP_PMULL` in `<sys/auxv.h>` and decouple CRC64 runtime detection from CRC32, so that binaries compile cleanly without `#error` and dynamically select PMULL when available on the host processor or fall back to generic slice-by-4 when absent.

**Why this priority**: Resolves the highest-severity blocker that breaks build on standard Linux ARM64 distributions.

**Independent Test**:
- Compile with CMake and Autotools on Linux AArch64 environments (and under simulated non-crypto targets).
- Verify successful build and runtime resolution to `crc64_arch_optimized` on PMULL-capable hardware and `lzma_crc64_generic` on non-PMULL hardware.

**Acceptance Scenarios**:
1. **Given** a Linux AArch64 system with `getauxval()` and `HWCAP_PMULL`, **When** configuring with CMake or Autotools, **Then** `HAVE_HWCAP_PMULL` is defined and `is_arch_extension_supported()` returns `true`.
2. **Given** an ARM64 environment where `HWCAP_PMULL` is unavailable in system headers, **When** compiling `liblzma`, **Then** `CRC64_ARM64_RUNTIME_DETECTION` is disabled and the build falls back gracefully to `CRC64_GENERIC` without compile errors.

---

### User Story 2 - Dual Build System Feature Probes (CMake & Autotools) (Priority: P1)

As an XZ developer building with either CMake or GNU Autotools, I need standard `--disable-arm64-crc64` (Autotools) and `XZ_ARM64_CRC64` (CMake) options, along with automated compiler capability probing for `vmull_p64` with `__attribute__((__target__("+crypto")))`, ensuring full cross-build-system feature parity with existing CRC32 options.

**Why this priority**: Conforms strictly to XZ project conventions established by lead maintainer Lasse Collin for optional architecture-specific accelerations.

**Independent Test**:
- Configure with `--disable-arm64-crc64` / `-DXZ_ARM64_CRC64=OFF` and verify that no ARM64 CRC64 symbols or headers are activated.
- Configure on a toolchain supporting `vmull_p64` and verify `HAVE_ARM64_CRC64` is defined.

**Acceptance Scenarios**:
1. **Given** `CMakeLists.txt`, **When** configuring with `-DXZ_ARM64_CRC64=ON`, **Then** `check_c_source_compiles` tests `vmull_p64` and defines `HAVE_ARM64_CRC64` on supported compilers.
2. **Given** `configure.ac`, **When** running `./configure`, **Then** `AC_LINK_IFELSE` tests `vmull_p64` with `__attribute__((__target__("+crypto")))` and defines `HAVE_ARM64_CRC64`.

---

### User Story 3 - Compiler Inlining & Attribute Safety (Priority: P2)

As a compiler optimizing `liblzma` check routines across GCC (versions 7 through 15) and Clang (versions 10 through 21), I need all `static inline` functions in `crc64_arm64.h` (`keep_high_bytes`, `shift_left`, `shift_right`, `clmul_00`, `clmul_10`, `clmul_11`, `fold`, `fold_xor`) to carry the explicit `crc64_attr_target` attribute, eliminating target-mismatch inlining errors.

**Why this priority**: Adheres to the defensive inlining design pattern established in `crc_x86_clmul.h`.

**Independent Test**:
- Compile `crc64_fast.c` under GCC with `-march=armv8-a` (no crypto flag in CFLAGS) and `-Winline -Wall -Wextra -Werror`.

**Acceptance Scenarios**:
1. **Given** `crc64_arm64.h`, **When** inspecting every `static inline` function, **Then** each function is decorated with `crc64_attr_target`.
2. **Given** compilation under `-march=armv8-a`, **When** inlining helper functions into `crc64_arch_optimized`, **Then** compilation succeeds with zero warnings and zero inlining failures.

---

### User Story 4 - Bit-Exact Correctness & Test Suite Pass (Priority: P1)

As a quality assurance engineer, I need all unit test suites (`test_check`, `test_block_header`, `test_index`, etc.) to pass 100% bit-exact across multiple buffer sizes ($0 \dots 1\text{MB}$) and alignment offsets ($0 \dots 63$), with zero memory leaks and zero undefined behaviors.

**Why this priority**: Guarantees zero regression on decompression/compression integrity.

**Independent Test**:
- Run `ctest` and `make check` under ASan / UBSan.

**Acceptance Scenarios**:
1. **Given** standard ECMA-182 CRC64 test vectors (`"123456789"` $\to$ `0x6C40DF5F0B497347`), **When** evaluated via `lzma_crc64`, **Then** the output exactly matches expected constants.
2. **Given** random data buffers from 0 to 65,536 bytes, **When** comparing hardware PMULL output against generic slice-by-4, **Then** all results match bit-for-bit.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: `configure.ac` MUST support `--disable-arm64-crc64` (defaulting to enabled).
- **FR-002**: `configure.ac` MUST use `AC_LINK_IFELSE` to test whether the compiler can compile and link `vmull_p64` with `__attribute__((__target__("+crypto")))`, defining `HAVE_ARM64_CRC64` on success.
- **FR-003**: `configure.ac` MUST check for the declaration of `HWCAP_PMULL` in `<sys/auxv.h>` and define `HAVE_HWCAP_PMULL` when present.
- **FR-004**: `CMakeLists.txt` MUST provide option `XZ_ARM64_CRC64` (default ON) and use `check_c_source_compiles` to verify `vmull_p64`, defining `HAVE_ARM64_CRC64`.
- **FR-005**: `CMakeLists.txt` MUST check for `HWCAP_PMULL` in `<sys/auxv.h>` using `check_symbol_exists` and add `HAVE_HWCAP_PMULL` to definitions.
- **FR-006**: `src/liblzma/check/crc_common.h` MUST decouple ARM64 runtime detection into `CRC32_ARM64_RUNTIME_DETECTION` and `CRC64_ARM64_RUNTIME_DETECTION`.
- **FR-007**: `src/liblzma/check/crc_common.h` MUST guard `CRC64_ARM64` with `HAVE_ARM64_CRC64` and endianness checks (`!defined(WORDS_BIGENDIAN)`).
- **FR-008**: `src/liblzma/check/crc64_arm64.h` MUST decorate all internal static inline helper functions with `crc64_attr_target`.
- **FR-009**: `src/liblzma/check/crc64_arm64.h` runtime check `is_arch_extension_supported()` MUST safely handle Linux (`HWCAP_PMULL`), FreeBSD (`elf_aux_info`), Windows (`PF_ARM_V8_CRYPTO_INSTRUCTIONS_AVAILABLE`), and macOS (`sysctlbyname`).

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Clean build with zero warnings under GCC and Clang with `-Wall -Wextra -Werror`.
- **SC-002**: 100% pass rate across all test binaries in `ctest` and `make check`.
- **SC-003**: Bit-exact mathematical parity between PMULL and Generic CRC64 across $100,000+$ iterations of randomized buffer sizes ($0 \dots 65,536$ bytes) and alignments ($0 \dots 63$).
- **SC-004**: Autotools `./configure --disable-arm64-crc64` and CMake `-DXZ_ARM64_CRC64=OFF` cleanly disable PMULL and compile generic CRC64.
- **SC-005**: All commits organized atomically with clear, objective commit messages conforming to GNU / XZ project conventions.
