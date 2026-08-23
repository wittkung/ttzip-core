# Data Model: 大师级系统工程与 AI 调度落地数据模型 (Craftsmanship Engineering & AI Orchestration Data Model)

**Feature Branch**: `042-craftsmanship-engineering-and-orchestration`  
**Feature Directory**: `specs/042-craftsmanship-engineering-and-orchestration`  
**Created**: 2026-08-17  
**Status**: Active  

---

## 1. Entity: `CraftsmanshipStandardSpec`

定义 TTZip 系统工程的核心准则与物理约束。

| 字段名 | 类型 | 必填 | 约束 / 枚举 / 描述 |
| :--- | :--- | :--- | :--- |
| `specVersion` | `String` | 是 | 规范版本号，固定为 `"1.0.0"` |
| `philosophy` | `PhilosophyModel` | 是 | 核心工程哲学（"Less, but Better"） |
| `securityStandards` | `SecurityStandardsModel` | 是 | C/Swift 跨语言安全与物理确界规范 |
| `performanceStandards` | `PerformanceStandardsModel` | 是 | 热路径零分配与硬件直通规范 |
| `testingStandards` | `TestingStandardsModel` | 是 | 测试真理预言机与性能门禁规范 |

### 1.1 `PhilosophyModel`
- `zeroConfigurationCreep`: `Boolean` — 严禁暴露可内部自动决定的配置开关
- `decadeGradeLongevity`: `Boolean` — 模块自包含与十年免维护标准
- `subtractiveRefactoring`: `Boolean` — 重构首要清扫残留宏与冗余地层
- `pessimisticWorldModel`: `Boolean` — 悲观世界模型（Invariant-First & Bounds-First）

### 1.2 `SecurityStandardsModel`
- `deadStoreImmunity`: `String` — 凭据物理擦除方案（`"memset_s"` / `"secure_zero_memory"`）
- `integerClampPolicy`: `String` — 跨架构 64 位向 `size_t` 转换 Clamp 策略（`"SSIZE_MAX_CLAMP"`）
- `streamInputValidation`: `String` — 流式 I/O 防短读与 NULL 指针校验策略（`"STRICT_BOUNDS_CHECK"`）
- `structMagicLifecycle`: `String` — C 句柄魔数生命周期状态（`"MAGIC_INVAL_ON_FREE"`）

### 1.3 `PerformanceStandardsModel`
- `hotPathAllocationPolicy`: `String` — 热循环分配策略（`"ZERO_HEAP_ALLOCATION"`）
- `pageAlignmentBytes`: `Integer` — 物理页对齐字节数（`16384`，Apple Silicon 16KB）
- `concurrencyLockPolicy`: `String` — 并发闭包同步策略（`"LOCK_FREE_PER_WORKER"`）
- `hardwareAcceleration`: `Array<String>` — 硬件加速旁路列表（`["ARM_CRC32", "NEON_AES", "APFS_CLONE"]`）

### 1.4 `TestingStandardsModel`
- `crc32Oracle`: `String` — 原生数学预言机模型（`"BITWISE_BITCRC32"`）
- `goldenCorpusDecoder`: `String` — 历史缺陷语料库解码器（`"UUDECODER"`）
- `systemDifferential`: `Array<String>` — 系统级双向差分工具（`["/usr/bin/tar", "/usr/bin/unzip"]`）
- `regressionAuditor`: `String` — 性能倒退审计工具（`"scripts/audit_performance_regression.py"`）
- `hardRegressionThresholdPercent`: `Number` — 物理阻断门禁阈值（`-10.0`）

---

## 2. Entity: `AIOrchestrationCampaign`

定义全方位升级落地的四阶段 AI 调度战役模型。

| 字段名 | 类型 | 必填 | 约束 / 描述 |
| :--- | :--- | :--- | :--- |
| `campaignId` | `String` | 是 | 战役唯一标识（如 `"phase-1-c-bridge-hardening"`） |
| `phaseNumber` | `Integer` | 是 | 阶段编号（`1`, `2`, `3`, `4`） |
| `title` | `String` | 是 | 战役标题 |
| `targetComponents` | `Array<String>` | 是 | 目标组件路径列表（如 `["Sources/CTTZipBridge/"]`） |
| `aiDirectives` | `Array<String>` | 是 | 给 AI 的具体调度指令与审查军规 |
| `physicalGateAssertions` | `Array<String>` | 是 | 物理硬闸门断言条件（如 `swift build -Xcc -Werror`, `swift test`） |
| `status` | `String` | 是 | 状态：`"PLANNED"`, `"IN_PROGRESS"`, `"VERIFIED"`, `"CONVERGED"` |
