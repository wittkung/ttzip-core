# Implementation Plan: ZIP Extreme Speed Multi-Core Block-Parallel Mode

## Technical Context

在 TTZip 中实现极速多核分块压缩引擎 `ZipExtremeBlockWriter`：
1. **分块策略**：512KB ~ 1MB 自适应分块，每个分块由独立线程上的 `libdeflate_compressor` 并发压缩。
2. **RFC 1951 流式拼接**：前置非末尾分块清除 `BFINAL` 位并注入 `0x00, 0x00, 0xFF, 0xFF` 同步标记；最终分块保留 `BFINAL=1`。
3. **ZIP Header 与 Central Directory**：写入标准的 Local File Header、Central Directory Entry 与 CRC32 校验和，确保任何第三方工具均可无缝解压。
4. **性能目标**：100MB 真实文件在 Apple Silicon 18 核上达到 **>10,000 MB/s**。

## Constitution Check

- [P0] 热路径零中间堆分配：使用无锁环形缓冲区与线程局部状态池。
- [P1] 零锁竞争：并发分块使用预分配插槽数组。
- [P2] 零退化：标准单文件压缩路径不受影响。

---

## Phase 0: Research Tasks & Findings

- R001 [SUBAGENT:research] 《RFC 1951 Deflate 流式分块拼接与 Apple Silicon 多核吞吐优化研究》：分析 BFINAL 标记、字节对齐与 18 核吞吐理论上限。

---

## Phase 1: Design Artifacts & Contracts

- `research.md`
- `data-model.md`
- `contracts/zip-extreme-mode-contract.json`
- `quickstart.md`
- `tasks.md`

---

## Planned Changes by Component

- [NEW] `Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift`: 极速分块多核并行 ZIP 归档创建器。
- [MODIFY] `Tests/TTZipTests/SoftwareParetoFrontierPkTests.swift`: 纳入 TTZip Extreme Speed 极速多核档位对比。
