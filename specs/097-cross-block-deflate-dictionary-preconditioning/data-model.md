# Data Model: 097-cross-block-deflate-dictionary-preconditioning

## Models & Memory Invariants

### 1. `CrossBlockDeflateChunkDescriptor`
```c
typedef struct {
    size_t chunk_index;           /**< 0-based chunk index */
    const uint8_t* chunk_ptr;     /**< Pointer to current block payload */
    size_t chunk_size;            /**< Uncompressed size of current block */
    const uint8_t* dict_ptr;      /**< Pointer to trailing <= 32KB of previous block (NULL for block 0) */
    size_t dict_size;             /**< Byte count of dictionary window (<= 32768) */
    bool is_final;                /**< True for terminal block */
} ttzip_cross_block_chunk_desc_t;
```
