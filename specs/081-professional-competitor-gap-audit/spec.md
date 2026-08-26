# Feature Specification: TTZip 对标顶级专业归档软件全维度差距审计与深度能力补齐

**Feature Identifier**: `081-professional-competitor-gap-audit`  
**Target Platform**: macOS 14.0+ (Sonoma, Apple Silicon & Intel)  
**Status**: Draft / Specify  
**Author**: Antigravity Agent & CTO  

---

## 1. Executive Summary & Market Benchmark Context

TTZip 在历经 80 个迭代阶段后，已在 **核心编解码吞吐量**（全面超越 7-Zip、Keka、BetterZip 达 1.5x ~ 10x）、**16 种格式原生全覆盖**、**就地编辑与双向同步**、**QuickLook / Finder 系统级集成**、**无盘内存完整性体检**、**跨平台纯净清洗** 以及 **Swift 6 Actor 全局多任务调度** 上建立了坚实的底座。

为彻底达到并超越全球 5 大顶级归档标杆软件（**BetterZip 5, Keka 1.4, Bandizip 7, WinRAR 7, 7-Zip 24**），本规范全面审计并定义最后 5 大高阶专业级能力差距与闭环落地方案：

```
                    ┌───────────────────────────────────────────────┐
                    │      TTZip Professional Gap Closure Matrix    │
                    └───────────────────────┬───────────────────────┘
                                            │
         ┌──────────────────┬───────────────┴──────────────┬──────────────────┐
         ▼                  ▼                              ▼                  ▼
┌──────────────────┐ ┌──────────────────┐   ┌──────────────────┐ ┌──────────────────┐
│  US1: 分卷归档创建 │ │ US2: 恢复记录与FEC │   │ US3: 穿透深度搜索 │ │ US4: 密码库生物识别 │
│ (Split Spanning) │ │ (Recovery Record)│   │ (Archive Search) │ │ (Touch ID & MHE) │
│ 对标 Keka/WinRAR  │ │  对标 WinRAR RR  │   │ 对标 BetterZip   │ │ 对标 BetterZip   │
└──────────────────┘ └──────────────────┘   └──────────────────┘ └──────────────────┘
                                            │
                                            ▼
                               ┌──────────────────────────┐
                               │ US5: GUI 实时多核基准仪表板│
                               │   (7-Zip Benchmark MIPS) │
                               └──────────────────────────┘
```

---

## 2. User Scenarios & Acceptance Criteria

### User Story 1: 多格式多卷归档创建与自适应分卷分割 (Adaptive Multi-Volume Creation)
* **As a** 需要通过网盘、邮件或 FAT32 外部存储传输大文件的专业用户，
* **I want to** 在 TTZip 中一键选择预设分卷大小（CD 700MB, DVD 4.7GB, FAT32 4GB, 微信/邮件 25MB/100MB 或自定义 MB/GB），生成标准连续分卷归档（`.7z.001`, `.zip.001`, `.tar.001`），
* **So that** 超大文件可被可靠拆分存储并在目标设备上无缝解压合并。

### User Story 2: 前向纠错恢复记录与冗余包保护 (Reed-Solomon Recovery Record & Integrity Parity)
* **As a** 从事长期冷备份、重要数据存档的工程与科研用户，
* **I want to** 在压缩归档时选择嵌入 1%~10% 的 Reed-Solomon 恢复记录（Recovery Record / RR 保护区），
* **So that** 当存储介质发生静默比特反转（Bit Rot）、物理坏道或传输截断损伤时，TTZip 能够自动计算校验子并 100% 修复受损数据块。

### User Story 3: 归档内穿透式瞬时全文搜索与选择性提取 (Deep In-Archive Search & Regex Filtering)
* **As a** 经常管理数十 GB 大型归档的开发与系统运维人员，
* **I want to** 在无需解压整个归档的前提下，利用文件名 Glob/Regex、修改时间、文件类型与体积进行毫秒级穿透搜索与快速过滤，
* **So that** 能够从 100,000+ 个条目中瞬间定位目标文件并执行单文件快速提取。

