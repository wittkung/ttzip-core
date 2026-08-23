# Data Model Specification: 084-lzham-branchless-decompression-and-circular-dict

**Feature Directory**: `specs/084-lzham-branchless-decompression-and-circular-dict`  
**Created**: 2026-08-18  
**Status**: Completed  
**Spec Reference**: [`spec.md`](spec.md) | **Plan Reference**: [`plan.md`](plan.md)

---

## 1. Core Data Structures (C11 Memory Layout)

### 1.1 Bitstream State Structure (`ttzip_bitstream_reader_t`)

64 位宽输入比特流预取状态机，紧凑常驻寄存器：

```c
typedef struct {
    uint64_t bit_buf;             // 64-bit 预取移位寄存器
    int32_t bit_count;            // 当前 bit_buf 中有效的比特数 (0 ~ 64)
    const uint8_t *in_ptr;        // 当前输入字节流游标指针
    const uint8_t *in_limit;      // 输入缓冲区末尾边界指针
    uint32_t is_eof;              // 输入流结束标志 (0 = 未结束, 1 = 已至末尾)
} ttzip_bitstream_reader_t;
```

**Field Invariants**:
- `bit_buf` 总是高位对齐当前待消费数据。
- `0 <= bit_count <= 64`。
- `in_ptr <= in_limit`。

---

### 1.2 Branchless 11-Bit Huffman Decoder Table (`ttzip_huffman_lut_t`)

用于单周期哈夫曼符号直出的 11 位查表加速结构：

```c
#define TTZIP_HUFFMAN_LUT_BITS 11
#define TTZIP_HUFFMAN_LUT_SIZE (1U << TTZIP_HUFFMAN_LUT_BITS) // 2048

typedef struct {
    uint32_t table_bits;          // 查表位数 (固定为 11)
    uint32_t max_code_len;        // 该表支持的最大码长 (例如 16)
    uint32_t table_max_code;      // 落在 LUT 内的最大有效码值
    uint32_t num_symbols;         // 表中包含的唯一符号总数
    uint32_t lookup[TTZIP_HUFFMAN_LUT_SIZE]; // 查表数据: [31:16]=len, [15:0]=symbol
} ttzip_huffman_lut_t;
```

**Field Invariants**:
- `lookup[idx]` 高 16 位为符号比特长度 `len` ($1 \le \text{len} \le 16$)。
- `lookup[idx]` 低 16 位为解码符号值 `symbol` ($0 \le \text{symbol} < \text{num\_symbols}$)。

---

### 1.3 Power-of-Two Circular Ring Dictionary (`ttzip_ring_dict_t`)

$2^N$ 掩码环形滑动窗口解压字典：

```c
typedef struct {
    uint8_t *dict_buf;            // 字典连续内存基址 (必须 64 字节对齐)
    size_t dict_size;             // 字典大小 (必须是 2 的整数次幂, 32KB ~ 512MB)
    size_t dict_size_mask;        // 字典掩码 = dict_size - 1
    size_t write_pos;             // 当前字典写入游标 (0 <= write_pos < dict_size)
    size_t total_written;         // 累计解压写入总字节数
} ttzip_ring_dict_t;
```

**Field Invariants**:
- `(dict_size & (dict_size - 1)) == 0`（严格 $2^N$ 约束）。
- `dict_size_mask == dict_size - 1`。
- `write_pos < dict_size`。

---

### 1.4 Match Copy Operation Parameters (`ttzip_match_copy_req_t`)

解压状态机单次 Match Copy 请求载荷：

```c
typedef struct {
    size_t match_dist;            // 匹配回溯距离 (1 <= match_dist <= dict_size)
    size_t match_len;             // 匹配长度 (1 <= match_len <= 65536)
    uint32_t is_rle;              // 是否为 match_dist == 1 的 RLE 填充 (0/1)
} ttzip_match_copy_req_t;
```

---

## 2. Bidirectional Mapping Table

| C 结构体字段 | 对应的 JSON Schema 字段 | 数据类型 | 约束条件 |
| :--- | :--- | :--- | :--- |
| `dict_size` | `dict_size` | `integer` | 最小值 $32768$，且为 $2^N$ |
| `dict_size_mask` | `dict_size_mask` | `integer` | 等于 `dict_size - 1` |
| `write_pos` | `write_pos` | `integer` | $0 \le \text{write\_pos} < \text{dict\_size}$ |
| `match_dist` | `match_dist` | `integer` | $1 \le \text{match\_dist} \le \text{dict\_size}$ |
| `match_len` | `match_len` | `integer` | $1 \le \text{match\_len} \le 65536$ |
| `table_bits` | `table_bits` | `integer` | 枚举值 `11` |
