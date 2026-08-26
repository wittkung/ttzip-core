# Data Model: LZ4 Engine and VFS Caching Subsystem

**Feature**: `063-lz4-engine-analysis`
**Created**: 2026-08-17
**Status**: Ready

---

## 1. Core Data Entities

### 1.1 LZ4BlockPayload
原始无状态 LZ4 内存数据块结构，代表单个独立压缩块。

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `uncompressedSize` | `integer` (uint32) | Yes | 原始未压缩数据长度（字节数，$\ge 0$） |
| `compressedSize` | `integer` (uint32) | Yes | 压缩后物理数据长度（字节数，$\ge 0$） |
| `acceleration` | `integer` (int32) | Yes | 压缩加速因子（默认 1，范围 $1 \sim 65537$） |
| `isCompressed` | `boolean` | Yes | 标识该块是否实际经过 LZ4 压缩（若膨胀则为 false） |
| `dataBufferBase64` | `string` | Yes | 块数据有效载荷（Base64 编码或内存裸指针句柄） |

---

### 1.2 LZ4FrameDescriptor
符合 LZ4 官方 Frame 规范（`0x184D2204`）的自包含帧元数据结构。

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `magicNumber` | `integer` (uint32) | Yes | 帧魔数（固定为 `407710212`，即十六进制 `0x184D2204`） |
| `blockIndependence` | `boolean` | Yes | 块独立性标志（true=块独立，false=跨块依赖） |
| `blockChecksumEnabled` | `boolean` | Yes | 是否启用单块 xxHash-32 校验和 |
| `contentChecksumEnabled` | `boolean` | Yes | 是否启用整帧 xxHash-32 内容校验和 |
| `blockMaxSizeId` | `integer` (int32) | Yes | 块最大容量档位（4=64KB, 5=256KB, 6=1MB, 7=4MB） |
| `contentSize` | `integer` (uint64) | No | 原始未压缩总尺寸（若帧头包含则有效，$\ge 0$） |
| `dictionaryId` | `integer` (uint32) | No | 静态字典 ID（若使用预置字典则有效） |

---

### 1.3 VFSTempCacheBlock
VFS 临时解压缓存池中的单分块索引与状态描述。

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `chunkIndex` | `integer` (uint32) | Yes | 缓存在归档全局解压流中的分块序号（从 0 开始） |
| `streamOffset` | `integer` (uint64) | Yes | 该分块在解压流中的逻辑起始字节偏移 |
| `rawSize` | `integer` (uint32) | Yes | 该分块原始解压大小（通常为 512KB ~ 1MB） |
| `compressedSize` | `integer` (uint32) | Yes | 该分块经 LZ4 压缩后在缓存池中的实际占用字节数 |
| `storageTier` | `string` (enum) | Yes | 存储层级：`"ram"` 或 `"disk_spill"` |
| `lastAccessTimestamp` | `integer` (uint64) | Yes | 上次访问单调时钟时间戳（纳秒，用于 LRU 驱逐） |
| `isDirty` | `boolean` | Yes | 脏页标志（是否已修改需要回写，预览为 false） |

---

### 1.4 TarSeekTableEntry
TAR.LZ4 归档穿透流式索引表条目，支撑无需全量解压即可随机预览。

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `entryPath` | `string` | Yes | 归档内相对规范路径（如 `"src/main.swift"`） |
| `tarHeaderOffset` | `integer` (uint64) | Yes | 512 字节 TAR 头部在未压缩流中的起始偏移 |
| `payloadOffset` | `integer` (uint64) | Yes | 条目文件内容在未压缩流中的绝对起始偏移 |
| `fileSize` | `integer` (uint64) | Yes | 文件实际大小（字节数） |
| `fileMode` | `integer` (uint32) | Yes | POSIX 权限模式（如 `0644`, `0755`） |
| `isDirectory` | `boolean` | Yes | 是否为目录条目 |
| `mtime` | `integer` (int64) | Yes | 文件修改时间戳（Unix Epoch 秒） |
