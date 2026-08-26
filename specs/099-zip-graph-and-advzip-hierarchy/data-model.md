# Phase 1 Data Model: ZIP 7-Tier Graph & Advzip Conquest

**Feature Directory**: `specs/099-zip-graph-and-advzip-hierarchy`  
**Created**: 2026-08-18  

---

## 1. Core Entities & Enums

### 1.1 `ZipGoldenTier`
定义 TTZip 7 大黄金梯队的元数据结构：

| 字段名 | 类型 | 必填 | 约束与说明 |
| :--- | :--- | :--- | :--- |
| `tier` | `Int` | 必填 | 取值范围 `1...7` |
| `name` | `String` | 必填 | 档位名称（如 `"1: 极速 (5.4 GB/s)"`、`"5: 图论近优 (200 MB/s)"`、`"7: 极限重压 (97.05%)"`） |
| `engineRawLevel` | `Int32` | 必填 | 底层 C 引擎调度级别（`2, 4, 5, 10, 11, 12, 15`） |
| `targetThroughputMBs` | `Double` | 必填 | 18 核心 Apple Silicon 目标吞吐（MB/s） |
| `expectedSavingsPct` | `Double` | 必填 | 100MB enwik8 预期空间节省率（%） |
| `expectedCompressedSizeMB` | `Double` | 必填 | 100MB enwik8 压缩后物理体积（MB） |

### 1.2 `ZipCompressionOption`
用户交互与 CLI 传入的参数模型：

| 字段名 | 类型 | 必填 | 说明 |
| :--- | :--- | :--- | :--- |
| `level` | `ArchiveCompressionLevel` | 必填 | 用户选择的档位（`.store` 或 `.level1` 至 `.level7`） |
| `format` | `ArchiveCompressionFormat` | 必填 | 固定为 `.zip` |
| `password` | `String?` | 可选 | ZIP 加密密码 |
| `splitVolumeSize` | `Int64?` | 可选 | 分卷大小（字节） |

---

## 2. Invariants & Validation Rules

1. **单调性不变式 (Monotonicity Invariant)**：
   - 空间节省率从 Tier 1 到 Tier 7 必须严格单调递增：
     $$\text{Savings}(\text{Tier } 1) < \text{Savings}(\text{Tier } 2) < \dots < \text{Savings}(\text{Tier } 7)$$
   - 压缩后文件物理大小从 Tier 1 到 Tier 7 必须严格单调递减：
     $$\text{Size}(\text{Tier } 1) > \text{Size}(\text{Tier } 2) > \dots > \text{Size}(\text{Tier } 7)$$
2. **Advzip-4 征服不变式 (Advzip Conquest Invariant)**：
   - Tier 7 在 100MB enwik8 上的压缩体积必须满足：
     $$\text{Size}(\text{Tier } 7) \le 2,994,957\text{ 字节} \quad (\text{advzip -4 基线})$$
