# Data Model: 100% 自研零外部依赖原生 Apple Silicon DEFLATE 引擎体系

**Feature ID**: `107-zero-dependency-native-deflate-engine`  
**Status**: APPROVED  

---

## 1. C 层数据模型实体 (C-Level Entities)

### 1.1 `ttzip_bitstream_t` (64-bit 寄存器位流累加器)
```c
typedef struct {
    uint64_t bit_buffer;   // 64-bit 累加寄存器 (0 <= bit_count <= 63)
    unsigned bit_count;    // 当前有效比特数
    uint8_t *out_next;     // 输出写入指针
    uint8_t *out_end;      // 缓冲区物理上限
    uint8_t *out_fast_end; // 64-bit 快速无分支写入哨兵 (out_end - 8)
} ttzip_bitstream_t;
```

### 1.2 `ttzip_deflate_fast_mf_t` (Tier 1/2 极速匹配查找器状态)
```c
#define TTZIP_HASH4_ORDER 15 // 32768 项
typedef int16_t ttzip_mf_pos_t;

typedef struct __attribute__((aligned(64))) {
    ttzip_mf_pos_t hash_tab[1 << TTZIP_HASH4_ORDER][2]; // 128KB, 100% L1D 驻留
} ttzip_deflate_fast_mf_t;
```

### 1.3 `ttzip_deflate_lazy_mf_t` (Tier 3/4 Lazy 延迟匹配查找器状态)
```c
typedef struct __attribute__((aligned(64))) {
    ttzip_mf_pos_t hash3_tab[32768];  // 64KB 短匹配查找表
    ttzip_mf_pos_t hash4_tab[32768];  // 64KB 主匹配查找表
    ttzip_mf_pos_t next_tab[32768];   // 64KB 链式回溯表
} ttzip_deflate_lazy_mf_t;
```

### 1.4 `ttzip_huffman_codes_t` (Canonical Huffman 码字与长度表)
```c
typedef struct {
    uint32_t codewords_litlen[288];  // 融合码字 (LSB-first 码字 | (extra_bits << len_bits))
    uint8_t  lens_litlen[288];       // 码长 (0..15 bits)
    uint32_t codewords_offset[32];   // 距离码字
    uint8_t  lens_offset[32];        // 距离码长 (0..15 bits)
} ttzip_huffman_codes_t;
```

### 1.5 `ttzip_deflate_options_t` (原生 Deflate 压缩配置模型)
```c
typedef struct {
    int32_t  tier_level;              // 1..7 档位
    uint32_t max_chain_depth;         // 链式回溯深度 (0 for Fast, 4..32 for Lazy)
    uint32_t nice_match_len;          // 提前终止长度 (32..258)
    bool     dynamic_huffman;         // 是否启用动态 Huffman 编码
    bool     enable_history_warmup;   // 是否开启 32KB 跨 Tile 字典预热
} ttzip_deflate_options_t;
```

---

## 2. Swift 层数据模型实体 (Swift-Level Entities)

### 2.1 `ZipNativeDeflateProfile`
```swift
public struct ZipNativeDeflateProfile: Sendable, Equatable {
    public let tierLevel: Int
    public let name: String
    public let maxChainDepth: Int
    public let niceMatchLength: Int
    public let useDynamicHuffman: Bool
    public let targetThroughputFloorMBs: Double
}
```

### 2.2 `NativeDeflateTileResult`
```swift
public struct NativeDeflateTileResult: Sendable {
    public let tileIndex: Int
    public let compressedData: Data
    public let uncompressedBytes: Int64
    public let compressedBytes: Int64
    public let crc32: UInt32
    public let isFinal: Bool
}
```
