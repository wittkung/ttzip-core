# Phase 1 Data Model: Codebase Algorithmic Optimization and Algebraic Kernels

**Feature**: `089-codebase-algorithmic-optimization`
**Created**: 2026-08-18
**Status**: Completed

---

## 1. Entities & Data Structures

### 1.1 `TTZipTarHeaderEntryInfo` (Stack-Allocated 48-Byte C Record)
Represents the in-place decoded metadata from a standard 512-byte POSIX ustar / GNU tar header block without heap allocations.

| Field | C Type | Swift Equivalent | Nullable | Description / Invariant |
| :--- | :--- | :--- | :--- | :--- |
| `size` | `uint64_t` | `UInt64` | No | File payload size in bytes (parsed via SWAR octal or GNU base-256 binary format) |
| `mode` | `uint32_t` | `UInt32` | No | File permission mode flags (octal parsed) |
| `mtime` | `int64_t` | `Int64` | No | Modification timestamp in seconds since POSIX epoch |
| `typeflag` | `uint8_t` | `UInt8` | No | Single-byte TAR entry type (`'0'`=regular, `'5'`=directory, `'2'`=symlink, etc.) |
| `is_ustar` | `uint8_t` | `UInt8` | No | Boolean flag: 1 if `"ustar\0"` or `"ustar "` magic present at offset 257, else 0 |
| `is_eoa_zero` | `uint8_t` | `UInt8` | No | Boolean flag: 1 if entire 512-byte block is all zero (End-of-Archive candidate) |
| `checksum_valid` | `uint8_t` | `UInt8` | No | Boolean flag: 1 if computed unsigned or signed checksum matches header value |
| `stored_checksum` | `uint32_t` | `UInt32` | No | The raw 6-digit octal checksum parsed from header offset 148 |
| `computed_unsigned` | `uint32_t` | `UInt32` | No | Branchless computed POSIX standard unsigned checksum over 512 bytes |
| `computed_signed` | `int32_t` | `Int32` | No | Branchless computed legacy signed checksum over 512 bytes |
| `name_offset` | `uint16_t` | `UInt16` | No | Offset in raw buffer where entry filename starts (0 for standard header) |
| `name_len` | `uint16_t` | `UInt16` | No | Length of filename string up to NUL terminator (max 100 or 256 with prefix) |

### 1.2 `TTZipVarintDecodeResult` (Branchless 7Z Varint Payload)
Represents the result of a single variable-length integer decoding step from a 7Z container metadata bitstream.

| Field | C Type | Swift Equivalent | Nullable | Description / Invariant |
| :--- | :--- | :--- | :--- | :--- |
| `value` | `uint64_t` | `UInt64` | No | Fully reconstructed 64-bit unsigned integer value |
| `bytes_consumed` | `size_t` | `Int` | No | Number of bytes consumed from input buffer ($1 \dots 9$, or 0 on error/short buffer) |
| `is_valid` | `uint8_t` | `UInt8` | No | Boolean flag: 1 if decoded within buffer bounds without UB, else 0 |

### 1.3 `TTZipAdler32ChunkState` (Mathematical Accumulator State)
Represents the register-mapped running state of an Adler-32 deferred modulo execution block.

| Field | C Type | Swift Equivalent | Nullable | Description / Invariant |
| :--- | :--- | :--- | :--- | :--- |
| `s1` | `uint32_t` | `UInt32` | No | Low 16-bit accumulator running sum ($s_1 \in [0, 65520]$ after reduction) |
| `s2` | `uint32_t` | `UInt32` | No | High 16-bit accumulator running sum ($s_2 \in [0, 65520]$ after reduction) |
| `chunk_bytes_processed` | `size_t` | `Int` | No | Total bytes processed in current deferred modulo chunk ($\le 5552$ bytes) |

### 1.4 `TTZipKernelVerificationReport` (Differential Validation Report)
Structured diagnostic object emitted during algorithmic test passes and CI verification gates.

| Field | Type | Required | Description / Invariant |
| :--- | :--- | :--- | :--- |
| `kernel_name` | `string` | Yes | Name of optimized kernel (`adler32_scalar`, `tar_parse_octal_swar`, `varint7z_clz`, `crc64_pmull_tail`) |
| `vector_count` | `integer` | Yes | Total number of test vectors evaluated ($> 0$) |
| `mismatches` | `integer` | Yes | Number of bit discrepancies detected (MUST strictly equal 0) |
| `throughput_mbps` | `number` | Yes | Physical benchmark throughput in MB/s on target hardware |
| `speedup_factor` | `number` | Yes | Ratio of optimized throughput vs. legacy reference implementation |
| `status` | `string` | Yes | Enum: `"PASSED"` or `"FAILED"` |

---

## 2. Invariants and State Transitions

1. **Adler-32 Chunk Boundary Invariant**:
   At any point in time during scalar chunk execution, the number of bytes accumulated between modulo reductions $N$ MUST satisfy $N \le 5552$. Overflow in $s_2$ ($s_2 \ge 2^{32}$) is mathematically impossible under this invariant.
2. **7Z Varint Shift Clamping Invariant**:
   For any varint with $k \in [0, 8]$ trailing bytes, the shift count operand for high-bit reconstruction MUST be calculated as `(k & 7) * 8`, guaranteeing shift amounts strictly in $\{0, 8, 16, 24, 32, 40, 48, 56\}$. Shifting by 64 is strictly eliminated.
3. **TAR Checksum Equivalence Invariant**:
   The header validation is satisfied if and only if `stored_checksum == computed_unsigned` OR `(int32_t)stored_checksum == computed_signed`.
