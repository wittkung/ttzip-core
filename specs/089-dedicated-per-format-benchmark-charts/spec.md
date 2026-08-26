# Feature Specification: Dedicated Per-Format Benchmark Charts & Multi-Software Suite

## 1. 业务背景与问题定义 (Context & Problem Statement)

在此前的综合帕累托图表中，存在两大用户体验痛点：
1. **Apple Native 仅有一个点**：仅测试了 `/usr/bin/ditto`，未覆盖 macOS 自带的 `/usr/bin/zip -1`（极速）、`/usr/bin/zip -6`（标准）、`/usr/bin/tar` 等原生工具链，导致 Apple 生态在图表上无法形成完整的演进轨迹。
2. **多格式混在同一张图容易失焦**：将 ZIP、7Z、TAR.ZST、LZ4 全部放在同一张图表虽然能看全局，但用户在评估特定格式（例如“在 ZIP 格式下各软件谁最快”、“在 7Z 格式下各软件压缩比如何”）时，不同格式的点位相互交错。

因此，本特性将基准测试与图表渲染系统升级为：
- **按格式独立出图（One Dedicated Pareto Chart Per Format）**：分别为 **ZIP 专场**、**7Z 专场**、**TAR.ZST 现代流式专场**、**LZ4 极速专场** 生成专属高清图表；
- **全方位扩充竞品软件与 Apple 原生工具链点位**：
  - Apple Native：`Apple ditto (ZIP)`、`Apple zip -1 (Fast)`、`Apple zip -6 (Normal)`、`Apple tar`
  - 7-Zip 26.02：`7-Zip ZIP Fast`、`7-Zip ZIP Normal`、`7-Zip 7Z Fast`、`7-Zip 7Z Normal`、`7-Zip 7Z Ultra`
  - TTZip：对应格式的多档位原生引擎。

---

## 2. 独立格式图表矩阵设计 (Dedicated Chart Matrix)

| 图表工件文件名 | 专场格式 | 参战软件与点位 | 专场核心看点 |
| :--- | :--- | :--- | :--- |
| **`pareto_pk_zip.png`** | **ZIP 格式专场** | • **TTZip** (ZIP Fast L1, ZIP Normal L6)<br>• **7-Zip** (ZIP Fast -mx=1, ZIP Normal -mx=6)<br>• **Apple Native** (ditto, zip -1, zip -6) | **通用生态王座**：同在标准 ZIP 格式下，TTZip 相比 7-Zip 和 Apple 原生提速 **2.5x ~ 28x** 的垂直压制线。 |
| **`pareto_pk_7z.png`** | **7Z 格式专场** | • **TTZip** (7Z Fast L1, 7Z Normal L5)<br>• **7-Zip 26.02** (7Z Fast -mx=1, 7Z Normal -mx=5, 7Z Ultra -mx=9) | **极限算力与压缩比**：在复杂 LZMA2 字典下，TTZip 原生 C 引擎与 7-Zip 官方 ARM64 二进制的单调前沿对决。 |
| **`pareto_pk_tar_zst.png`** | **TAR.ZST 专场** | • **TTZip** (TAR.ZST Direct L1, L3)<br>• 官方 zstd / 7-Zip 扩展 | **现代流式吞吐**：在万兆网络与 NVMe 极速归档场景下的超高吞吐（>8,000 MB/s）表现。 |
| **`pareto_pk_lz4.png`** | **LZ4 专场** | • **TTZip** (LZ4 Direct In-Memory)<br>• 官方 lz4 / 系统引擎 | **内存总线极限**：逼近 Apple Silicon 统一内存带宽极限（>9,000 MB/s）的微秒级响应。 |
| **`software_pareto_pk.png`**| **4-Tier 全景图** | 全软件、全格式综合全景图（含 GMean 综合评分） | 全局宏观统揽。 |

---

## 3. 功能需求 (Functional Requirements)

- **FR-001**：在 `SoftwareParetoFrontierPkTests` 中，分别执行各格式专场测试，并独立导出对应的 `.png` 与 `.svg`。
- **FR-002**：扩充 Apple Native 测试矩阵，完整测量 `/usr/bin/ditto` 与 `/usr/bin/zip`（`-1`、`-6`）的真实吞吐与压缩率。
- **FR-003**：单格式图表自适应 X 轴压缩率与 Y 轴吞吐速度，保留纯白学术背景、水平单向网格线、Hero 蓝色药丸卡片与 `most efficient ↗` 引导。
- **FR-004**：更新 Markdown 报告工件，结构化内嵌各格式专属图表。
