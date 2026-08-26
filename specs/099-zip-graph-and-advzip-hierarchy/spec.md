# Feature Specification: ZIP 7-Tier Graph-Theoretic Shortest-Path & Ultimate Advzip-4 Conquest Hierarchy

**Feature ID**: `099-zip-graph-and-advzip-hierarchy`  
**Status**: Draft  
**Author**: Antigravity CTO / Spec Kit Autonomous Pipeline  
**Target Platform**: macOS 14.0+ (Apple Silicon NEON SIMD & Multi-Core DAG Acceleration)  
**Created**: 2026-08-18  

---

## 1. Executive Summary & First Principles

TTZip 的 ZIP 格式当前在极速多核档位（5.4 GB/s）和超级压缩档位（3.0 MB/s @ 97.01%）已展现出强大性能。然而，在现有 4 级（1.8 GB/s @ 96.65%）与 5 级（3.0 MB/s @ 97.01%）之间存在长达 2.5 个数量级的速度与压缩比断层；同时，在极限冷备份场景下，现有最高档位尚未对 `advzip -4`（7z Deflate 15 轮次深度迭代）形成决定性超越。

本规范旨在构建 **TTZip ZIP 7 大黄金梯队体系（7-Tier Golden Hierarchy）**：
1. **在 4 级与 5 级之间引入【近优图论最短路径解析器 (Graph DAG Near-Optimal Parser)】**：在维持 >100 MB/s 高速多核吞吐的前提下，利用有限前瞻窗口的 DAG 最短路径动态规划，将压缩率提升至 ~96.85%；
2. **在最高层引入【极限多轮次位流迭代与动态块切分器 (Multi-Pass Iterative Deflate & Dynamic Block Splitter)】**：通过多轮次动态 Huffman 树权重再平衡与自适应块边界切分，实现对标并超越 `advzip -4` 的物理压缩率（$\ge 97.02\%$）。

---

## 2. User Scenarios & Personas

### Scenario 1: 日常高频与大文件即时交付 (User Tiers 1-3)
- **用户画像**: 开发者、音视频创作者。
- **行为**: 快速打包 1GB~100GB 源码或媒体资产。
- **预期体验**: 依靠 Level 1 (5.4 GB/s)、Level 2 (3.8 GB/s)、Level 3 (3.2 GB/s)，在秒级内完成打包，UI 零卡顿。

### Scenario 2: 兼顾传输带宽与高速打包的平衡流 (User Tier 5 - NEW)
- **用户画像**: CI/CD 流水线管理员、游戏资源打包工具。
- **行为**: 打包每日构建 (Nightly Build) 或云端分发包，无法容忍单核龟速（几分钟），但极度渴望比常规 Deflate 多节省 10%~20% 剩余空间。
- **预期体验**: 选择 Level 5（图论高速档），多核满载跑出 **150~400 MB/s**，体积直逼极限 Zopfli，比标准 zip -9 小 5% 以上。

### Scenario 3: 极限冷备份与最终发布包 (User Tiers 6-7 - NEW Tier 7)
- **用户画像**: 软件发布负责人、只读归档管理员。
- **行为**: 构建向全球数百万用户分发的 App DMG / 安装包 ZIP。
- **预期体验**: 选择 Level 7（极限重压档），经过多轮次深度图论迭代与最优块切分，压缩率击穿现有所有工具（包含 `advzip -4`），获得全球最小的 RFC 1951 兼容 ZIP。

---

## 3. Product Hierarchy Matrix (7 大黄金档位)

