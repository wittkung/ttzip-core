# Feature Specification: Multi-Tier Compression Format Selection & Benchmark Architecture

## 1. 业务背景与问题定义 (Context & Problem Statement)

在数据压缩与归档工具的性能评估中，**测试格式（Format Selection）的选择直接决定了评测结果的客观性、代表性与公信力**。如果仅测试单一格式（例如只测 ZIP 或只测 7Z），将产生严重的评估偏差：
- 若只测 **ZIP (Deflate)**：无法反映现代大字典算法（Zstd/LZMA2）在海量结构化文本上的高压缩比能力，也无法反映多核并行流式管道的吞吐上限。
- 若只测 **7Z (LZMA2)**：测试过程高度偏向 CPU 密集型复杂匹配查找（Match Finder），掩盖了软件在轻量日常传输与微秒级高频解压场景下的 I/O 效率。
- 若只测 **TAR.ZST**：虽然吞吐极高，但在跨平台通用消费端（如 Windows 资源管理器、老旧邮件客户端）缺乏原生免安装支持。

因此，为了全面、科学、立体地反映软件在**不同业务场景、不同硬件瓶颈、不同算法特化**下的真实性能，TTZip 需要构建一套**4 阶代表性格式矩阵选型体系（4-Tier Representative Format Benchmark Matrix）**，并将其标准化注入到自动化 PK 跑分与帕累托图表生成管线中。

---

## 2. 4 阶代表性评测格式矩阵体系 (4-Tier Format Taxonomy)

```
                       ┌─────────────────────────────────────────────────────────┐
                       │           全场景代表性压缩格式矩阵 (4-Tier Matrix)         │
                       └─────────────────────────────────────────────────────────┘
                                                    │
         ┌───────────────────┬──────────────────────┴────────────────────┬───────────────────┐
         ▼                   ▼                                           ▼                   ▼
┌──────────────────┐┌──────────────────┐                       ┌──────────────────┐┌──────────────────┐
│ Tier 1: 通用兼容 ││ Tier 2: 极限归档 │                       │ Tier 3: 现代流式 ││ Tier 4: 极限吞吐 │
│    【 ZIP 】     ││     【 7Z 】     │                       │  【 TAR.ZST 】   ││    【 LZ4 】    │
├──────────────────┤├──────────────────┤                       ├──────────────────┤├──────────────────┤
│• 算法: Deflate   ││• 算法: LZMA2     │                       │• 算法: Zstd(FSE) ││• 算法: LZ4 Byte  │
│• 字典: 32KB      ││• 字典: 64MB~1GB  │                       │• 字典: 1MB~128MB ││• 字典: 64KB      │
│• 场景: 日常交换  ││• 场景: 冷备归档  │                       │• 场景: 现代云传输││• 场景: 内存I/O   │
│• 考验: 缓冲吞吐  ││• 考验: 匹配查找器│                       │• 考验: 零拷贝管道││• 考验: 总线带宽  │
└──────────────────┘└──────────────────┘                       └──────────────────┘└──────────────────┘
```

### 2.1 Tier 1: 通用生态与日常交换基准 (Universal Compatibility Tier)
- **代表格式**：`ZIP` (Level 1 Fast, Level 6 Standard)
- **核心算法**：Deflate (LZ77 + Static/Dynamic Huffman, 32KB Sliding Window)
- **评测价值**：
  - 测试软件在**全球通用标准容器**下的 Local Header、Central Directory 构建效率与 CRC32 计算吞吐；
  - 反映 macOS / Windows / iOS / Linux 跨平台文件互传的日常体验。

### 2.2 Tier 2: 极限空间与冷备归档基准 (Extreme Compression & Archive Storage Tier)
- **代表格式**：`7Z` (Level 1 Fast, Level 9 Ultra)
- **核心算法**：LZMA2 (Multi-core Block LZMA + 64MB~1GB Dictionary + Range Coder)
- **评测价值**：
  - 测试软件在**极限算力与大内存消耗**下的多核负载均衡、BT4/HC4 匹配查找器优化与冷备份极限体积压缩能力；
  - 反映大型数据集分发、云存储归档与低带宽长距离传输场景的体积压榨极限。

