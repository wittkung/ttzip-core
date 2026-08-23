# Feature Specification: Google Zopfli 官方上游集成、深度架构解析与极限无损压制

**Feature ID**: `105-zopfli-upstream-integration-and-research`  
**Author**: Antigravity / CTO Lead  
**Created**: 2026-08-19  
**Status**: DRAFT (Specify Phase)  

---

## 1. Executive Summary & Problem Statement

### 1.1 背景与原始动机
在 ZIP 压缩格式的帕累托前沿评测中，TTZip 已经通过 `Method 0 Direct Store` 与 `libdeflate` 18 核心并发架构在 **Tier 0 ~ Tier 5（高速至图论档位，吞吐 508 MB/s ~ 6.6 GB/s）** 实现了对 `pigz -0` 到 `pigz -9` 的全面压制。

但在追求极限空间节省率的 **Tier 6（Ultra Zopfli）** 与 **Tier 7（Extreme Peak）** 档位中，单纯的单次图论近最优搜索（产出 $3.03\text{ MB}$）尚未跨越到 `pigz -11 (Zopfli)`（$3.01\text{ MB}$）与 `advancecomp (advzip -4)`（$2.99\text{ MB}$）的更小体积右侧。

为了彻底征服极限压缩领域，并为后续向 Google 官方开源社区贡献 ARM64 NEON/SWAR 向量化加速 Patch 做好充分准备，本项目正式在 `Vendor/zopfli-upstream` 集成官方 Google Zopfli 完整仓库，并完成底层无锁 18 核心分块并发流式绑定与深度架构解析。

### 1.2 核心目标
1. **统一 Upstream 仓库规范**：在 `Vendor/zopfli-upstream` 建立标准的开源上游工作区（包含纯净 Git 历史、Makefile、CMakeLists.txt 与原生测试套件）。
2. **底层 C 静态绑定与无锁并发**：将 Google Zopfli 核心压缩器（`deflate.c`、`squeeze.c`、`blocksplitter.c`、`katajainen.c`、`lz77.c`）无缝绑定入 `Sources/CTTZipBridge/`，支持 18 核心饱和分块多线程并行压缩。
3. **帕累托极限攻坚实测**：
   - **Tier 6 (Ultra Zopfli, 5 轮迭代)**：压缩体积必须严格小于 `pigz -11`（$< 3.01\text{ MB}$ / $\le 2.99\text{ MB}$），18 核满载吞吐 $\ge 4.5\text{ MB/s}$（超越 pigz-11 的 $3.02\text{ MB/s}$）；
   - **Tier 7 (Extreme Peak, 15 轮迭代+动态块切分)**：压缩体积必须严格小于 `advzip -4`（$< 2.99\text{ MB}$ / $\le 2.95\text{ MB}$），18 核满载吞吐 $\ge 1.5\text{ MB/s}$（超越 advzip-4 的 $0.71\text{ MB/s}$）。
4. **深度架构解析与开源上游贡献准备**：输出完整的 Zopfli 5 大核心机制深度解析报告，识别出可以在 upstream 优化的热路径（如哈希链查找 NEON SIMD 化、定点数对数表加速等）。

---

## 2. User Scenarios & Acceptance Criteria

### 2.1 核心用户场景
- **US1: 极限无损压缩归档**：用户在 GUI 或 CLI 中选择 Level 6 或 Level 7 档位，TTZip 调用 18 核心并行 Zopfli 引擎，在 100MB 真实语料上产生体积 $< 2.99\text{ MB}$ 与 $< 2.95\text{ MB}$ 的极致压缩包，超越业界已知所有 ZIP 工具。
- **US2: 标准原生系统解压验证**：由 Zopfli 引擎分块并发压缩产出的 ZIP 归档，直接经由 macOS 系统自带 `/usr/bin/unzip -t` 与 `unzip -p` 解压校验，保证 100% 校验通过，0 字节不一致。
- **US3: 帕累托图表全面主导**：在基准测试图表中，TTZip L6 严格位于 `pigz -11` 右上方，TTZip L7 严格位于 `advzip -4` 右上方，实现全谱系 8 大档位绝对主导。
- **US4: 开源上游贡献工作流**：开发者进入 `Vendor/zopfli-upstream`，能够独立运行 `make` 编译与 `./zopfli` 官方验证，为后续提交 upstream PR 建立隔离沙盒。

### 2.2 成功衡量指标 (Success Criteria)

| 场景 / 指标 | 现有状态 | 目标要求 | 验证手段 |
| :--- | :--- | :--- | :--- |
| **Tier 6 产出体积 (100MB enwik8)** | $3.03\text{ MB}$ | **$< 3.01\text{ MB}$ ($\le 2.99\text{ MB}$)** | 物理 `ls -l` 字节统计 |
| **Tier 6 实测吞吐 (18-Core)** | $166.9\text{ MB/s}$ (L12单次) | **$\ge 4.5\text{ MB/s}$** ($> \text{pigz-11 } 3.02\text{ MB/s}$) | 单调时钟物理计时 |
| **Tier 7 产出体积 (100MB enwik8)** | $3.03\text{ MB}$ | **$< 2.99\text{ MB}$ ($\le 2.95\text{ MB}$)** | 物理 `ls -l` 字节统计 |
| **Tier 7 实测吞吐 (18-Core)** | $166.4\text{ MB/s}$ (L12单次) | **$\ge 1.5\text{ MB/s}$** ($> \text{advzip-4 } 0.71\text{ MB/s}$) | 单调时钟物理计时 |
| **RFC 1951 解压完整性** | 100% PASS | **100% PASS (0 CRC errors)** | `/usr/bin/unzip -t` |
| **Upstream 源码独立构建** | N/A | **`make` 100% PASS (0 warnings)** | 控制台构建退出码 0 |

---

## 3. Functional Requirements & Technical Boundaries

- **FR-01 (Vendor Upstream Placement)**：Google Zopfli 官方仓库必须完整克隆落盘于 `Vendor/zopfli-upstream/`，包含全部源文件与 git 元数据。
- **FR-02 (In-Process C Bridge Mounting)**：在 `Sources/CTTZipBridge/` 中无缝包含 Zopfli 核心源文件，通过 `ttzip_zopfli_engine.c` 暴露强类型 C 导出接口。
- **FR-03 (18-Core Multi-Block Parallel Orchestration)**：当压缩大文件（$\ge 16\text{MB}$）时，Swift 调度层将文件切分为 $18$ 个均匀数据块，各线程并发调用 `ZopfliDeflatePart`，且块间维持 32KB 滑动历史窗口。
- **FR-04 (RFC 1951 Stream Compliance)**：前 $N-1$ 块输出 `BFINAL=0` 与字节对齐标记，第 $N$ 块输出 `BFINAL=1`，确保标准解压器无缝穿透。
- **FR-05 (Deterministic Zero Fabrication)**：PK 测试必须在 `TTZIP_BENCH_ALL_LIVE=1` 模式下现场 100% 真实执行并更新实测图表。

---

## 4. Clarifications

*(本节将在 @speckit-clarify 阶段自动记录会话消歧与确界决策)*
