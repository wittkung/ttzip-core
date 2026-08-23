# Implementation Plan: 094 Entropy-Aware Tiered Chunking Engine

## Technical Context

将基于香农熵的自适应阶梯分块函数下沉至 C 桥接层与 Swift 核心层：
1. **C 阶梯分块决策函数**：
   ```c
   size_t ttzip_calculate_adaptive_block_size(double entropy, size_t file_size);
   ```
   - $H < 3.5 \implies 2048\text{ KB}$ ($2\text{MB}$)；
   - $3.5 \le H < 6.0 \implies 512\text{ KB}$；
   - $6.0 \le H < 7.35 \implies 128\text{ KB}$；
   - $H \ge 7.35 \implies 0$ (Direct Store Method 0)。
2. **Swift 引擎调度**：
   - `ZipExtremeBlockWriter.swift`：根据 `ttzip_probe_entropy_and_compressibility` 得出的 `entropy` 与 `file_size` 动态计算 `effectiveBlockSize`，并发调度分块。

## Constitution Check

- [P0] 零堆分配：分块决策函数为纯数学寄存器运算，零 `malloc`。
- [P1] 门禁约束：低熵大文件压缩比提升 $\ge 15\%$，高熵吞吐 $\ge 15\text{ GB/s}$。
- [P2] 比特精确兼容：生成的分块 ZIP 包通过 `/usr/bin/unzip -t` 100% 校验。

---

## Phase 0: Research Tasks & Findings

- R001 [SUBAGENT:research] 《香农信息熵阶梯与 Apple Silicon M 系列芯片 L1/L2 缓存拓扑匹配数学研究》

---

## Phase 1: Design Artifacts & Contracts

- `research.md`
- `data-model.md`
- `contracts/entropy-tiered-chunking-contract.json`
- `quickstart.md`
- `tasks.md`

---

## Planned Changes by Component

- [MODIFY] `Sources/CTTZipBridge/include/CTTZipStreamCoder.h`: 声明 `ttzip_calculate_adaptive_block_size`。
- [MODIFY] `Sources/CTTZipBridge/CTTZipStreamCoder.c`: 实现阶梯映射算法。
- [MODIFY] `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift`: 接入自适应分块大小与熵自适应调度。
- [NEW] `Tests/TTZipTests/EntropyTieredChunkingEngineTests.swift`: 单元测试与 4-Tier 熵阶梯基准实测。
