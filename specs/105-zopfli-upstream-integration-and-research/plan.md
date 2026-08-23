# Implementation Plan: Google Zopfli 官方上游集成、深度架构解析与极限无损压制

**Feature ID**: `105-zopfli-upstream-integration-and-research`  
**Author**: Antigravity / CTO Lead  
**Created**: 2026-08-19  
**Status**: DRAFT (Phase 1 Plan)  

---

## 1. Technical Context

- **模块位置**：`Vendor/zopfli-upstream/`、`Sources/CTTZipBridge/zopfli/`、`Sources/CTTZipBridge/ttzip_zopfli_engine.c`、`Sources/TTZipCore/Zip/ZipExtremeBlockWriter.swift`。
- **硬件架构**：Apple Silicon M 系列 (18-Core满载并发，ARM64 NEON)。
- **核心依赖**：In-process C 静态绑定（零外部进程拉起，零外部 CLI 依赖）。
- **Phase 0 研究项与子 Agent 调度**：
  - `- R001 [SUBAGENT:research] 《Zopfli 最优路径图论 DP 与迭代重加权 (Squeeze & Iterative Re-weighting)》`：解析 `squeeze.c`、`katajainen.c`、`tree.c`。
  - `- R002 [SUBAGENT:research] 《动态熵变最优块切分与 18 核心分块并发无锁内存拓扑》`：解析 `blocksplitter.c`、`deflate.c` 与跨块 32KB 字典预热。
- **Phase 1 契约清单**：
  - `specs/105-zopfli-upstream-integration-and-research/contracts/zopfli-upstream.schema.json`

---

## 2. Constitution Check & Invariants

1. **热路径零动态堆分配 (Zero-Cost Abstraction)**：
   - 18 核心并发压缩循环中，各工作线程在进入 Squeeze 前一次性分配内存，内部杜绝 `malloc/free` 抖动。
2. **Fast-Path 旁路保留原则**：
   - Tier 0..5 的 `Method 0` 和 `libdeflate 1..10` 高速旁路 100% 保留，严禁被 Zopfli 慢路径覆盖。
3. **物理实测与绝对零造假铁律**：
   - 所有基准测试数据必须为现场单调时钟实测，严禁使用硬编码假数据。
4. **RFC 1951 解压语义对称性**：
   - 确保 `/usr/bin/unzip -t` 100% PASS（0 CRC errors）。

---

## 3. Proposed Changes & Component Impact

### Component 1: `Vendor/zopfli-upstream/` (Upstream Workspace)
- 完整克隆 Google Zopfli 官方仓库，保持纯净 upstream 状态，支持独立 `make` 与测试。

### Component 2: `Sources/CTTZipBridge/` (In-Process C Bridge)
- 挂载 Zopfli 核心源文件：`deflate.c`, `squeeze.c`, `blocksplitter.c`, `katajainen.c`, `lz77.c`, `tree.c`, `hash.c`, `cache.c`, `util.c`。
- 在 `ttzip_zopfli_engine.c` 中实现 `ttzip_zopfli_compress_block_with_history`，直接调用 `ZopfliDeflatePart`。

### Component 3: `Sources/TTZipCore/` (Swift Parallel Engine)
- 在 `ZipExtremeBlockWriter.swift` 中，当 `activeProfile.level == .level6` 或 `.level7` 时，使用 18 核分块多线程并发调用 `ttzip_zopfli_compress_block_with_history`，前 $N-1$ 块输出 `BFINAL=0` 与 `Z_SYNC_FLUSH`，最后一块输出 `BFINAL=1`。

---

## 4. Verification Plan

1. **Upstream 独立构建**：
   `cd Vendor/zopfli-upstream && make clean && make -j8 && ./zopfli --help`
2. **18 核心并发分块测试与解压验证**：
   `swift test --filter ZipExtremeBlockWriterTests`
3. **全量现场帕累托实测**：
   `TTZIP_BENCH_ALL_LIVE=1 swift test --filter ZipMultiCoreParetoFrontierPkTests`
4. **13 项硬性能门禁防倒退审查**：
   `swift test --filter XCTestPerformanceMeasureTests`
