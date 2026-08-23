# Data Model & In-Register Execution State: AArch64 compare256 2x-Unrolled NEON

**Feature Directory**: `specs/110-aarch64-compare256-zero-overhead-optimization`  

---

## 1. Core State Entities

### In-Register Vector Pipeline State (`compare256_neon_static`)

```
Registers Allocated:
- GPR x8: Pointer offset (src0 - src1)
- GPR x1: Base pointer src1 (auto-incremented via post-indexing)
- GPR w9: Branch condition flag (fmov w9, s2)
- SIMD Q0: Chunk 1 Source 0 Vector (128-bit)
- SIMD Q1: Chunk 1 Source 1 Vector (128-bit)
- SIMD Q2: Chunk 2 Source 0 Vector (128-bit)
- SIMD Q3: Chunk 2 Source 1 Vector (128-bit)
- SIMD V0: Chunk 1 Difference (veorq_u8(Q0, Q1))
- SIMD V1: Chunk 2 Difference (veorq_u8(Q2, Q3))
- SIMD V2: Merged Difference (vorrq_u8(V1, V0))
```

---

## 2. Execution State Transitions

```
[State 0: Loop Init]
  offset = (intptr_t)src0 - (intptr_t)src1
  len = 0

[State 1: Dual 16B Vector Load (32 Bytes)]
  Q0 = *(src1 + offset)
  Q1 = *(src1), src1 += 16
  Q2 = *(src1 + offset)
  Q3 = *(src1), src1 += 16

[State 2: XOR Difference & Merge]
  V0 = Q0 ^ Q1
  V1 = Q2 ^ Q3
  V2 = V0 | V1

[State 3: Single-Instruction Horizontal Max]
  B2 = UMAXV(V2)
  w9 = FMOV(B2)

[State 4A: Fast-Path 32B Match (w9 == 0)]
  len += 32
  if (len < 256) goto State 1
  return 256

[State 4B: Pinpoint Fallback (w9 != 0)]
  lane = V0.lane[0] (bytes 0..7)   --> if (lane) return len + CTZ(lane)/8
  lane = V0.lane[1] (bytes 8..15)  --> if (lane) return len + 8 + CTZ(lane)/8
  lane = V1.lane[0] (bytes 16..23) --> if (lane) return len + 16 + CTZ(lane)/8
  lane = V1.lane[1] (bytes 24..31) --> return len + 24 + CTZ(lane)/8
```
