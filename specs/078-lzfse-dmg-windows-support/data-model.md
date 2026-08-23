# Data Model: 078-lzfse-dmg-windows-support

**Feature**: [078-lzfse-dmg-windows-support](file:///Users/kevintung/Documents/dev/TTZip/specs/078-lzfse-dmg-windows-support/spec.md)
**Status**: Completed (Phase 1)
**Date**: 2026-08-18

---

## 1. Core Data Entities

### 1.1 `LZFSEDecodeRequest`
表示针对单一 LZFSE / LZVN 压缩块或流的解码请求载荷。

| 字段名 | 物理类型 | 必填 | 约束 / 范围 | 描述 |
| :--- | :--- | :--- | :--- | :--- |
| `sourceLength` | `integer` (uint64) | 是 | $1 \le \text{len} \le 2,147,483,648$ (2GB) | 输入压缩字节缓冲区长度 |
| `destinationCapacity` | `integer` (uint64) | 是 | $1 \le \text{cap} \le 4,294,967,296$ (4GB) | 输出解压目标缓冲区最大容量 |
| `isStreamingBlock` | `boolean` | 是 | `true` 或 `false` | 是否为微缓冲流式分块解码模式 |
| `expectedOutputSize` | `integer` (uint64) | 否 | $\ge 0$ | 预期解压输出字节大小（如已知） |

### 1.2 `LZFSEDecodeResponse`
表示 LZFSE 解码操作返回的结果状态与物理计量指标。

| 字段名 | 物理类型 | 必填 | 约束 / 范围 | 描述 |
| :--- | :--- | :--- | :--- | :--- |
| `status` | `integer` (int32) | 是 | 枚举：`0` (OK), `-1` (Generic Error), `-2` (Corrupt), `-3` (OOM) | 底层 C 解码状态码 |
| `bytesWritten` | `integer` (uint64) | 是 | $\ge 0$ | 实际成功解压写入目标缓冲区的字节数 |
| `detectedCodec` | `string` | 是 | 枚举：`"LZFSE"`, `"LZVN"`, `"RAW"`, `"UNKNOWN"` | 实际识别并解码的数据块子类型 |
| `scratchAllocatedBytes` | `integer` (uint32) | 是 | 恒定为 `2129920` (2.03MB) | 所使用的线程局部 Scratch 空间大小 |

### 1.3 `DMGUDIFDescriptor`
表示从 DMG 尾部 `koly` trailer（512 字节）解析得出的全局元数据。

| 字段名 | 物理类型 | 必填 | 约束 / 范围 | 描述 |
| :--- | :--- | :--- | :--- | :--- |
| `signature` | `string` | 是 | 恒为 `"koly"` (0x6B6F6C79) | UDIF 尾部引导标识 |
| `version` | `integer` (uint32) | 是 | 恒为 `4` | UDIF 格式规范版本 |
| `headerSize` | `integer` (uint32) | 是 | 恒为 `512` | Trailer 字节尺寸 |
| `flags` | `integer` (uint32) | 是 | $\ge 0$ | 镜像状态标志（0x01 = Flattened） |
| `dataForkOffset` | `integer` (uint64) | 是 | 恒为 `0` | 数据分支在文件中的起始绝对偏移 |
| `dataForkLength` | `integer` (uint64) | 是 | $\ge 0$ | 数据分支物理字节长度 |
| `xmlOffset` | `integer` (uint64) | 是 | $\ge 0$ | 嵌入式 XML Property List (plist) 绝对字节偏移 |
| `xmlLength` | `integer` (uint64) | 是 | $\ge 0$ | 嵌入式 XML Property List (plist) 字节长度 |
| `sectorCount` | `integer` (uint64) | 是 | $\ge 1$ | 虚拟磁盘解压后逻辑总扇区数 (每扇区 512 字节) |
| `segmentNumber` | `integer` (uint32) | 是 | $\ge 1$ | 分卷序号 |
| `segmentCount` | `integer` (uint32) | 是 | $\ge 1$ | 分卷总数 |

### 1.4 `UDIFChunkBlock`
表示 `blkx` 块映射表中单个数据块的描述符（40 字节 `BLKXChunkEntry`）。

| 字段名 | 物理类型 | 必填 | 约束 / 范围 | 描述 |
| :--- | :--- | :--- | :--- | :--- |
| `entryType` | `integer` (uint32) | 是 | 枚举值（见下表） | 块压缩类型 ID |
| `entryTypeName` | `string` | 是 | 枚举：`"ZERO"`, `"RAW"`, `"IGNORE"`, `"ZLIB"`, `"BZIP2"`, `"LZFSE"`, `"LZMA"`, `"TERMINATOR"` | 块压缩类型人类可读名称 |
| `sectorNumber` | `integer` (uint64) | 是 | $\ge 0$ | 该块在分区内的起始逻辑扇区号 |
| `sectorCount` | `integer` (uint64) | 是 | $\ge 0$ | 该块覆盖的逻辑扇区数（解压大小 = 扇区数 $\times 512$） |
| `compressedOffset` | `integer` (uint64) | 是 | $\ge 0$ | 该块在 DMG 文件中的绝对压缩数据偏移 |
| `compressedLength` | `integer` (uint64) | 是 | $\ge 0$ | 压缩数据物理长度（字节） |

#### 块类型枚举映射表 (`entryType`):
- `0x00000000`: `ZERO` (零填充扇区)
- `0x00000001`: `RAW` (未压缩裸扇区)
- `0x00000002`: `IGNORE` (忽略空闲扇区)
- `0x80000005`: `ZLIB` (zlib/DEFLATE 压缩)
- `0x80000006`: `BZIP2` (传统 bzip2 压缩)
- `0x80000007`: `LZFSE` (Apple LZFSE / ULFO 压缩，现代规范标识为 0x80000006/0x80000007)
- `0x80000008`: `LZMA` (Apple LZMA / ULMO 压缩)
- `0xFFFFFFFF`: `TERMINATOR` (块表终止符)

### 1.5 `UDIFPartitionEntry`
表示从 DMG XML plist `resource-fork` -> `blkx` 解析出的单个逻辑分区。

| 字段名 | 物理类型 | 必填 | 约束 / 范围 | 描述 |
| :--- | :--- | :--- | :--- | :--- |
| `partitionId` | `string` | 是 | 非空字符串 | 分区标识符（如 `"0"`, `"1"`, `"-1"`） |
| `partitionName` | `string` | 是 | 非空字符串 | 分区物理标识（如 `"Apple_APFS"`, `"Apple_HFS"`, `"GPT Header"`） |
| `volumeName` | `string` | 是 | 字符串 | 卷标显示名称（CFName） |
| `startSector` | `integer` (uint64) | 是 | $\ge 0$ | 分区起始逻辑扇区号 |
| `sectorCount` | `integer` (uint64) | 是 | $\ge 1$ | 分区总逻辑扇区数 |
| `chunks` | `array[UDIFChunkBlock]`| 是 | 长度 $\ge 1$ | 组成该分区的有序数据块列表 |

### 1.6 `DMGExtractionProgress`
表示 DMG 穿透解压过程中的流式事件结构。

| 字段名 | 物理类型 | 必填 | 约束 / 范围 | 描述 |
| :--- | :--- | :--- | :--- | :--- |
| `state` | `string` | 是 | 枚举：`"reading_header"`, `"parsing_chunks"`, `"decompressing_sectors"`, `"extracting_files"`, `"completed"`, `"failed"` | 当前解压处理阶段 |
| `bytesProcessed` | `integer` (uint64) | 是 | $\ge 0$ | 当前已处理的压缩/未压缩字节数 |
| `totalBytes` | `integer` (uint64) | 是 | $\ge 1$ | 目标总字节数 |
| `currentChunkIndex` | `integer` (uint32) | 是 | $\ge 0$ | 正在解码的 Chunk 索引号 |
| `totalChunks` | `integer` (uint32) | 是 | $\ge 1$ | 总 Chunk 数量 |
| `currentFileName` | `string` | 是 | 字符串 | 当前正在提取的文件名或分区名 |
| `throughputMBs` | `number` (float64) | 是 | $\ge 0.0$ | 当前瞬时物理吞吐量 (MB/s) |

---

## 2. Invariants & Bounds

1. **零通配断言**：本数据模型中严禁出现未受约束的 `any`、`unknown` 或泛型字典 `Dict[str, Any]`。
2. **扇区对齐确界**：所有解压扇区大小严格按 $512$ 字节对齐，解压输出缓冲区必须为 $512$ 字节整数倍且支持 $16\text{KB}$ 内存对齐。
3. **Scratch 内存生命周期**：`LZFSEScratchArena` 物理生命周期绑定当前执行线程（Thread-Local），禁止跨线程共享或在 GCD 循环体内部重复 `malloc`/`free`。
4. **大端序一致性**：所有 `DMGUDIFDescriptor` 与 `BLKXChunkEntry` 字段在从二进制流解析时必须统一通过 `OSReadBigInt32` / `OSReadBigInt64` 或 `CFSwapIntXXBigToHost` 进行端序转换。
