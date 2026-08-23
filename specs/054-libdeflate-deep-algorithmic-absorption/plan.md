# Implementation Plan: Deep Algorithmic Absorption of libdeflate

**Feature Directory**: `specs/054-libdeflate-deep-algorithmic-absorption`
**Created**: 2026-08-18
**Status**: Ready for Tasks

---

## 1. Technical Context

深入剖析并吸收 `libdeflate` 源码仓库（C11/汇编）的五大底层核心技术精髓：
1. **硬件多项式宽折叠 Adler-32 / CRC-32 校验和体系**：5552 字节分块延迟取模、NEON DotProd 4 路展开与 PMULL/PCLMUL 宽折叠，将校验和吞吐拉升至 25~35 GB/s。
2. **SIMD `matchfinder_rebase` 向量化索引重置与 24-bit 宽字哈希**：16-bit 相对坐标、NEON `vqaddq_s16` 饱和减法重置（$< 2\mu s$）与 Knuth 乘法哈希。
3. **64-bit 无分支位流解码与 1 次访存 Huffman 联合表**：64 位机器字长累加器、无分支预充、单条目联合解码与 16 字节 SIMD 重叠拷贝。
4. **32/64 字节平坦缓存行对齐结构体**：消除指针间接引用与堆碎片，L1D Cache 命中率提升至 99%。
5. **跨平台运行时 CPU 特性探测中枢**：平台自适应函数指针表无锁派发。

---

## 2. Constitution Check

- [x] **流式第一性 (Stream-First)**：保持零全量内存假设与微缓冲拉取；热路径零堆碎片与零 `Data(count:)`。
- [x] **纵深防御 (Invariant-First)**：非对齐内存加载宏防护；尾部短数据安全回退标量循环。
- [x] **确定性确界 (Bounds-First)**：5552 字节严格数学无溢出上界；16-bit 饱和截断保证索引不越界。
- [x] **真实预言机 (Oracle-First)**：与 RFC 1950 / RFC 1951 / RFC 1952 官方预言机进行 100% 比特精确差分比对。
- [x] **性能底线 (Hard Floors)**：Adler-32 $\ge 20\text{ GB/s}$，CRC-32 $\ge 25\text{ GB/s}$，已有性能门禁零倒退。

---

## 3. Phase 0: Research Index

- - R001 [SUBAGENT:research] 《硬件级 Adler-32 延迟取模与 NEON/AVX2 向量化实现》：详见 [`research.md#R001`](file:///Users/kevintung/Documents/dev/TTZip/specs/054-libdeflate-deep-algorithmic-absorption/research.md)
- - R002 [SUBAGENT:research] 《SIMD matchfinder_rebase 向量化索引重置与短字长哈希》：详见 [`research.md#R002`](file:///Users/kevintung/Documents/dev/TTZip/specs/054-libdeflate-deep-algorithmic-absorption/research.md)
- - R003 [SUBAGENT:research] 《64-bit 无分支位流解码与 1 次访存 Huffman 联合解码表》：详见 [`research.md#R003`](file:///Users/kevintung/Documents/dev/TTZip/specs/054-libdeflate-deep-algorithmic-absorption/research.md)

---

## 4. Phase 1: Design Artifacts & Contracts

- **Data Model**: [`data-model.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/054-libdeflate-deep-algorithmic-absorption/data-model.md)
- **Contracts** `[SUBAGENT:research]`:
  - [`contracts/adler32_checksum_result.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/054-libdeflate-deep-algorithmic-absorption/contracts/adler32_checksum_result.json)
  - [`contracts/matchfinder_rebase_params.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/054-libdeflate-deep-algorithmic-absorption/contracts/matchfinder_rebase_params.json)
  - [`contracts/branchless_bitbuf_state.json`](file:///Users/kevintung/Documents/dev/TTZip/specs/054-libdeflate-deep-algorithmic-absorption/contracts/branchless_bitbuf_state.json)
- **Validation Guide**: [`quickstart.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/054-libdeflate-deep-algorithmic-absorption/quickstart.md)

---

## 5. Proposed Changes by Component

### Component 1: CTTZipBridge (C Algorithmic Core)

#### [NEW] [CTTZipAdler32Neon.c](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/CTTZipAdler32Neon.c)
- 硬件级 Adler-32 实现，包含 5552 字节延迟取模、NEON DotProd 4 路展开与基准 NEON pairwise 累加。

#### [NEW] [CTTZipChecksum.h](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/CTTZipChecksum.h)
- 统一导出 `ttzip_adler32_fast` 与 `ttzip_crc32_fast` 硬件加速校验和中枢接口。

#### [MODIFY] [CTTZipNEONMatchFinder.h](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/CTTZipNEONMatchFinder.h)
- 增加 16-bit 相对位置 `ttzip_matchfinder_rebase_neon`（`vqaddq_s16`）与 `ttzip_load_u24_unaligned` 宏定义。

---

### Component 2: TTZipCore (Swift Hardware Adapters)

#### [NEW] [HardwareChecksumAdapter.swift](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Crypto/HardwareChecksumAdapter.swift)
- Swift 强类型统一硬件校验和适配器，直通 `ttzip_adler32_fast` 与 `libdeflate_crc32`。

---

### Component 3: Tests & Oracle

#### [NEW] [HardwareChecksumTests.swift](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/HardwareChecksumTests.swift)
- Adler-32 与 CRC-32 吞吐微基准与 RFC 1950 黄金预言机逐比特一致性测试。

#### [NEW] [FastMatchFinderTests.swift](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/FastMatchFinderTests.swift)
- 16-bit 相对索引重置延迟（$\le 5\mu s$）与短哈希碰撞率比对测试。

---

## 6. Verification Plan

1. **Adler-32 与 CRC-32 黄金预言机测试**：`swift test --filter HardwareChecksumTests`
2. **SIMD Rebase 延迟与正确性测试**：`swift test --filter FastMatchFinderTests`
3. **全量单元测试与热路径门禁**：`swift test && swift test --filter XCTestPerformanceMeasureTests`
