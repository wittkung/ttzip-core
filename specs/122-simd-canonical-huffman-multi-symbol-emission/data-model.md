# Data Model: SIMD Canonical Huffman Coding & Multi-Symbol Emission

**Feature**: `122-simd-canonical-huffman-multi-symbol-emission`
**Created**: 2026-08-19

---

## Entities & Type Definitions

### 1. `HuffmanBitstreamEncoderRequest`
Represents an invocation of the multi-symbol Huffman bitstream serializer.

| Field | Type | Required | Description | Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `token_count` | `uint64` | Yes | Number of tokens in input array | Non-negative integer |
| `use_dynamic_huffman` | `boolean` | Yes | Whether dynamic or static Huffman tables are used | Boolean |
| `chunk_size_bytes` | `uint64` | Yes | Original uncompressed chunk size | Non-negative integer |

### 2. `HuffmanBitstreamEncoderResponse`
Represents the result of bitstream encoding.

| Field | Type | Required | Description | Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `compressed_bytes_written` | `uint64` | Yes | Total bytes written to destination bitstream | Non-negative integer |
| `packed_token_writes` | `uint64` | Yes | Number of 64-bit multi-symbol packed writes performed | Non-negative integer |
| `elapsed_nanoseconds` | `uint64` | Yes | Encoding execution duration in nanoseconds | Non-negative integer |

---

## JSON Schema Mapping
The models defined above map 1:1 with:
- `contracts/huffman_encoder_request.schema.json`
- `contracts/huffman_encoder_response.schema.json`
