# Interface & Build Contract: XZ PR #241 Remediation

**Feature**: `specs/214-xz-arm64-crc64-remediation`  
**Contract Version**: 1.0.0

---

## 1. Build System Interface Contract

### CMake Interface

| Option | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `XZ_ARM64_CRC64` | `BOOL` | `ON` | Enable ARM64 PMULL instructions for CRC64 if compiler supports `vmull_p64` |

### Autotools Interface

| Flag | Default | Description |
| :--- | :--- | :--- |
| `--disable-arm64-crc64` | Enabled (`yes`) | Do not use ARM64 PMULL instructions even if support is detected |

---

## 2. C ABI Function Signatures & Exports

```c
// Public API in lzma/check.h
extern LZMA_API(uint64_t) lzma_crc64(
    const uint8_t *buf, size_t size, uint64_t crc);

// Internal Arch-Optimized Signature in check/crc64_arm64.h
crc64_attr_target
static uint64_t crc64_arch_optimized(
    const uint8_t *buf, size_t size, uint64_t crc);

// Internal Generic Signature in check/crc64_fast.c
static uint64_t lzma_crc64_generic(
    const uint8_t *buf, size_t size, uint64_t crc);

// Runtime Detection Hook in check/crc64_arm64.h
static inline bool is_arch_extension_supported(void);
```

---

## 3. Behavioral Guarantees

1. **Bit-Exact Invariance**:
   $$\forall \text{buf} \in \mathcal{B}^N, \forall \text{crc}_0 \in \mathbb{U}_{64}: \text{lzma\_crc64}(\text{buf}, N, \text{crc}_0) \equiv \text{lzma\_crc64\_generic}(\text{buf}, N, \text{crc}_0)$$
2. **Buffer Safety**:
   - Zero out-of-bounds reads for buffer size $N \in [0, \infty)$ and pointer alignment $P \equiv \text{buf} \pmod{64} \in [0, 63]$.
3. **Graceful Fallback**:
   - If CPU lacks PMULL capability, `is_arch_extension_supported()` evaluates to `false`, routing transparently to `lzma_crc64_generic` with 0 crash risk.