| 档位 (Level) | 档位名称 | 核心技术引擎 | 预期吞吐 (18-Core) | 预期空间节省率 (enwik8) | 典型应用场景 |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Level 1** | ⚡ 极速首选 (Fastest) | `libdeflate` L2 + NEON SIMD 分块并发 | **$5,200 \sim 5,600\text{ MB/s}$** | $\sim 95.50\%$ | 日常超大文件秒压 |
| **Level 2** | 🚀 快速顺滑 (Fast) | `libdeflate` L4 + 启发式快速剪枝 | **$3,600 \sim 4,000\text{ MB/s}$** | $\sim 96.30\%$ | 常用归档默认推荐 |
| **Level 3** | ⚖️ 标准平衡 (Normal) | `libdeflate` L5 + 中度哈希搜索 | **$3,000 \sim 3,400\text{ MB/s}$** | $\sim 96.55\%$ | 标准平衡场景 |
| **Level 4** | 📦 深度高压 (Maximum) | `libdeflate` L10 + 深度贪心解析 | **$1,600 \sim 2,000\text{ MB/s}$** | $\sim 96.65\%$ | 传统最高单核/多核比率 |
| **Level 5** | 🧩 图论近优 (Graph Fast) *(NEW)* | 18 核心有限前瞻 DAG 最短路径动态规划 | **$150 \sim 400\text{ MB/s}$** | **$\sim 96.85\%$** | **CI/CD 与高性能分发包** |
| **Level 6** | 💎 超级压缩 (Ultra Zopfli) | 18 核心全局最短路径穷举图论解析器 | **$2.5 \sim 4.0\text{ MB/s}$** | **$\sim 97.01\%$** | 高价值冷归档 |
| **Level 7** | 🏆 极限重压 (Extreme Peak) *(NEW)* | 15 轮次动态 Huffman 权重重平衡 + 最优块切分 | **$0.3 \sim 1.0\text{ MB/s}$** | **$\ge 97.02\% \sim 97.05\%$** | **超越 advzip-4 终极发布包** |

---

## 4. Functional Requirements

### FR-001: 7 档位全系统语义枚举与向下兼容
- `ArchiveCompressionLevel` 必须原生支持 `.level1` 至 `.level7` 的 7 档位映射；
- 旧版 level 8..12 配置在加载时必须无缝自动映射至新的 1~7 黄金档位，保证历史存档与 CLI 参数兼容。

### FR-002: Level 5 高速图论近优引擎 (Graph Fast DAG Parser)
- 基于有向无环图 (DAG) 与 Viterbi 动态规划算法，将 32KB 滑动窗口内字符序列建模为边权（Cost = 位长度估计）；
- 采用 **有限前瞻剪枝 (Bounded Lookahead Pruning)**：前瞻深度限制在 $K \in [8, 32]$，在多核并行下将吞吐锁定在 $\ge 150\text{ MB/s}$，压缩率突破 96.80%。

### FR-003: Level 7 极限多轮重平衡引擎 (Extreme Advzip-4 Buster)
- 实现多轮次（8~15 次迭代）统计频率收集与 Huffman 码长重新赋值；
- 实现 **动态 Deflate 块最优切分算法 (Optimal Block Splitting)**：基于局部熵变识别最佳切分点，彻底消除固定块带来的码表冗余；
- 在 100MB 真实语料（enwik8）下，输出压缩后体积必须严格小于或等于 `advzip -4`（$\le 2.99\text{ MB}$）。

### FR-004: UI 与 CLI 交互无缝对齐
- SwiftUI / AppKit 压缩配置面板在选择 `.zip` 时，Tile 列表与 Slider 严格展示 1~7 档，并标注清晰的吞吐与比率预估；
- 帕累托基准测试图表生成器自动以 7 档位为基准生成帕累托前沿曲线。

---

## 5. Success Criteria & Verification

- **SC-001 (Level 5 性能门禁)**: 18 核心多核吞吐 $\ge 150\text{ MB/s}$，空间节省率 $\ge 96.80\%$。
- **SC-002 (Level 7 极限超越门禁)**: 空间节省率 $\ge 97.015\%$，输出体积绝对值 $\le \text{advzip -4}$ 体积。
- **SC-003 (帕累托单调性断言)**: 从 Level 1 到 Level 7，空间节省率必须严格单调递增（$S_1 < S_2 < S_3 < S_4 < S_5 < S_6 \le S_7$），且吞吐量在各阶段形成合理的台阶。
- **SC-004 (RFC 1951 格式合规性)**: Level 5 和 Level 7 生成的所有 ZIP 归档，必须 100% 通过 `/usr/bin/unzip -t`、`7z t` 与系统 Archive Utility 的解压校验。
