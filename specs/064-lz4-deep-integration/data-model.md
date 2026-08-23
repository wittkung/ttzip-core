# Data Model: LZ4 Deep Integration

**Feature**: `064-lz4-deep-integration`
**Created**: 2026-08-17

---

## 1. LZ4PerformanceRecord
性能基准实测记录实体。

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `benchmarkScenario` | `string` | Yes | 测试场景名称（如 `"LZ4 Compression"`, `"TAR.LZ4 Archive"`） |
| `payloadSizeMB` | `number` | Yes | 测试载荷尺寸（MB） |
| `compressedSizeMB` | `number` | Yes | 压缩后物理尺寸（MB） |
| `compressionRatio` | `number` | Yes | 压缩比（原始尺寸 / 压缩后尺寸） |
| `preThroughputMBs` | `number` | Yes | 优化前基线吞吐量（MB/s） |
| `postThroughputMBs` | `number` | Yes | 优化后实测吞吐量（MB/s） |
| `gainPercentage` | `number` | Yes | 性能提升幅度百分比（$\Delta\%$） |
| `status` | `string` | Yes | 审查状态：`"optimal"` / `"acceptable"` / `"regression"` |
