# Data Model: Streaming Fast-Path Decompressor & Dual-Symbol LUT

**Feature**: `124-streaming-fastpath-decompressor-dual-symbol-lut`
**Created**: 2026-08-19

---

## Entities & Type Definitions

### 1. `DecompressorRequest`
Represents an invocation to decompress a Deflate bitstream.

| Field | Type | Required | Description | Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `input_size_bytes` | `uint64` | Yes | Byte size of compressed input stream | Non-negative integer |
| `output_capacity_bytes` | `uint64` | Yes | Byte capacity of destination buffer | Non-negative integer |

### 2. `DecompressorResponse`
Represents decompression execution results.

| Field | Type | Required | Description | Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `decompressed_bytes` | `uint64` | Yes | Exact number of uncompressed bytes produced | Non-negative integer |
| `status_code` | `int32` | Yes | 0 on success, non-zero on error | Integer |

---

## JSON Schema Mapping
The models defined above map 1:1 with:
- `contracts/decompressor_request.schema.json`
- `contracts/decompressor_response.schema.json`
