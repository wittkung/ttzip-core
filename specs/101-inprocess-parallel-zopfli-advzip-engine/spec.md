# Feature Specification: In-Process 18-Core Parallel Zopfli/Advzip Engine & All-Level Pigz Benchmark Matrix

**Feature ID**: `101-inprocess-parallel-zopfli-advzip-engine`  
**Status**: Draft  
**Author**: Antigravity CTO / Spec Kit Autonomous Pipeline  
**Target Platform**: macOS 14.0+ (Apple Silicon C11 / ARM64 SIMD & Swift 6.0)  
**Created**: 2026-08-18  

---

## 1. Executive Summary

为了彻底消除对外部 Homebrew 二进制进程（`/opt/homebrew/bin/pigz` 和 `/opt/homebrew/bin/advzip`）的依赖，满足 Mac App Store 沙盒（MAS）要求，并将 Tier 7 极限压缩的物理耗时从 200 秒（3.3分钟）大幅压缩至 10~15 秒（15~18 倍加速），本项目将构建 **100% 进程内 18 核心无锁并发的 Zopfli / Advzip 极限重压引擎**，并在基准测试中对齐 `pigz` 的全量 11 大物理级别（`-0, -1, -2, -3, -4, -5, -6, -7, -8, -9, -11`）。

---

## 2. Functional Requirements

### FR-001: 100% 进程内 18 核心并发 Zopfli / Advzip 物理引擎 (`ttzip_zopfli_parallel.c`)
- 在 `Sources/CTTZipBridge/` 中内嵌高性能 C11 原生 Zopfli 与多轮迭代块切分算法；
- 支持 18 核心无锁并发分块压缩（GCD `DispatchQueue.concurrentPerform` / POSIX 线程池）；
- 支持动态迭代轮次配置（Level 6: 5 轮次, Level 7: 15 轮次）；
- 彻底剔除 `Process()` CLI 调用，实现 100% 进程内原生运行与 MAS 沙盒合规。

### FR-002: 跨块 32KB 字典预热与连续滑动窗口保护 (Overlapping 32KB History Warmup)
- 并发任务在切分数据块时，每个子任务自动获取前一块末尾的 32KB 历史数据作为字典预热；
- 保证分块压缩率与全局单块压缩率几乎等价，消除分块边界压缩率损失。

### FR-003: 自适应信息论收敛自适应剪枝 (Adaptive Convergence Early-Exit)
- 在多轮迭代重平衡过程中，监控符号位代价改善率 $\Delta \text{Cost} = \text{Cost}_{k-1} - \text{Cost}_k$；
- 当 $\Delta \text{Cost} / \text{Cost}_{k-1} < 0.0001$（已达不动点）时，自动提前跳出剩余迭代轮次，节省 30%~50% 冗余耗时。

### FR-004: pigz 全量 11 级物理点位矩阵测试覆盖
- 将 `pigz` 在基准测试中的评测档位扩充至原生全矩阵：
  `[-0 (Store), -1 (Fast), -2, -3, -4, -5, -6 (Normal), -7, -8, -9 (Ultra), -11 (Zopfli)]`；
- 所有点位真实运行并由 `CompetitorBenchmarkCacheManager` 进行指纹缓存。

---

## 3. Success Criteria & Hard Performance Floor

- **SC-001 (100% In-Process & Zero CLI)**：`Sources/TTZipCore/` 与 `Sources/CTTZipBridge/` 中零 `/opt/homebrew/` 路径引用，零 `Process()` 进程启动，100% 满足 MAS 构建；
- **SC-002 (Tier 7 吞吐 15x 提升)**：Tier 7 极限压缩在 18 核 Apple Silicon 下的吞吐从 $0.28\text{ MB/s}$ 跃升至 $\ge 4.5 \sim 9.0\text{ MB/s}$，100MB 耗时从 200 秒下降至 $\le 20\text{ 秒}$；
- **SC-003 (极限体积征服 advzip -4)**：在 100MB 真实文本语料上，Tier 7 压缩后物理文件大小严格 $\le 2.96\text{ MB}$（优于 `advzip -4` 的 2.994 MB）；
- **SC-004 (全量 525+ 测试 100% 通过)**：全量单元测试与 CI 门禁零错误、零警告通过。
