# Implementation Plan: 081-professional-competitor-gap-audit

**Feature**: TTZip 对标顶级专业归档软件全维度差距审计与深度能力补齐  
**Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/081-professional-competitor-gap-audit/spec.md)  
**Status**: In Planning  

---

## 1. Technical Context

TTZip 已具备顶尖的底层编解码吞吐性能与基础 macOS 集成能力。本 Feature 聚焦攻克专业归档领域的 5 大高阶维度：
1. **连续多卷分卷归档（Split Spanning）**：支持标准分卷流式切割与写入器。
2. **Reed-Solomon 前向纠错恢复记录（Recovery Record / RR）**：嵌入透明 FEC 元数据段与灾难自愈引擎。
3. **极速穿透检索与选择性提取（In-Archive Search & Selective Extract）**：毫秒级树节点扁平化索引与按需解压。
4. **密码保险库 Touch ID 解锁与 7Z 头部文件名加密（-mhe）**：macOS LocalAuthentication 生物认证集成与 7Z AES-256 加密头。
5. **GUI 原生多核能效基准仪表盘（Hardware MIPS Gauge）**：对标 7-Zip Benchmark MIPS，实时 CPU/内存吞吐与能效曲线渲染。

---

## 2. Constitution & Invariant Checks

| 铁律检查项 | 约束规则 | 验证与应对方案 |
| :--- | :--- | :--- |
| **热路径零堆分配** | 编解码与流式分卷、FEC 运算热路径严禁动态对象树与额外内存拷贝 | 分卷切割在 C/POSIX 句柄层进行零拷贝切分；RS-FEC 采用固定栈/页缓冲区运算 |
| **Fast-Path 保留** | 不得以通用慢路径降级现有专用快速通道 | 分卷与 FEC 仅作为独立选项在写入管道首尾挂载，不污染主编解码器 |
| **确定性与线程安全** | 所有多任务、搜索与基准计算面向 Swift 6 并发模型与 Actor 隔离 | 基准测试在后台 detached Task 执行，UI 状态更新汇聚于 `@MainActor`，密码与搜索状态使用 Actor 隔离 |
| **零裸对象契约** | `contracts/` 下所有 JSON Schema 统一 Draft-07，严禁裸 `type: object` | 建立 `split-volume-config.json`, `recovery-record-payload.json`, `archive-search-query.json`, `hardware-benchmark-metric.json` 等强类型契约 |

---

## 3. Phase 0: Research Items

- R001 [SUBAGENT:research] 《多卷连续分卷归档创建架构》：研究标准 7-Zip / PKZIP / TAR 分卷切割算法与跨平台解压兼容性。
- R002 [SUBAGENT:research] 《Reed-Solomon 恢复记录与 FEC 嵌入协议》：研究 GF(2^8)/GF(2^16) 前向纠错算法与透明归档尾部元数据段格式。
- R003 [SUBAGENT:research] 《归档内瞬时穿透检索与选择性流式提取》：研究 100k 节点扁平化倒排索引与单流按需解压管道。
- R004 [SUBAGENT:research] 《Touch ID 生物识别认证与 7Z 加密头》：研究 macOS LocalAuthentication 与 7Z AES-256 目录加密规范。
- R005 [SUBAGENT:research] 《GUI 原生多核 MIPS 能效基准仪表盘》：研究 7-Zip MIPS 评分算法与 SwiftUI 30Hz 动态遥测管道。

---

## 4. Phase 1: Design Artifacts

- [x] `data-model.md`: 领域模型与数据结构定义
- [x] `contracts/`: JSON Schema 契约体系
  - `contracts/split-volume-config.json` [SUBAGENT:research]
  - `contracts/recovery-record-payload.json` [SUBAGENT:research]
  - `contracts/archive-search-query.json` [SUBAGENT:research]
  - `contracts/hardware-benchmark-metric.json` [SUBAGENT:research]
- [x] `quickstart.md`: 验收场景与验证指南

---

## 5. Planned Changes by Component

### Component 1: `Sources/TTZipCore/` (Engine Layer)
- `Sources/TTZipCore/Split/SplitVolumeWriter.swift`: 分卷切分与连续卷流式写入器。
- `Sources/TTZipCore/Security/ReedSolomonFEC.swift`: Reed-Solomon 前向纠错计算与恢复记录生成器。
- `Sources/TTZipCore/Search/ArchiveSearchEngine.swift`: 归档内穿透检索与 Glob/Regex 快速过滤器。
- `Sources/TTZipCore/Security/TouchIDAuthenticator.swift`: macOS Touch ID / Apple Watch 生物识别认证器。
- `Sources/TTZipCore/Benchmark/MIPSHardwareBenchmarkEngine.swift`: 7-Zip 对齐的多核 MIPS 基准测试引擎。

### Component 2: `Sources/TTZipApp/` (GUI Layer)
- `Sources/TTZipApp/Views/ArchiveExplorerView.swift`: 集成顶栏即时穿透搜索框与按需解压右键菜单。
- `Sources/TTZipApp/Views/CompressModalView.swift`: 增加分卷大小选择器与“添加恢复记录（1%~10%）”与“加密文件名”开关。
- `Sources/TTZipApp/Views/BenchmarkDashboardView.swift`: 全新现代化硬件多核基准测试仪表盘。
- `Sources/TTZipApp/Views/PasswordVaultManagerView.swift`: 接入 Touch ID 一键生物识别解锁。

### Component 3: `Tests/TTZipTests/` (Test Layer)
- `Tests/TTZipTests/SplitVolumeCreationTests.swift`: 分卷创建与跨平台解压单测。
- `Tests/TTZipTests/ReedSolomonRecoveryRecordTests.swift`: 人工坏块注入与灾难自愈纠错测试。
- `Tests/TTZipTests/InArchiveSearchEngineTests.swift`: 100k 节点检索延迟与正则过滤测试。
- `Tests/TTZipTests/MIPSBenchmarkEngineTests.swift`: 多核 MIPS 评分与硬件基准单测。
