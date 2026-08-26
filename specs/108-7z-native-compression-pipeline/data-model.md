# Data Model: 7z 全链路原生压缩流算法与自研无依赖引擎

**Feature ID**: `108-7z-native-compression-pipeline`  
**Date**: 2026-08-19  
**Status**: Ready  

---

## 1. 核心实体模型 (Core Domain Entities)

### 1.1 `SevenZipEncoderPipelineConfig` (7z 编码管道配置)
定义自研 7z 编码器在执行压缩归档时的数据配置实体。

| 字段名 | 类型 | 必填 | 默认值 | 说明与取值范围 |
| :--- | :--- | :--- | :--- | :--- |
| `outputPath` | `String` | 是 | - | 目标输出 `.7z` 归档绝对路径 |
| `inputPaths` | `[String]` | 是 | - | 待打包压缩的源文件或目录路径列表 |
| `compressionLevel` | `Int` | 是 | `5` | 压缩等级：`0` (Store), `1` (Fast), `5` (Normal), `7` (Maximum), `9` (Ultra) |
| `dictSize` | `Int` | 是 | `16777216` | 字典大小（字节）：`65536` (64KB) 到 `67108864` (64MB) |
| `solidBlockSizeMb` | `Int` | 是 | `128` | Solid 固实压缩分块大小（MB）：`16` 到 `1024` |
| `enableNeonAcceleration` | `Bool` | 是 | `true` | 是否启用 ARM64 NEON 与 ACLE CRC32 硬件加速 |
| `password` | `String?` | 否 | `nil` | 可选的 AES-256 加密密码（UTF-8 字符串） |

---

### 1.2 `SevenZipDecoderPipelineConfig` (7z 解码管道配置)
定义自研 7z 解码器在执行归档解压时的数据配置实体。

| 字段名 | 类型 | 必填 | 默认值 | 说明与取值范围 |
| :--- | :--- | :--- | :--- | :--- |
| `archivePath` | `String` | 是 | - | 输入待解压 `.7z` 归档文件路径 |
| `destinationDir` | `String` | 是 | - | 解压目标输出目录路径 |
| `skipMacJunk` | `Bool` | 是 | `true` | 是否过滤 macOS 特有元数据（`.DS_Store`、`__MACOSX`） |
| `password` | `String?` | 否 | `nil` | 可选的 AES-256 解密密码 |
| `verifyChecksum` | `Bool` | 是 | `true` | 是否强制校验每个条目的 CRC-32 校验和 |

---

### 1.3 `SevenZipExecutionResult` (7z 执行结果与统计)
定义 7z 操作（编码或解码）完成后的统计结果数据模型。

| 字段名 | 类型 | 必填 | 说明 |
| :--- | :--- | :--- | :--- |
| `statusCode` | `Int` | 是 | 状态码：`0` 表示成功，非 0 表示具体错误码（如 `-1` 参数错误, `-102` 无文件等） |
| `operationType` | `String` | 是 | 操作类型：枚举 `"encode"` 或 `"decode"` |
| `totalInputBytes` | `Int` | 是 | 输入原始数据总字节数 |
| `totalOutputBytes` | `Int` | 是 | 输出结果数据总字节数 |
| `compressionRatio` | `Double` | 是 | 压缩率：`totalOutputBytes / totalInputBytes`（解码时为反比） |
| `durationMs` | `Double` | 是 | 执行总耗时（毫秒） |
| `throughputMBps` | `Double` | 是 | 物理吞吐速率（MB/s） |
| `filesProcessed` | `Int` | 是 | 处理的文件/条目总数 |

---

### 1.4 `SevenZipAuditAssetRecord` (7z 架构资产与依赖审计项)
定义架构审计中每一个 7z 模块的资产属性模型。

| 字段名 | 类型 | 必填 | 说明 |
| :--- | :--- | :--- | :--- |
| `componentId` | `String` | 是 | 模块唯一标识（如 `"7z_header_parser"`, `"lzma2_encoder_l1"`） |
| `sourceFilePath` | `String` | 是 | 物理源文件绝对或相对路径 |
| `lineRange` | `String` | 是 | 核心逻辑覆盖行号区间（如 `"L49-L170"`） |
| `currentDependency` | `String` | 是 | 当前依赖实现（如 `"In-House Native C"`, `"liblzma.a"`, `"fast-lzma2"`） |
| `targetDependency` | `String` | 是 | 演进目标依赖（统一为 `"100% In-House Native C"`） |
| `zipReusableFeature` | `String` | 是 | 对应的 ZIP 底层复用模块（如 `"ttzip_hybrid_match_len_neon"`） |
| `targetThroughputMBps` | `Double` | 是 | 目标硬吞吐下限（MB/s） |

---

## 2. C 层状态机结构体定义 (C-Level State Data Structures)

### 2.1 `ttzip_lzma2_dec_state_t` (自研原生 LZMA2 解码器状态)
```c
typedef struct {
    uint32_t range;                         // 当前 Range 区间宽度 (0x00000000 ~ 0xFFFFFFFF)
    uint32_t code;                          // 当前 Range 编码值
    const uint8_t* in_ptr;                  // 输入比特流当前指针
    const uint8_t* in_limit;                // 输入比特流末尾边界
    uint32_t rep[4];                        // 历史 4 个重复距离 (rep0..rep3)
    uint32_t state;                         // 当前 LZMA 状态 (0..11)
    uint16_t probs[16384];                  // 概率表模型 (2048 定点概率)
    uint8_t  lc;                            // Literal context bits (0..8, 默认 3)
    uint8_t  lp;                            // Literal pos bits (0..4, 默认 0)
    uint8_t  pb;                            // Pos state bits (0..4, 默认 2)
    uint8_t  corrupt;                       // 损坏/异常标志位
} ttzip_lzma2_dec_state_t;
```

### 2.2 `ttzip_lzma2_fast_enc_state_t` (自研原生 Double-Fast 极速编码器状态)
```c
typedef struct {
    uint32_t table_small[65536];            // 4-Byte 哈希直连查找表 (256KB)
    uint32_t table_long[65536];             // 8-Byte 哈希直连查找表 (256KB)
    uint16_t probs[16384];                  // 编码器概率表 (28KB)
    uint32_t rep[4];                        // 4 个最近匹配距离
    uint32_t state;                         // LZMA 状态转移机
    uint32_t dict_size;                     // 当前字典大小 (64KB ~ 1MB)
} ttzip_lzma2_fast_enc_state_t;
```

### 2.3 `ttzip_lzma2_opt_parser_state_t` (自研前向 DP 最优解析器状态)
```c
typedef struct {
    uint32_t price;                         // 累计最小比特代价 (定点化 bits * 64)
    uint32_t pos_prev;                      // 前驱节点索引
    uint32_t back_prev;                     // 前驱转移距离
    uint32_t state;                         // 转移后 LZMA 状态
    uint32_t reps[4];                       // 转移后 4 个重复距离
} ttzip_lzma2_opt_node_t;

typedef struct {
    ttzip_lzma2_opt_node_t opt_nodes[4096]; // 定长 4096 最优前向决策窗口
    uint32_t prob_prices[512];              // 概率代价速查表
    uint32_t opt_cur;                       // 当前已决策位置
    uint32_t opt_end;                       // 搜索探测终点
} ttzip_lzma2_opt_parser_state_t;
```
