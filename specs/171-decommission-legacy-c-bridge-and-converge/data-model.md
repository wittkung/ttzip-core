# Data Model: CTTZipBridge 兼容 C-ABI 符号映射模型 (Feature 171)

**Feature ID**: `171-decommission-legacy-c-bridge-and-converge`  
**Created**: 2026-08-21  
**Status**: Completed  
**Artifact**: Phase 1 Data Model & Types

---

## 1. 兼容 C-ABI 导出符号列表

### 1.1 内存与对齐分配符号
```c
void* ttzip_core_aligned_alloc_16k(size_t size);
void ttzip_core_aligned_free_16k(void* ptr);
```

### 1.2 字符串与魔法字节探测符号
```c
int ttzip_strnatcmp(const char *a, const char *b);
int ttzip_strnatcasecmp(const char *a, const char *b);
typedef struct {
    uint32_t kind;
    const char *format;
    const char *mime;
} ttzip_magic_info_t;
ttzip_magic_info_t ttzip_magic_sniff_buffer(const uint8_t *buf, size_t len);
```

### 1.3 霍夫曼与分块评估符号
```c
void ttzip_make_canonical_huffman_code_inplace(const uint8_t *lens, uint16_t *codes, size_t count);
uint16_t ttzip_canonical_bit_reverse(uint16_t code, uint8_t len);
int32_t ttzip_eval_best_block_type(uint64_t dynamic_cost, uint64_t static_cost, uint64_t block_len, uint64_t bit_count, uint64_t *best_cost);
```

### 1.4 快压快解与辅助 C 别名
```c
size_t ttzip_gzip_compress_bound(size_t src_len);
size_t ttzip_zlib_compress_bound(size_t src_len);
size_t ttzip_gzip_compress_fast(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_cap, int32_t level);
size_t ttzip_gzip_decompress_fast(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_cap);
size_t ttzip_zlib_compress_fast(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_cap, int32_t level);
size_t ttzip_zlib_decompress_fast(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_cap);
```
