# Data Model: 全代码库规范与系统级不变量审计元数据模型 (Codebase Standards Audit Data Model)

**Feature Directory**: `specs/041-full-codebase-standards-audit`  
**Created**: 2026-08-17  
**Status**: Ready

---

## 1. 实体定义 (Entity Definitions)

### 1.1 DefectSeverity (缺陷严重级别枚举)
- **Type**: `String`
- **Allowed Values**:
  - `P0`: 阻塞级/致命安全漏洞（Zip-Slip、软链接逃逸、密码明文泄漏、解压算法损坏、模糊测试伪通过）
  - `P1`: 严重级/稳定性与资源失控（50GB+ OOM 隐患、全解包 SSD 磨损、深层递归栈溢出、强制解包崩溃、缺乏系统差分）
  - `P2`: 次要级/性能衰减与架构违规（热路径零填充中断、幽灵享元借还、跨层违规 import、缺乏硬件防溢出）
  - `P3`: 规范级/代码异味与文档改进（类型双重转换、冗余接口、过时 API、文档注释同步）

### 1.2 InvariantCategory (系统工程铁律与规范维度枚举)
- **Type**: `String`
- **Allowed Values**:
  - `StreamFirst`: 流式微缓冲拉取管道与零内存假设
  - `InvariantFirst`: POSIX 级路径安全防护、延后 Fixup 倒序回写与 TOCTOU 免疫
  - `BoundsFirst`: 结构体 Magic 哨兵、memset_s 敏感内存擦除与 SSIZE_MAX Clamp
  - `OracleFirst`: 真实历史缺陷语料库、系统 CLI 双向差分与崩溃优先模糊测试
  - `DesignPattern`: 28 大设计模式热路径隔离与规范实现
  - `LayerIsolation`: 表现层、核心引擎层与底层 C 桥接层的单向架构依赖

### 1.3 AuditDefectItem (单项缺陷实体)
- **Fields**:
  - `id`: `String` (Pattern: `^(P0|P1|P2|P3)-[0-9]{2}$`) — 缺陷唯一编号，如 `P0-01`
  - `severity`: `DefectSeverity` — 缺陷严重级别
  - `category`: `InvariantCategory` — 所属铁律/规范维度
  - `modulePath`: `String` — 源码相对路径（如 `Sources/CTTZipBridge/CTTZipBridge_7zSolid.c`）
  - `startLine`: `Integer` (>= 1) — 缺陷起始代码行号
  - `endLine`: `Integer` (>= startLine) — 缺陷结束代码行号
  - `title`: `String` — 缺陷简明标题
  - `impactDescription`: `String` — 缺陷潜在危害与根因剖析
  - `remediationPlan`: `String` — 针对性修复建议与技术方案
  - `frozenFile`: `Boolean` — 是否涉及处于冻结状态的文件

### 1.4 CodebaseAuditReport (综合审计全景报告实体)
- **Fields**:
  - `reportVersion`: `String` — 报告规范版本（如 `1.0.0`）
  - `auditDate`: `String` (Format: `YYYY-MM-DD`) — 审计完成日期
  - `totalFilesScanned`: `Integer` (>= 1) — 全量扫描源文件总数
  - `summary`: `Object`:
    - `totalDefects`: `Integer` — 发现缺陷总数
    - `p0Count`: `Integer` — P0 级缺陷数
    - `p1Count`: `Integer` — P1 级缺陷数
    - `p2Count`: `Integer` — P2 级缺陷数
    - `p3Count`: `Integer` — P3 级缺陷数
  - `defects`: `Array<AuditDefectItem>` — 详细缺陷条目列表
  - `roadmapPhases`: `Array<Object>`:
    - `phaseIndex`: `Integer` (>= 1) — 重构阶段编号
    - `phaseTitle`: `String` — 阶段目标与主题
    - `targetDefectIds`: `Array<String>` — 本阶段治理的缺陷编号列表
    - `priority`: `String` — 阶段紧迫度

---

## 2. 一致性核对 (Bidirectional Consistency Check)

- `AuditDefectItem` 与 `contracts/codebase_audit_spec.json` 的 definitions 完全对应。
- `CodebaseAuditReport` 与根 JSON Schema 字段定义、必填性与类型严格 1:1 镜像。
