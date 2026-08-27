# TTZip Microkernel Systems Philosophy & Design Principles
## Lessons from High-Performance Systems Engineering (`libdeflate` Case Study)

> **Document Classification**: TTZip Core Architectural Charter  
> **Target Audience**: Systems Engineers, Core Kernel Developers, and Security Auditors  
> **Revision**: 1.0 (Post-libdeflate Deep Audit & Absorption)  
> **Status**: Active & Enforced via Local CI Gates

---

## Ⅰ. Executive Architectural Charter

High-performance data compression and archiving engines operate at the intersection of raw hardware capability, memory topology, and algorithmic complexity. Drawing from the architectural excellence of `libdeflate` (authored by Eric Biggers and hardened across hundreds of millions of production devices), TTZip adopts **Five Fundamental Systems Principles** that govern all microkernel codebases across Rust, C/FFI, and Swift facades.

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        TTZip Microkernel Five Pillars of Systems Design                │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ 1. Pure Microkernel & Zero Global State (天然无锁并发与重入安全)                           │
│ 2. Single-Allocation Invariant & Memory Shaping (单次分配物理塑形与零碎片化)             │
│ 3. Pure Compute Core vs Imperative Shell (纯计算核心与命令式边缘外壳严格物理隔离)          │
│ 4. Bounds-First & Margin Guards (外层单次代价确界与内层分支消除)                          │
│ 5. Compilers Empathy & Zero Undefined Behavior (编译器共情与静态编译期确界)              │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Ⅱ. The Five Pillars in Depth

### 1. Pure Microkernel & Zero Global Mutable State (纯微内核与零全局状态)

- **The Axiom**: The core transformation pipeline must be completely pure, reentrant, and instance-self-contained.
- **The Invariant**:
  - **Zero Mutable Static State**: No global counters, mutable configuration singletons, or uncoordinated global locks are permitted in the calculation hot-path.
  - **Isolated Handle Contexts**: Every compressor/decompressor instance owns its entire scratchpad memory, lookup tables, and state machine.
  - **Lock-Free Scalability**: When processing multi-file batches across 16+ Rayon CPU threads or Grand Central Dispatch queues, execution proceeds with **zero mutex contention, zero atomic fences, and zero inter-thread false sharing**.

---

### 2. Single-Allocation Invariant & Memory Shaping (单次分配物理塑形)

- **The Axiom**: Dynamic memory allocation (`malloc` / `Vec::new`) in the inner hot-path is an architectural defect.
- **The Invariant**:
  - **Deterministic Pre-Allocation**: When an engine or compressor is initialized, all required scratchpads (hash tables, binary trees, dynamic programming node matrices, and sequence buffers) are allocated in **a single contiguous memory block**.
  - **Memory Shaping by Level**: Memory footprints must strictly scale with the chosen compression preset:
    - *Level 1 (Fastest)*: Inline hash tables (~198 KB).
    - *Level 2~9 (Standard)*: Dual hash-chains (~653 KB).
    - *Level 10~12 (Ultra DAG)*: Full binary-tree search & 1.5M match cache (~8.6 MB).
    - *Decompressor*: Compact Huffman lookup table (~12 KB).
  - **Cache Line & SIMD Alignment**: Core buffers must be 64-byte aligned (e.g., via `[-1]` backtracking storage pointer technique) to maximize AVX-512 / ARM64 NEON wide-vector memory throughput.

---

### 3. Pure Compute Core vs. Imperative Shell (纯计算核心与外围外壳隔离)

- **The Axiom**: Calculation must be purely mathematical; side-effects must be pushed entirely to the system boundaries.
- **The Invariant**:
  - **Pure Slice Transformation**: Core compression/decompression functions accept exclusively pure memory slices `(const u8 *in, size_t in_len, u8 *out, size_t out_len)`.
  - **Zero Side-Effects**: The compute core is strictly prohibited from invoking file system I/O (`read`/`write`), networking, thread spawning, or OS-level allocations.
  - **Orthogonal Container Composition**: Packaging formats (GZIP RFC 1952, ZLIB RFC 1950, ZIP, TAR) must be constructed as thin, stateless wrappers that inject headers/trailers and invoke the pure engine without tight coupling.

---

### 4. Bounds-First & Margin Guards (外层单次代价确界与内层分支消除)

- **The Axiom**: Micro-benchmarks are won or lost in branch mispredictions. Inner loops must execute branchless memory operations.
- **The Invariant**:
  - **Worst-Case Margin Calculation**: Before entering the inner literal/match decoding fast-loop, the engine establishes a single safety waterline:
    $$\text{Margin} = \text{MaxWritePerIteration} = 2 + \text{MaxMatchLen} + (5 \times \text{WordBytes}) - 1 \approx 299\text{ Bytes}$$
  - **Branchless Fast-Loop**: As long as remaining pointers reside within the safety margin, all boundary checks `if (ptr >= end)` are omitted in favor of unconditional wide-register loads and stores.
  - **Shift-UB Immunity**: Variable-length bitstream accumulators define active capacity as 63 bits (`BITBUF_NBITS = 63`). With `shift = bitcount & ~7`, the maximum shift is mathematically capped at $56 < 64$, completely eliminating shift-overflow undefined behaviors without defensive branch penalties.

---

### 5. Compilers Empathy & Zero Undefined Behavior (编译器共情与静态确界)

- **The Axiom**: Code must strictly uphold language standards while actively collaborating with modern compiler backends (LLVM/rustc/Clang) to emit optimal vector assembly.
- **The Invariant**:
  - **Strict Aliasing Discipline**: Pointer casting (`*(uint64_t *)p`) for unaligned memory reads is strictly forbidden. Unaligned wide reads must be expressed via compiler-intrinsic memory copies (`memcpy`), which compilers automatically recognize and lower into single unaligned load instructions (`ldr x0` / `mov rax`).
  - **Compile-Time Contract Enforcement**: Use static assertions (`STATIC_ASSERT` / Rust `const_assert!`) to mathematically guarantee struct sizes, table bit-widths, and power-of-two alignments at compile time.
  - **Make Illegal States Unrepresentable**: Leverage strict algebraic data types, non-null guarantees, and exhaustive enum matching to eliminate runtime defensive null-checks.

---

## Ⅲ. TTZip Architecture Enforcement Checklist

Every pull request and microkernel module must pass the following audit checklist before merge:

| Category | Verification Criterion | Automated Gate |
| :--- | :--- | :--- |
| **Purity** | Zero mutable global/static variables across the transformation pipeline | `./scripts/verify_uniffi_symbols.sh` |
| **Allocation** | Zero heap allocation inside inner compression/decompression loops | `ttzip-bench pipeline` |
| **Bounds** | Margin guards applied with scalar fallbacks for boundary tails | `./scripts/run_deflate_defense_tests.sh` |
| **Fault Resilience** | OOM fault-injection tests pass with 0 memory leaks and 0 dangling pointers | `test_custom_malloc` |
| **DoS Defense** | Degenerate/bomb streams (e.g. empty static Huffman blocks) handled in constant time | `test_slow_decompression` |
| **CPU Stripping** | 100% test passing under `LIBDEFLATE_DISABLE_CPU_FEATURES` scalar fallback | `[Stage 7/7] Deflate Deep Defense Gate` |

---

*Authored by the TTZip Core Systems Engineering Team.*
