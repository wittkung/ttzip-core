# Data Model: Google Zopfli 官方上游集成与多核并发实体规范

**Feature ID**: `105-zopfli-upstream-integration-and-research`  
**Created**: 2026-08-19  
**Status**: DRAFT (Phase 1 Design)  

---

## 1. Entities & Data Structures

### 1.1 `TTZipZopfliProfileConfig` (Zopfli 运行时配置模型)
定义单次压缩任务或并发线程的 Zopfli 执行参数：

| 字段名 | 类型 | 必填 | 取值范围 / 约束 | 说明 |
| :--- | :--- | :--- | :--- | :--- |
| `compressionLevel` | `Int32` | 是 | `1 ... 15` | 请求的压缩档位级别 (Level 6 或 Level 7) |
| `numIterations` | `Int32` | 是 | `1 ... 100` | Squeeze DP 迭代轮次 (Level 6: 5, Level 7: 15) |
| `blockSplitting` | `Bool` | 是 | `true / false` | 是否启用动态局部熵变最优块切分 |
| `maxBlockSplits` | `Int32` | 是 | `0 ... 64` | 单块允许的最大子切分块数 (Level 7: 15) |
| `earlyExitThreshold` | `Double` | 是 | `0.0 ... 0.01` | 自适应迭代收敛早退阈值 (默认 0.00005) |

### 1.2 `TTZipZopfliTileDescriptor` (多核并发分块任务描述符)
描述 18 核心并发分块拓扑中单个 Tile 的调度元数据：

| 字段名 | 类型 | 必填 | 约束 | 说明 |
| :--- | :--- | :--- | :--- | :--- |
| `tileIndex` | `Int32` | 是 | `0 ... 63` | 当前分块在文件中的序列索引 |
| `totalTiles` | `Int32` | 是 | `1 ... 64` | 总分块数 (通常为 CPU 物理核心数 18) |
| `inOffset` | `Int64` | 是 | `0 ... rawSize` | 当前 Tile 在原始文件中的起始字节偏移量 |
| `inLength` | `Int64` | 是 | `1 ... 33554432` | 当前 Tile 的有效原始数据字节数 |
| `historyOffset` | `Int64` | 是 | `0 ... inOffset` | 跨 Tile 32KB 历史字典的起始偏移量 |
| `historyLength` | `Int32` | 是 | `0 ... 32768` | 历史字典有效字节数 (首块为 0, 后续块 $\le 32768$) |
| `isFinalTile` | `Bool` | 是 | `true / false` | 是否为整个归档流的最后一个分块 |

### 1.3 `TTZipZopfliTileResult` (单个分块压缩执行结果)
工作线程执行 `ZopfliDeflatePart` 后的输出度量：

| 字段名 | 类型 | 必填 | 约束 | 说明 |
| :--- | :--- | :--- | :--- | :--- |
| `tileIndex` | `Int32` | 是 | `0 ... 63` | 对应的分块索引 |
| `compressedBytes` | `Int64` | 是 | `1 ... inLength + 512` | 该分块压缩生成的 Deflate 字节数 (含 SYNC_FLUSH) |
| `uncompressedBytes` | `Int64` | 是 | `inLength` | 原始未压缩字节数 |
| `executionDurationMs`| `Double`| 是 | `>= 0.0` | 该分块在线程上的物理计算耗时 (ms) |
| `status` | `String`| 是 | `enum: SUCCESS, BUFFER_OVERFLOW, INVALID_INPUT` | 执行状态 |

---

## 2. Invariants & Boundaries

1. **RFC 1951 流式合法性不变式 (RFC 1951 Stream Invariant)**：
   - 设总块数为 $N$。对于任意分块 $k \in [0, N-2]$，其输出必须且仅能以 `BFINAL=0` 结束并紧跟 `0x00, 0x00, 0xFF, 0xFF` 字节对齐标记；
   - 分块 $k = N-1$ 其末尾块必须输出 `BFINAL=1`，且不得追加多余的空块。
2. **字典引用边界不变式 (History Window Invariant)**：
   - 跨块历史字典长度严格约束在 `historyLength <= 32768`；
   - 任何 match distance $d$ 必须满足 $1 \le d \le 32768$。
3. **零通配类型安全 (Zero Bare Objects)**：
   - 所有数据结构字段必须具备确定的强类型，严禁使用 `Any` 或未约束的 `Object`。
