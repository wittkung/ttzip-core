# Research & Technical Decisions: XZ PR #241 Remediation

**Feature**: `specs/214-xz-arm64-crc64-remediation`  
**Target**: `Vendor/worktrees/xz/pr2-arm64-crc64`

---

## 1. Technical Decisions

### Decision 1: Build System Compiler Probes (`CMakeLists.txt` and `configure.ac`)

- **Decision**: Introduce explicit compile-and-link checks (`AC_LINK_IFELSE` in Autotools and `check_c_source_compiles` in CMake) testing `vmull_p64` with `__attribute__((__target__("+crypto")))`, defining `HAVE_ARM64_CRC64`.
- **Rationale**:
  - Parallels existing `HAVE_ARM64_CRC32` and `HAVE_USABLE_CLMUL` checks in XZ Utils.
  - Allows seamless fallback to generic CRC64 implementation on compilers lacking PMULL intrinsics or target attribute support.
- **Alternatives Considered**:
  - *Blind activation on `__aarch64__`*: Fails on older compilers or bare-metal embedded toolchains without Crypto extensions.

### Decision 2: Decoupled Runtime Detection Macros in `crc_common.h`

- **Decision**: Split `CRC_ARM64_RUNTIME_DETECTION` into `CRC32_ARM64_RUNTIME_DETECTION` and `CRC64_ARM64_RUNTIME_DETECTION`.
  ```c
  #if defined(_WIN32) \
          || (defined(HAVE_GETAUXVAL) && defined(HAVE_HWCAP_CRC32)) \
          || defined(HAVE_ELF_AUX_INFO) \
          || (defined(__APPLE__) && defined(HAVE_SYSCTLBYNAME))
  #   define CRC32_ARM64_RUNTIME_DETECTION 1
  #endif

  #if defined(_WIN32) \
          || (defined(HAVE_GETAUXVAL) && defined(HAVE_HWCAP_PMULL)) \
          || defined(HAVE_ELF_AUX_INFO) \
          || (defined(__APPLE__) && defined(HAVE_SYSCTLBYNAME))
  #   define CRC64_ARM64_RUNTIME_DETECTION 1
  #endif
  ```
- **Rationale**:
  - On Linux AArch64, `HWCAP_CRC32` and `HWCAP_PMULL` are independent bitflags. A kernel/glibc environment may define one without the other.
  - Decoupling ensures that `crc64_arm64.h` is only included when `HAVE_HWCAP_PMULL` is physically present, eliminating `#error Runtime detection method unavailable.`
- **Alternatives Considered**:
  - *Unified macro checking both*: Would disable CRC32 acceleration if `HWCAP_PMULL` is missing on older glibc, causing unnecessary performance regression.

### Decision 3: Comprehensive Function Target Attributes in `crc64_arm64.h`

- **Decision**: Decorate all internal static inline functions (`keep_high_bytes`, `shift_left`, `shift_right`, `clmul_00`, `clmul_10`, `clmul_11`, `fold`, `fold_xor`) with `crc64_attr_target`.
- **Rationale**:
  - When compiling with generic architecture flags (e.g., `-march=armv8-a`), GCC strict inlining rules require callee functions invoking target-specific intrinsics (`vmull_p64`) to match or exceed the caller's target attributes.
  - Matches the exact pattern used by Lasse Collin in `crc_x86_clmul.h` (`crc_attr_target`).
- **Alternatives Considered**:
  - *Decorating only top-level function*: Fails with `inlining failed due to target mismatch` on GCC 8-11.

---

## 2. Upstream Architecture Alignment Matrix

| Dimension | x86 CLMUL Reference (`crc_x86_clmul.h`) | ARM64 CRC32 Reference (`crc32_arm64.h`) | ARM64 CRC64 PMULL (`crc64_arm64.h`) |
| :--- | :--- | :--- | :--- |
| **Autotools Flag** | `--disable-clmul-crc` | `--disable-arm64-crc32` | `--disable-arm64-crc64` |
| **CMake Option** | `XZ_CLMUL_CRC` | `XZ_ARM64_CRC32` | `XZ_ARM64_CRC64` |
| **Compiler Macro** | `HAVE_USABLE_CLMUL` | `HAVE_ARM64_CRC32` | `HAVE_ARM64_CRC64` |
| **Runtime HWCAP** | N/A (CPUID) | `HWCAP_CRC32` | `HWCAP_PMULL` |
| **Inline Target Attr** | `crc_attr_target` on all helpers | Target on function | `crc64_attr_target` on all helpers |
