# Data Model: Fast-LZMA2 Multi-Threaded Engine Integration

**Feature Directory**: `specs/055-fast-lzma2-engine-integration`

**Created**: 2026-08-17

**Status**: Defined

---

## 1. Core C Bridge Data Models (`CTTZipBridge`)

### `ttzip_fl2_engine_config_t`
C 级 Fast-LZMA2 引擎会话配置结构体，控制多线程并发、字典确界与路由策略。

| 字段名 | C 类型 | Swift 映射 | 必填 | 说明与确界约束 |
| :--- | :--- | :--- | :--- | :--- |
| `magic` | `uint32_t` | `UInt32` | 是 | 确界 Magic：固定为 `0x464C3243` ("FL2C") |
| `compression_level` | `int32_t` | `Int32` | 是 | 压缩等级：`1` 到 `9` |
| `dictionary_size` | `uint32_t` | `UInt32` | 是 | 字典大小字节数：`65536` 到 `67108864` (64KB ~ 64MB) |
| `thread_count` | `uint32_t` | `UInt32` | 是 | 并发工作线程数：`1` 到 `32` |
| `max_memory_budget_bytes` | `uint64_t` | `UInt64` | 是 | 内存确界字节上限：默认 `536870912` (512MB) |
| `enable_hybrid_neon_l1` | `bool` | `Bool` | 是 | 是否在 Level 1 自动路由至自研 ARM64 NEON Fast-Path |
| `enable_zero_block_bypass` | `bool` | `Bool` | 是 | 是否开启全零块 NEON 向量扫描与直通编码 |

---

### `ttzip_fl2_block_task_t`
单分块压缩任务入参与输出结果结构体。

| 字段名 | C 类型 | Swift 映射 | 必填 | 说明与确界约束 |
| :--- | :--- | :--- | :--- | :--- |
| `block_id` | `uint32_t` | `UInt32` | 是 | 任务分块索引号（从 0 开始自增） |
| `src_offset` | `uint64_t` | `UInt64` | 是 | 原始数据在全局流中的字节偏移量 |
| `src_length` | `size_t` | `Int` | 是 | 原始数据分块长度（字节，$\le 134217728$ 128MB） |
| `dst_capacity` | `size_t` | `Int` | 是 | 目标压缩缓冲区容量 |
| `compressed_size` | `size_t` | `Int` | 是 | 实际压缩产出字节数（0 表示未压缩或失败） |
| `out_dict_size` | `uint32_t` | `UInt32` | 是 | 实际生效并写入 LZMA2 头的字典大小 |
| `is_zero_block` | `bool` | `Bool` | 是 | 是否为全零数据块 |
| `status_code` | `int32_t` | `Int32` | 是 | 执行状态码：`0` 成功，负数为错误代码 |
| `crc32_checksum` | `uint32_t` | `UInt32` | 是 | 原始数据块计算得到的 CRC-32 校验和 |
| `elapsed_nanoseconds` | `uint64_t` | `UInt64` | 是 | 单块压缩耗时（纳秒） |

---

### `ttzip_fl2_stream_state_t`
流式压缩管道状态机上下文。

| 字段名 | C 类型 | Swift 映射 | 必填 | 说明与确界约束 |
| :--- | :--- | :--- | :--- | :--- |
| `magic` | `uint32_t` | `UInt32` | 是 | 状态机 Magic：固定为 `0x464C3253` ("FL2S") |
| `total_in_bytes` | `uint64_t` | `UInt64` | 是 | 累计已送入未压缩字节数 |
| `total_out_bytes` | `uint64_t` | `UInt64` | 是 | 累计已输出压缩字节数 |
| `is_finished` | `bool` | `Bool` | 是 | 是否已完成流式冲刷 (Flush End) |
| `active_workers` | `uint32_t` | `UInt32` | 是 | 当前活跃工作线程数 |
| `allocated_arena_bytes`| `size_t` | `Int` | 是 | 当前会话常驻 Arena 内存字节数 |

---

## 2. High-Level Swift Engine Models (`TTZipCore`)

### `SevenZipLZMA2HybridStrategy`
Swift 高层 7Z/XZ 压缩策略配置实体。

```swift
public struct SevenZipLZMA2HybridStrategy: Sendable {
    public let level: Int
    public let dictionarySize: Int
    public let threadBudget: Int
    public let maxMemoryBytes: Int
    public let routeMode: LZMA2RouteMode
}

public enum LZMA2RouteMode: String, Sendable, Codable {
    case neonFastPath       // Level 1: 手写 ARM64 NEON
    case fastLZMA2Parallel  // Level 3~9: Fast-LZMA2 多线程 Radix
    case zeroBlockBypass    // 全零块快速封装
}
```