### 2.3 Tier 3: 现代工业级平衡与网络流式基准 (Modern Balanced & Cloud Streaming Tier)
- **代表格式**：`TAR.ZST` (Level 1 Real-time, Level 3 Default, Level 19 Max)
- **核心算法**：Zstandard (Finite State Entropy + Large Dictionary + Repcode)
- **评测价值**：
  - 测试软件在**现代万兆网络 (10Gbps) 与 NVMe SSD 直读直写**下的超高吞吐管道表现；
  - 反映现代数据中心、CI/CD 缓存、实时日志收集与容器镜像分发的综合效能。

### 2.4 Tier 4: 内存级极限与超高 IOPS 传输基准 (Memory-Speed & High-IOPS Tier)
- **代表格式**：`LZ4` / `TAR.LZ4` (Level 1 Fast)
- **核心算法**：Byte-aligned LZ4 (Token-based match/literal parsing)
- **评测价值**：
  - 测试软件在**接近 Apple Silicon 统一内存物理带宽极限**（解压突破 $30\sim 40\text{ GB/s}$）下的零拷贝与寄存器直通能力；
  - 反映进程间 IPC、实时数据库 WAL 日志压缩与极端微秒级延迟场景。

---

## 3. 功能需求 (Functional Requirements)

### FR-001: 标准化多阶格式选型配置器 (Standard Format Matrix Selector)
- 提供预设配置策略：
  - `--format-matrix=4tier` (默认推荐全矩阵：ZIP, 7Z, TAR.ZST, LZ4)
  - `--format-matrix=classic` (经典兼容矩阵：ZIP, 7Z, TAR.GZ)
  - `--format-matrix=modern` (现代化高吞吐矩阵：TAR.ZST, LZ4, BROTLI)
  - `--format-matrix=all16` (16 种格式全量拉通矩阵)

### FR-002: 多阶格式综合加权效能评分 (Multi-Tier Composite Performance Index)
- 构建科学的多维度评分公式（包含几何平均吞吐量、综合空间节省率与全链路耗时），杜绝单一格式偏袒：
  $$\text{Score}_{\text{composite}} = \sqrt[4]{\text{Speed}_{\text{ZIP}} \times \text{Speed}_{\text{7Z}} \times \text{Speed}_{\text{ZST}} \times \text{Speed}_{\text{LZ4}}}$$

### FR-003: 软件家族多格式能力雷达与 DeepSWE 帕累托图联动 (Multi-Format Trajectory Plotting)
- 在 DeepSWE 风格帕累托图中，自动将同一软件在 4 阶格式下的点位聚合为软件专属的能力演进曲线（Family Capability Curves）。
- 读者可一眼洞悉：
  - TTZip 在 **Tier 1 (ZIP)** 上比 7-Zip 快 28 倍；
  - TTZip 在 **Tier 3 (ZST)** 与 **Tier 4 (LZ4)** 上形成 $4\sim 9\text{ GB/s}$ 的统治级高地；
  - 7-Zip 在 **Tier 2 (7Z Ultra)** 上占据极限体积端点。

---

## 4. 非功能性约束与验收标准 (Success Criteria)

- **AC-001**：在 100MB Wikipedia 标准语料上执行 4-Tier 格式全矩阵测试，输出结构化的全场景多软件对比数据表。
- **AC-002**：DeepSWE 图表中清晰呈现各软件在 ZIP、7Z、TAR.ZST、LZ4 四大代表格式下的轨迹线，无视觉歧义。
- **AC-003**：基准测试执行时间控制在 15 秒以内，全自动化回归测试 100% 绿灯。