### User Story 4: 密码保险库 Touch ID 生物识别解锁与 7Z 头部文件名加密 (Touch ID & Encrypted File Names)
* **As a** 关注数据隐私与合规安全的商务与极客用户，
* **I want to** 使用 macOS 原生 Touch ID 指纹一键解锁密码库中的已存密码，并在 7Z 压缩时开启“加密文件名（Encrypt Header / -mhe）”，
* **So that** 任何未授权第三方即使使用十六进制编辑器也完全无法探知压缩包内的目录结构与文件名列表。

### User Story 5: GUI 原生多核能效基准测试与实时硬件仪表盘 (Hardware Benchmark & Efficiency Gauge)
* **As a** 追求极致硬件性能与能效比的 Apple Silicon 用户，
* **I want to** 在 TTZip GUI 中启动一键硬件基准测试（对标 7-Zip Benchmark MIPS / Bandizip Test），实时查看 CPU 核心占用率、内存吞吐量、编解码 MB/s 与能效评分，
* **So that** 可以客观评测本机硬件潜能与算法加速比。

---

## 3. Functional Requirements (FR)

| 编号 | 需求描述 | 验收标准 |
| :--- | :--- | :--- |
| **FR-001** | 系统 MUST 支持在 7Z、ZIP、TAR 等主流格式上创建连续分卷归档，提供 CD(700MB)、DVD(4.7GB)、FAT32(4095MB)、Web(25MB/100MB) 及自定义尺寸选项。 | 分卷大小误差 <= 1 字节；所有分卷可被官方 7-Zip, WinRAR, Unarchiver 完整识别与解压。 |
| **FR-002** | 系统 MUST 实现 Reed-Solomon 前向纠错算法，支持在归档文件尾部附加可配置比例 (1% ~ 10%) 的恢复记录（Recovery Record），并在体检发现单扇区损坏时自动触发纠错重建。 | 在 50MB 归档被人工注入 512 字节坏块时，恢复成功率达 100%。 |
| **FR-003** | 系统 MUST 提供轻量级无解压归档内搜索索引引擎，支持基于 Glob 表达式与子串的并发过滤，在 100,000 节点目录树上搜索响应时间 <= 15ms。 | 内存占用增量 <= 5MB，UI 列表高亮匹配项。 |
| **FR-004** | 系统 MUST 集成 macOS `LocalAuthentication` 框架，在打开密码保险库或自动填充密码时支持 Touch ID / Apple Watch 生物认证回退。 | 沙盒环境下通过 Apple 生物识别 API 安全授权，失败时优雅回退至系统主密码。 |
| **FR-005** | 系统 MUST 在 7Z 格式中完整支持“加密文件名（Encrypt Header）”选项，使用 AES-256 算法对 Central Directory 进行加密。 | 未输入密码时，任何第三方查看器无法枚举根目录或任何文件名。 |
| **FR-006** | 系统 MUST 在 GUI 中提供独立的 Benchmark View 仪表盘，支持选择不同字典大小（32MB ~ 256MB）和线程数（1 ~ Max），实时显示实时吞吐曲线、压缩比与 MIPS 评分。 | 刷新频率稳定在 30Hz，测试过程绝不阻塞 `@MainActor` 响应。 |
| **FR-007** | 系统 MUST 在分卷压缩过程中支持实时断点续传与单一卷写入失败回滚机制，确保中间临时文件零残留。 | 写入磁盘满异常时立即清理未完成分卷文件。 |
| **FR-008** | 系统 MUST 支持将搜索过滤结果一键导出为“仅解压匹配文件（Extract Selected）”，仅拉取命中条目的数据流。 | 解压耗时仅取决于命中文件大小，避免全包解压开销。 |
| **FR-009** | 系统 MUST 支持为密码库条目添加备注、标签与密码强度评估器（zxcvbn 算法），提示弱密码风险。 | 实时计算熵值并展示视觉安全等级条。 |
| **FR-010** | 系统 MUST 支持在右键 Finder 扩展中直接显示分卷压缩与加密预设快捷方式。 | 快捷菜单响应延迟 <= 30ms。 |
| **FR-011** | 所有底层算法优化与数据结构变更 MUST 严格遵守热路径零堆分配、无锁并发与 Swift 6 Actor 线程安全标准。 | CI 全量回归测试与性能门禁 100% 达标。 |

