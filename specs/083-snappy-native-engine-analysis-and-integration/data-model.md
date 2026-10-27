# Data Model: Google Snappy 原生引擎与流式帧架构 (083-snappy-native-engine-analysis-and-integration)

**Feature Branch**: `083-snappy-native-engine-analysis-and-integration`  
**Created**: 2026-08-18  
**Status**: Ready for Implementation  
**Feature Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/083-snappy-native-engine-analysis-and-integration/spec.md)

---

## 1. Snappy 核心实体定义 (Core Entities)

### 1.1 SnappyBlockCompressionRequest
原始块（Raw Block）压缩输入请求实体。

| 字段名 | 类型 | 必填 | 约束与说明 |
| :--- | :--- | :--- | :--- |
| `input_bytes` | `String` (Base64) | 是 | 待压缩原始字节流，长度 $\ge 0$ 且 $\le 2,147,483,647$ 字节 |
| `input_length` | `Integer` | 是 | 待压缩字节数，范围 $[0, 2^{31}-1]$ |
| `max_output_capacity` | `Integer` | 是 | 分配的输出缓冲区最大容量，必须 $\ge \text{max\_compressed\_len}(\text{input\_length})$ |

### 1.2 SnappyBlockCompressionResponse
原始块压缩响应结果实体。

| 字段名 | 类型 | 必填 | 约束与说明 |
| :--- | :--- | :--- | :--- |
| `status` | `String` (Enum) | 是 | 枚举值：`"ok"`, `"buffer_too_small"`, `"invalid_parameter"` |
| `compressed_length` | `Integer` | 是 | 实际输出的压缩字节数 |
| `compressed_bytes` | `String` (Base64) | 是 | 压缩后的二进制数据 |
| `ratio` | `Number` | 是 | 压缩比率（`compressed_length / input_length`），保留 4 位小数 |

---

### 1.3 SnappyBlockDecompressionRequest
原始块解压请求实体。

| 字段名 | 类型 | 必填 | 约束与说明 |
| :--- | :--- | :--- | :--- |
| `compressed_bytes` | `String` (Base64) | 是 | Snappy 压缩二进制数据 |
| `compressed_length` | `Integer` | 是 | 压缩流字节数，范围 $[1, 2^{31}-1]$ |
| `max_allowed_uncompressed_length` | `Integer` | 否 | 解压容量防 OOM 阈值，默认 $1,073,741,824$ (1 GB) |

### 1.4 SnappyBlockDecompressionResponse
原始块解压响应结果实体。

| 字段名 | 类型 | 必填 | 约束与说明 |
| :--- | :--- | :--- | :--- |
| `status` | `String` (Enum) | 是 | 枚举值：`"ok"`, `"corrupt_varint"`, `"corrupt_tag"`, `"offset_out_of_bounds"`, `"literal_overrun"`, `"buffer_too_small"` |
| `uncompressed_length` | `Integer` | 是 | 实际还原出的原始数据字节数 |
| `uncompressed_bytes` | `String` (Base64) | 是 | 还原后的二进制数据 |

---

### 1.5 SnappyFramingChunkHeader
Snappy Framing Format 单个分块头部模型。

| 字段名 | 类型 | 必填 | 约束与说明 |
| :--- | :--- | :--- | :--- |
| `chunk_type` | `Integer` | 是 | 1 字节无符号整数：`0` (Compressed), `1` (Uncompressed), `254` (Padding), `255` (Stream ID), `128..253` (Skippable), `2..127` (Reserved) |
| `chunk_length` | `Integer` | 是 | 24 位小端长度，范围 $[0, 16,777,215]$ 字节；对数据块限制 $\le 65,540$ |
| `masked_crc32c` | `Integer` | 否 | 32 位无符号 CRC32C 掩码校验和（仅 Chunk Type 为 `0` 或 `1` 时存在） |

### 1.6 SnappyFramingStreamConfig
Snappy 流式帧编解码配置实体。

| 字段名 | 类型 | 必填 | 约束与说明 |
| :--- | :--- | :--- | :--- |
| `max_chunk_size` | `Integer` | 是 | 单块未压缩数据上限，固定为 `65536` (64 KB) |
| `enable_arm64_hardware_crc32c` | `Boolean` | 是 | 是否启用 Apple Silicon ARM64 ACLE 硬件指令加速 |
| `verify_crc_on_decompression` | `Boolean` | 是 | 解压时是否强制逐块校验 Masked CRC32C，默认 `true` |
| `allow_cascade_streams` | `Boolean` | 是 | 是否允许处理连续多个 Stream Identifier 拼接的级联流，默认 `true` |

---

### 1.7 SnappyTarPipelineCommand
TAR.SZ 进程内归档与流式解包命令模型。

| 字段名 | 类型 | 必填 | 约束与说明 |
| :--- | :--- | :--- | :--- |
| `action` | `String` (Enum) | 是 | 枚举值：`"create_archive"`, `"extract_archive"` |
| `archive_path` | `String` | 是 | `.tar.sz` 或 `.sz` 归档物理路径 |
| `target_paths` | `Array[String]` | 是 | 输入文件/目录路径列表（压缩）或解包目标目录（解压） |
| `skip_mac_junk` | `Boolean` | 是 | 是否跳过 macOS 元数据垃圾文件（`__MACOSX`, `.DS_Store`） |
| `in_process_mode` | `Boolean` | 是 | 强制断言 100% 进程内内存管道（无外部子进程） |

---

## 2. 状态码与异常模型 (Error Taxonomy)

```
TTZIP_SNAPPY_OK                       (0)  -> 成功
TTZIP_SNAPPY_ERR_INVALID_MAGIC       (-1)  -> Stream Identifier 校验不匹配
TTZIP_SNAPPY_ERR_CORRUPT_VARINT      (-2)  -> Varint 超过 5 字节或未终止
TTZIP_SNAPPY_ERR_CORRUPT_TAG         (-3)  -> Tag 字节编码非法
TTZIP_SNAPPY_ERR_OFFSET_OUT_OF_BOUNDS(-4)  -> Copy Tag offset == 0 或超出历史窗口下界
TTZIP_SNAPPY_ERR_LITERAL_OVERRUN     (-5)  -> Literal 长度超出输入或输出缓冲区
TTZIP_SNAPPY_ERR_BUFFER_TOO_SMALL    (-6)  -> 目标解压缓冲区容量不足
TTZIP_SNAPPY_ERR_CRC32C_MISMATCH     (-7)  -> Masked CRC32C 校验和不匹配
TTZIP_SNAPPY_ERR_UNSUPPORTED_CHUNK   (-8)  -> 遇到保留不可跳过 Chunk Type (0x02~0x7f)
TTZIP_SNAPPY_ERR_UNEXPECTED_EOF      (-9)  -> 流式输入意外截断
```
