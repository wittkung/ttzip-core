# Phase 1 Data Model: Blosc2 Exhaustive Architectural Conquest

**Feature**: `specs/091-blosc2-exhaustive-architectural-conquest`  
**Date**: 2026-08-18  

---

## 1. Entities & Structural Definitions

```mermaid
classDiagram
    class PluginRegistry {
        +uint8_t user_filter_slots[96]
        +uint8_t user_codec_slots[96]
        +register_filter(plugin) int
        +register_codec(plugin) int
        +dispatch_forward(id, src, dst, len) int
        +dispatch_backward(id, src, dst, len) int
    }

    class LazySliceRequest {
        +int64_t start_byte
        +int64_t length
        +int32_t first_chunk_idx
        +int32_t last_chunk_idx
        +int32_t first_block_idx
        +int32_t last_block_idx
    }

    class BitGroomingConfig {
        +uint8_t nsd
        +uint8_t mode
        +uint8_t type_size
        +uint32_t mantissa_mask
    }

    class Blosc2FrameV2 {
        +char magic[8]
        +int64_t frame_len
        +int64_t nbytes
        +int64_t cbytes
        +int32_t chunksize
        +int32_t blocksize
        +int64_t nchunks
        +int64_t coffsets[]
        +VLMetaEntry metalayers[]
        +uint32_t trailer_crc
    }

    PluginRegistry --> Blosc2FrameV2 : Injected Pipeline
    Blosc2FrameV2 --> LazySliceRequest : Slices By Micro-Blocks
    Blosc2FrameV2 --> BitGroomingConfig : Pre-Filter Masking
```

---

## 2. Invariants & Data Constraints

1. **Plugin ID Range Invariant**:
   - Built-in filter/codec IDs: $0 \le ID \le 159$.
   - User-defined plugin IDs: $160 \le ID \le 255$.
   - Attempting to register outside $[160, 255]$ returns an error code `-1`.
2. **Lazy Slicing Range Invariant**:
   - Slicing bounds: $0 \le \text{start\_byte} < \text{uncompressed\_size}$.
   - $\text{start\_byte} + \text{length} \le \text{uncompressed\_size}$.
   - For all skipped blocks $b < \text{first\_block}$ and $b > \text{last\_block}$, zero decompression calls and zero byte buffer allocations are performed.
3. **Bit-Grooming Precision Invariant**:
   - For `float32`, $1 \le \text{NSD} \le 7$. Mantissa bits kept: $\text{prc} = \lceil 3.321928 \times \text{NSD} \rceil + 1$.
   - For `float64`, $1 \le \text{NSD} \le 15$. Mantissa bits kept: $\text{prc} = \lceil 3.321928 \times \text{NSD} \rceil + 2$.
   - Guaranteed bounded relative precision: $\frac{|x - x_{\text{quant}}|}{|x|} \le 0.5 \times 10^{1 - \text{NSD}}$.
4. **Frame Format Invariant**:
   - Frame header starts with 8-byte magic `0x62326672616D6500` (`"b2frame\0"`).
   - Trailer starts with 8-byte magic `"TTZIPVLM"` and ends with 4-byte CRC-32 integrity tag.
