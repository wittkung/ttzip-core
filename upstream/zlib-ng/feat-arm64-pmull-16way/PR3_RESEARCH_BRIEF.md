# PR 3 预研与攻坚案卷：ARM64 16-Way PMULL+EOR3 CRC-32 超标量硬件加速

## 一、 背景与现有上游状态
在 `zlib-ng` 中，PR #2023 引入了基于 Peter Cawley `fast-crc32` 的 3-way 标量 + 9-way PMULL 向量交织实现 (`arch/arm/crc32_armv8_pmull_eor3.c`)。
在现代 8~10 发射宽度的 Apple Silicon (M1~M5) 及 AWS Graviton 3/4 (Neoverse V2) 处理器上，当前物理实测吞吐为 **~11.35 GB/s**。

## 二、 优化路径推导与数学常数 (Mathematically Grounded Constants)
通过多项式模指数算法 $x^N \pmod P$ ($P = \text{0x104c11db7}$)，针对不同折叠步长的常数矩阵已验证完全精确：

| 折叠跨度 | $k_0 = x^{8N-33} \pmod P$ | $k_1 = x^{8N+31} \pmod P$ | 适用场景 |
| :--- | :--- | :--- | :--- |
| **16 字节** | `0xccaa009e` | `0xae689191` | 树状归约末级折叠 |
| **32 字节** | `0x81256527` | `0xf1da05aa` | 2 向量合并 |
| **64 字节** | `0x1d9513d7` | `0x8f352d95` | 4 向量合并 |
| **128 字节** | `0x910eeec1` | `0x33fff533` | 8 向量合并 |
| **144 字节** | `0x3f41287a` | `0x26b70c3d` | 现有 9 向量主循环 |
| **256 字节** | `0xe95c1271` | `0xce3371cb` | 16 向量超宽主循环 |

## 三、 本地基准与验证计划
- 验证套件：`scratch/test_pmull_folding.c` 与 `scratch/bench_pmull_16way.c`；
- 基线吞吐：11.35 GB/s；
- 目标吞吐：25+ GB/s；
- 推进时机：待 PR 1 (#2415) 与 PR 2 (#2416) 正式合并后发起。
