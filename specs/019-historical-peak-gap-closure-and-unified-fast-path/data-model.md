# Data Model & Schema Definitions (Feature 019)

**Feature**: Historical Peak Gap Closure & Unified Fast-Path Alignment  
**Directory**: `specs/019-historical-peak-gap-closure-and-unified-fast-path/`

---

## 1. Entities

### 1.1 `EntropyProbeResult`
表示文件头部信息熵快速探测结果。

| 字段名 | 类型 | 必填 | 说明 |
| :--- | :--- | :--- | :--- |
| `filePath` | `String` | 是 | 探测的目标文件路径 |
| `sampleBytes` | `Integer` | 是 | 探测采样字节数（固定 65536） |
| `shannonEntropy` | `Double` | 是 | 香农熵值（0.0 ~ 8.0 bit/byte） |
| `isHighEntropy` | `Boolean` | 是 | 是否判定为不可压缩高熵数据（$\ge 7.92$） |

---

### 1.2 `DirectRoutingDecision`
表示分发层路由策略裁决结果。

| 字段名 | 类型 | 必填 | 说明 |
| :--- | :--- | :--- | :--- |
| `format` | `String` | 是 | 目标归档格式 |
| `containsDirectory` | `Boolean` | 是 | 输入是否包含目录 |
| `engineType` | `String` | 是 | 路由选定引擎（`"C_NATIVE_DIRECT"`, `"PARALLEL_STREAM"`, `"IN_MEMORY_RAM"`） |
| `effectiveLevel` | `Integer` | 是 | 结合高熵探测后实际生效的压缩等级 |
