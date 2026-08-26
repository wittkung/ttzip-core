# Phase 1 Data Model: LZMA2 Match Finder & SWAR Core

## 1. Entities & Structures

### `ttzip_match_t`
```c
typedef struct {
    uint32_t len;    // 匹配长度 (>= 2, <= len_limit)
    uint32_t dist;   // 匹配距离偏移 (0-based: actual_distance - 1)
} ttzip_match_t;
```

### `ttzip_hc4_t`
```c
typedef struct {
    uint32_t* hash2;         // 2-byte 直接哈希表 (65536 项 = 256KB)
    uint32_t* hash3;         // 3-byte 直接哈希表 (65536 项 = 256KB)
    uint32_t* hash4;         // 4-byte 哈希链表 (hash_mask + 1 项，L1 为 64K 项 = 256KB)
    uint32_t* chain;         // 链表节点数组 (dict_size 项)
    const uint8_t* buffer;   // 输入数据指针
    uint32_t  buffer_size;   // 输入总大小
    uint32_t  pos;           // 当前读取游标
    uint32_t  dict_size;     // 字典大小 (L1 为 64KB ~ 256KB)
    uint32_t  hash_mask;     // hash4 掩码
    uint32_t  cut_value;     // 最大链表遍历深度 (L1 设为 1 ~ 4)
    uint32_t  nice_len;      // 满意匹配长度 (L1 设为 8 ~ 32)
    uint32_t  len_limit;     // 最大允许匹配长度 (273)
} ttzip_hc4_t;
```

## 2. Invariants & Bounds Rules
1. `ttzip_match_len_neon(p1, p2, max_len)` 返回值严格满足：`0 <= return_value <= max_len`。
2. 当 `max_len < 8` 时，跳过 64-bit 块循环，直接由逐字节标量比对兜底。
3. 内存读取绝不超过 `p1 + max_len` 与 `p2 + max_len`。