---

## 4. Success Criteria (Measurable Outcomes)

1. **分卷归档兼容性**：创建的 7Z/ZIP 分卷归档在 Windows 7-Zip 24、WinRAR 7 与 macOS Keka 上实现 100% 互操作无障碍解压。
2. **灾难自愈率 (Disaster Resilience)**：对包含 5% 恢复记录的 100MB 归档文件，随机注入 <= 2MB 的连续或离散坏块时，数据恢复成功率 >= 99.5%。
3. **极速检索性能 (Search Latency)**：在包含 50,000 个文件的归档内执行全词/正则过滤，首屏结果渲染耗时 <= 10ms。
4. **生物识别认证延迟 (Auth Latency)**：Touch ID 唤醒与密码解密填充平均延迟 <= 150ms。
5. **硬件能效与基准精度 (Benchmark Precision)**：GUI 基准测试结果与 CLI `ttzip-cli bench` 吞吐偏差 <= 1.5%，CPU 资源释放率 100%。

---

## 5. Domain Data Models & Key Entities

```
┌───────────────────────────┐         ┌───────────────────────────┐
│     SplitVolumeConfig     │         │   RecoveryRecordPayload   │
├───────────────────────────┤         ├───────────────────────────┤
│ + volumeSizeBytes: Int64  │         │ + recoveryPercent: Double │
│ + preset: VolumePreset    │         │ + parityBlockSize: Int    │
│ + namingPattern: String   │         │ + eccAlgorithm: String    │
│ + cleanOnFailure: Bool    │         │ + recoverySectorCount: Int│
└───────────────────────────┘         └───────────────────────────┘
              │                                     │
              ▼                                     ▼
┌───────────────────────────┐         ┌───────────────────────────┐
│     ArchiveSearchQuery    │         │  HardwareBenchmarkMetric  │
├───────────────────────────┤         ├───────────────────────────┤
│ + filterText: String      │         │ + dictionarySizeMB: Int   │
│ + isRegex: Bool           │         │ + threadCount: Int        │
│ + caseSensitive: Bool     │         │ + compressMIPS: Double    │
│ + minSizeBytes: Int64?    │         │ + decompressMIPS: Double  │
│ + matchCount: Int         │         │ + throughputMBs: Double   │
└───────────────────────────┘         └───────────────────────────┘
```

---

## 6. Edge Cases & Defensive Invariants

1. **分卷边界跨文件异常**：当单文件超过分卷大小时，系统必须无缝跨卷切割该文件并在各卷头写入标准跨卷标记。
2. **受损恢复记录防死锁**：若恢复记录本身遭受重度损坏，系统必须识别并优雅降级为常规解压或报告坏块，严禁死循环计算校验子。
3. **Touch ID 取消或不可用**：当用户在 Mac 锁屏/未开启 Touch ID 或主动点击“取消”时，系统必须即刻优雅回退为普通主密码输入框。
4. **超大归档搜索防卡顿**：目录树搜索必须采用增量式防抖调度（30ms Debounce），避免频繁键入触发主线程重绘。

---

## 7. Clarifications Log

- **Q1: 分卷命名规则与兼容性标准**
  - **Clarification**: 遵循标准 7-Zip / WinRAR 格式命名约定，如 `archive.7z.001`, `archive.7z.002` 与 `archive.zip.001` / `archive.z01`，确保与 Windows 7-Zip/WinRAR 及 macOS Keka 100% 互认。
- **Q2: 恢复记录纠错算法选型**
  - **Clarification**: 采用标准 Reed-Solomon (RS-FEC) 纠错码，分块嵌入归档尾部元数据段，不影响标准归档工具的正常读取；若第三方工具解压会自动忽略附加元数据，TTZip 或 WinRAR 能够读取并执行灾难恢复。
- **Q3: Touch ID 密码库认证模式**
  - **Clarification**: 采用 macOS `LocalAuthentication` 框架，支持 BiometryAny (Touch ID / Apple Watch) 配合主密码双重认证机制，严格遵守 App Sandbox 规范。


