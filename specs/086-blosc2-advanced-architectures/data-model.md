# Phase 1: Data Model & Binary Memory Layouts

## 1. Float Truncation Filter State & Types

```c
typedef enum {
    TTZIP_FILTER_TRUNCATE_FLOAT32 = 4,
    TTZIP_FILTER_TRUNCATE_FLOAT64 = 5
} ttzip_filter_truncate_type_t;

typedef struct {
    uint8_t keep_mantissa_bits; // 1..23 for float32, 1..52 for float64
    bool enable_rounding_bias;  // true for half-bit unbiased rounding
} ttzip_truncate_config_t;
```

---

## 2. Prefetch Ring Buffer State Machine

```c
typedef enum {
    TTZIP_PREFETCH_SLOT_EMPTY = 0,
    TTZIP_PREFETCH_SLOT_LOADING = 1,
    TTZIP_PREFETCH_SLOT_READY = 2,
    TTZIP_PREFETCH_SLOT_CONSUMING = 3
} ttzip_prefetch_slot_state_t;

typedef struct {
    uint8_t* buffer;           // 128-byte aligned memory page buffer
    size_t capacity;           // Typically 4MB ~ 16MB
    size_t valid_bytes;        // Actual uncompressed/compressed payload size
    int64_t block_index;       // Logical chunk/block index
    _Atomic ttzip_prefetch_slot_state_t state;
} ttzip_prefetch_slot_t;

typedef struct {
    ttzip_prefetch_slot_t slots[2]; // Double buffering
    pthread_mutex_t lock;
    pthread_cond_t cond_ready;
    pthread_cond_t cond_empty;
    bool is_stopped;
} ttzip_prefetch_pipeline_t;
```

---

## 3. VLMeta Binary Trailer Layout

```
Offset 0x00:  [8 bytes: Magic "TTZIPVLM"]
Offset 0x08:  [4 bytes: Version uint32_t (1)]
Offset 0x0C:  [4 bytes: Layer Count uint32_t]
Offset 0x10:  [8 bytes: Uncompressed Size uint64_t]
Offset 0x18:  [8 bytes: Compressed Payload Size uint64_t]
Offset 0x20:  [Payload: Zstd Compressed MessagePack Key-Value Table]
...
Offset EOF-16: [8 bytes: Trailer Start File Offset uint64_t]
Offset EOF-8:  [8 bytes: Footer Magic "TTZIPVLM"]
```

---

## 4. N-Dimensional Tensor Geometry

```c
typedef struct {
    int8_t ndim;                     // Number of dimensions (1..8)
    int64_t shape[8];                // Full array shape S_i
    int32_t chunkshape[8];           // Chunk shape C_i
    int32_t blockshape[8];           // Block shape B_i
    int64_t chunk_strides[8];        // Linear stride per chunk
    int32_t block_strides[8];        // Linear stride per block inside chunk
    int32_t elem_strides[8];         // Linear stride per element inside block
    uint8_t item_size;               // Size per element (1, 2, 4, 8, 16)
} ttzip_tensor_geometry_t;
```
