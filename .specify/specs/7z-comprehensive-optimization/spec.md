# Feature Specification: 7Z 引擎全方位深度升级 (7Z Comprehensive Optimization)

## 一、 需求背景与目标 (Background & Goals)

结合 2024–2026 年最新前沿研究（Branchless Range Coding、ARM64 BCJ 向量化、FL2 Radix 架构、7z-Zstd 容器扩展），分阶段对 TTZip 的 7Z 引擎进行端到端重构与性能升级，使 7Z 压缩与解压吞吐在 Apple Silicon 平台上达到极致，且每个阶段均需通过严格的回归门禁与正向性能优化验证。

## 二、 阶段任务拆解 (Phased Milestones)

- **Phase 1 (Thread-Local 解码器池化与零堆分配)**：
  - 消除每次解压块时 `lzma_raw_decoder` / `lzma_end` 的堆分配与结构体初始化开销，引入 TLS 解压流缓存。
- **Phase 2 (ARM64 指令加速与 NEON 向量化 BCJ 过滤器)**：
  - 引入 ARM64 NEON 指令跳转表过滤器（BCJ for ARM64），加速可执行文件与二进制归档。
  - 优化 Range Coder 概率模型更新为 Branchless CSEL 模式。
- **Phase 3 (7z 固实归档 $O(1)$ 随机访问索引表)**：
  - 实现 `SevenZipSeekTable.swift`，基于 LZMA2 Chunk Checkpoints 支持单文件免全包解压快速提取。
- **Phase 4 (7z-Zstandard 现代混合容器扩展)**：
  - 注册 `0x4F71101` (Zstd) 与 `0x4F71104` (LZ4) 容器 Codec ID，打通 7z 容器原生 Zstd 编解码。

## 三、 质量与性能红线 (Invariants & Redlines)

1. **每步强制回归**：每个阶段完成后，必须执行 `swift test`（550+ tests 全绿）与 `XCTestPerformanceMeasureTests` / `ttzip-cli bench -f 7z`。
2. **零性能退步**：各阶段必须实现指标正向提升或持平，严禁任何场景负优化。
3. **闭环交付**：各阶段测试通过后立即执行原子性 Git 提交与 Push。
