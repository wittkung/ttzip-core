# Feature Specification: 全格式深度攻坚与全场景全面霸榜 (Universal Format Dominance & Zero Regression)

## 一、 需求背景与动机 (Background & Motivation)

TTZip 作为 macOS 原生高性能归档工具，在 ZIP 格式下已全面碾压官方 `ditto` 与 `7-Zip 7zz`，解压吞吐突破 10+ GB/s。
但在 7Z、TAR.ZST、TAR.GZ 等格式的部分极限场景下，仍存在落后于竞品或未全面应用 ZIP 架构经验的情况：
1. **7Z 单体大文件解压落后**：缺少 ARM64 寄存器常驻汇编与无分支 Range Coder 状态机。
2. **7Z AES-256 加密解压落后**：缺少 ARMv8 Crypto Extensions（`vaeseq`/`vaesmcq`/`vsha256`）硬件指令直通。
3. **7Z 小文件 Level 1 压缩落后**：缺少 Fast LZMA2 (FL2) Radix 匹配查找器与无锁任务环形队列。
4. **TAR.ZST / TAR.GZ 压缩流水线落后**：缺少多核独立分块滑动窗口与流式流水线。
5. **AAR (Apple Archive)**：走 CLI 外部进程，缺少 100% 进程内 `AppleArchive.framework` / `libcompression` 原生绑定。

## 二、 核心攻坚目标 (Goals & Deliverables)

- **GOAL-001 (初始基准固化)**：在修改代码前将全部格式与场景数据持久化至 `docs/benchmarks/universal_pre_optimization_baseline.json`，确立不可跌破的零退步硬门禁。
- **GOAL-002 (7Z 大文件解压突破)**：手写 ARM64 寄存器常驻与无分支 Range Coder 状态机，大文件解压突破 6,000+ MB/s。
- **GOAL-003 (7Z AES-256 硬件线速解密)**：打通 ARMv8 Crypto 硬件指令，加密解压超越 7-Zip（>= 4,000 MB/s）。
- **GOAL-004 (7Z 小文件 FL2 Radix 打包)**：集成 Fast LZMA2 Radix 匹配查找器，小文件打包突破 1,000 MB/s。
- **GOAL-005 (TAR.ZST / TAR.GZ 多核流水线)**：重构为全核分块滑动窗口与 ZSTD 流式流水线，压缩全线超越竞品。
- **GOAL-006 (全格式复制 ZIP 架构)**：小文件 $\le 64\text{KB}$ 栈缓冲、64B 内存对齐、256 FD 信号量池、无锁原子同步、SeekTable 随机访问。

## 三、 验收标准与硬门禁 (Acceptance Criteria)

- **AC-001**：全量单元测试（552+ tests）100% 通过，0 失败、0 错误、0 warning。
- **AC-002**：`AllFormatsPkSuiteTests` 全场景物理 1v1 测速中，对比 `universal_pre_optimization_baseline.json` 实现 **0% 性能倒退**，各攻坚点实现大幅正向领先。
- **AC-003**：各阶段实施后执行原子性 Git Commit 并 Push 至 `origin/main`。
