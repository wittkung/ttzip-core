# Research Findings: XZ PR 2 Review Remediation

## R001: Vector Shift Nomenclature and Comments in ARM NEON

- **Decision**: Define `shift_left` as shifting bytes toward higher indices (towards most significant bytes, clearing lower indices), and `shift_right` as shifting toward lower indices (clearing higher indices).
- **Rationale**: In little-endian ARM64 vectors, low memory addresses occupy lower byte lanes (lanes 0..7 in low 64-bit d-register). `vqtbl1q_u8(v, vmasks + 32 - amount)` shifts byte values into higher lane positions, leaving zeros at the lowest lane positions. The comment must say "Shift left by amount bytes. The lowest amount bytes are cleared."
- **Alternatives Considered**: Inverting the function names (`shift_left` <-> `shift_right`). Rejected because the function names already match `crc_x86_clmul.h` conventions and changing names would break architectural symmetry with x86.
- **Source**: `Vendor/xz-upstream/src/liblzma/check/crc_x86_clmul.h:86-102` & ARM NEON Intrinsic Reference.

---

## R002: macOS sysctl Feature Detection Error Handling

- **Decision**: Match the exact pattern in `src/liblzma/check/crc32_arm64.h:134-138`:
  ```c
  if (sysctlbyname("hw.optional.arm.FEAT_PMULL", &has_pmull, &size, NULL, 0) != 0)
      return false;
  return has_pmull;
  ```
- **Rationale**: Eliminates the unintended fallthrough `return true;` while keeping the detection concise, idiomatic, and 100% consistent with the rest of liblzma.
- **Alternatives Considered**: Direct compile-time `#define is_arch_extension_supported() true` on Apple Silicon. Rejected because liblzma maintains a unified runtime detection structure for `CRC64_ARCH_OPTIMIZED` builds.
- **Source**: [`Vendor/xz-upstream/src/liblzma/check/crc32_arm64.h:126-139`](file:///Users/kevintung/Documents/dev/TTZip/Vendor/xz-upstream/src/liblzma/check/crc32_arm64.h#L126-L139).

---

## R003: Reproducibility Harness Architecture

- **Decision**: Create a single self-contained C11 file with embedded reference table generator, PMULL kernel, golden ECMA-182 vectors, and memory clobber loops.
- **Rationale**: Reviewers like `@ssvb` require an immediate, reproducible way to test on Linux ARM64 (Raspberry Pi, Graviton, Neoverse) and macOS (Apple Silicon M1/M2/M3/M4/M5) without needing autotools/cmake setup.
- **Alternatives Considered**: Shell script running `xz -b`. Rejected because `xz -b` tests multi-threaded LZMA2 compression rather than the isolated CRC64 arithmetic throughput.
- **Source**: TTZip Benchmark Suite & ECMA-182 Standard.
