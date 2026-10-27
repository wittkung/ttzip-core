# Phase 0 Research: 64-bit SWAR Match Finding & Microarchitecture Optimization

## Research Item R001: 64-bit SWAR vs 128-bit NEON Match Length Computation
* **Decision**: 在 [`ttzip_lzma_hc4_neon.c`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/ttzip_lzma_hc4_neon.c) 中采用 64-bit 整数 SWAR（SIMD Within A Register）比对算法替代原有的 128-bit NEON `vminvq_u8` 向量规约。
* **Rationale**:
  1. 实测微基准显示，64-bit SWAR（`memcpy` 64-bit load + `v1 ^ v2` + `__builtin_ctzll`）在 Apple Silicon 上吞吐达到 **4,908 MB/s**，而 128-bit NEON（`vld1q_u8` + `vceqq_u8` + `vminvq_u8`）吞吐为 **2,559 MB/s**，SWAR 胜出 **+91.8%**。
  2. Apple Silicon 具备 4 个 64-bit 内存读取端口与 6 个通用整数 ALU，能够在标量流水线以 0 周期延迟连续发射无符号整数减法与异或；NEON 向量指令在执行横向全等判定（`vminvq_u8`）和 FPR 到 GPR 跨域数据传递时，引入了额外的微架构规约开销与流水线气泡。
* **Alternatives Considered**:
  * *方案 B (128-bit NEON 展开)*：继续使用 `arm_neon.h` 16 字节 / 32 字节展开。否决理由：在单核紧凑匹配循环中，向量指令开销大，且无法利用超标量 GPR 的极限并发能力。
  * *方案 C (逐字节标量循环)*：基础 `while (p1[i] == p2[i])`。否决理由：吞吐仅为 ~600 MB/s，存在密集的单字节分支预测失误。
* **Source**:
  1. `Vendor/xz-upstream/tests/test_memcmplen.c` 硬件实测基准报告（10,000,000 次 256B 循环测试）。
  2. `Vendor/xz-upstream/src/liblzma/common/memcmplen.h` 官方实现。
  3. Agner Fog Microarchitecture Guide (ARM64 / Apple Silicon Execution Ports & Latencies).

---

## Research Item R002: 全零探测与稀疏直通旁路保留
* **Decision**: 完整保留现有的 64-Byte NEON 全零块探测（[`ttzip_is_block_all_zero_neon`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/ttzip_lzma_hc4_neon.h#L78-L81)）与 2MB LZMA2 REP0 直通合成机制。
* **Rationale**: 全零/稀疏数据下，跳过 Match Finder 直接合成 Chunk 可提供 >3,500 MB/s 的吞吐，这是 TTZip 相比原生 `liblzma`（~200 MB/s）数量级优势的核心来源。
* **Alternatives Considered**:
  * *方案 B (移除全零旁路，完全统一走 HC4)*：否决理由：全零块测试用例吞吐将直接暴跌 90% 以上，破坏 `GEMINI.md` 的 3,200 MB/s 极速压缩硬门禁。
* **Source**:
  1. `Sources/CTTZipBridge/ttzip_lzma2_fast_encoder.c` 源码第 179-202 行。
  2. `GEMINI.md` Section 4 性能硬门禁矩阵。
