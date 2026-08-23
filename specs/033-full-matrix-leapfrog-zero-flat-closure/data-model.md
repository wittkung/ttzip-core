# Data Model: 全矩阵清零持平、波动与倒退并全面大幅跃升 (Feature 033)

**Entities**: 100% 强类型实体映射与契约一致性约束

---

## 1. `InProcessStreamConfig` (进程内流式编解码器配置)

| 字段名 | 类型 | 必填 | 描述 | 契约对齐 |
| :--- | :--- | :--- | :--- | :--- |
| `format` | `String` | 是 | 归档格式 (lz4 / lzip / lrzip / tar.xz) | `contracts/all_green_closure.schema.json#/definitions/InProcessStreamConfig/properties/format` |
| `compressionLevel` | `Int` | 是 | 压缩等级 (1 ~ 9) | `contracts/all_green_closure.schema.json#/definitions/InProcessStreamConfig/properties/compressionLevel` |
| `threadCount` | `Int` | 是 | 并发工作线程数 (1 ~ 64) | `contracts/all_green_closure.schema.json#/definitions/InProcessStreamConfig/properties/threadCount` |
| `chunkSizeBytes` | `Int` | 是 | 块大小 (例如 65536 或 1048576) | `contracts/all_green_closure.schema.json#/definitions/InProcessStreamConfig/properties/chunkSizeBytes` |
| `useFastMode` | `Bool` | 是 | 是否开启硬件快速匹配模式 | `contracts/all_green_closure.schema.json#/definitions/InProcessStreamConfig/properties/useFastMode` |

---

## 2. `CryptoDispatchRoute` (加密归档分发路由)

| 字段名 | 类型 | 必填 | 描述 | 契约对齐 |
| :--- | :--- | :--- | :--- | :--- |
| `archivePath` | `String` | 是 | 归档物理路径 | `contracts/all_green_closure.schema.json#/definitions/CryptoDispatchRoute/properties/archivePath` |
| `isEncrypted` | `Bool` | 是 | 是否加密归档 | `contracts/all_green_closure.schema.json#/definitions/CryptoDispatchRoute/properties/isEncrypted` |
| `engineType` | `String` | 是 | 分派引擎 (`SevenZipEngine` / `ZipParallelExtractor`) | `contracts/all_green_closure.schema.json#/definitions/CryptoDispatchRoute/properties/engineType` |
| `useNeonSimd` | `Bool` | 是 | 是否启用 ARM NEON SIMD 硬件解密 | `contracts/all_green_closure.schema.json#/definitions/CryptoDispatchRoute/properties/useNeonSimd` |

---

## 3. `BenchmarkClosureAudit` (基准测试收敛审计记录)

| 字段名 | 类型 | 必填 | 描述 | 契约对齐 |
| :--- | :--- | :--- | :--- | :--- |
| `totalDimensions` | `Int` | 是 | 全矩阵细分维度总数 (246) | `contracts/all_green_closure.schema.json#/definitions/BenchmarkClosureAudit/properties/totalDimensions` |
| `improvedCount` | `Int` | 是 | 提升项数量 (> +3.0%) | `contracts/all_green_closure.schema.json#/definitions/BenchmarkClosureAudit/properties/improvedCount` |
| `flatCount` | `Int` | 是 | 持平项数量 (±3.0%) | `contracts/all_green_closure.schema.json#/definitions/BenchmarkClosureAudit/properties/flatCount` |
| `warningCount` | `Int` | 是 | 波动项数量 (-3.0% ~ -10.0%) | `contracts/all_green_closure.schema.json#/definitions/BenchmarkClosureAudit/properties/warningCount` |
| `criticalCount` | `Int` | 是 | 倒退项数量 (< -10.0%) | `contracts/all_green_closure.schema.json#/definitions/BenchmarkClosureAudit/properties/criticalCount` |
| `allPassed` | `Bool` | 是 | 全量测试是否全部绿灯通过 | `contracts/all_green_closure.schema.json#/definitions/BenchmarkClosureAudit/properties/allPassed` |
