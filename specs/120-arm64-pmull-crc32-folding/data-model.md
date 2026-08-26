# Data Model: ARM64 PMULL / CRC32 Multi-Way Folding & Cache Fusion

**Feature**: `120-arm64-pmull-crc32-folding`
**Created**: 2026-08-19

---

## Entities & Type Definitions

### 1. `CRC32ComputationRequest`
Represents an invocation of hardware-accelerated CRC-32 checksumming.

| Field | Type | Required | Description | Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `initial_crc` | `uint32` | Yes | Initial CRC-32 seed (0 for first chunk) | Range `[0, 4294967295]` |
| `buffer_size_bytes` | `uint64` | Yes | Length of data buffer in bytes | Non-negative integer |
| `alignment_offset` | `uint8` | Yes | Memory address modulo 64 alignment offset | Range `[0, 63]` |

### 2. `CRC32ComputationResponse`
Represents the result and diagnostics of a CRC-32 calculation.

| Field | Type | Required | Description | Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `final_crc` | `uint32` | Yes | Computed 32-bit CRC-32 checksum (IEEE 802.3) | Range `[0, 4294967295]` |
| `bytes_processed` | `uint64` | Yes | Total bytes accumulated into checksum | Matches `buffer_size_bytes` |
| `execution_mode` | `string` | Yes | Selected kernel execution pipeline | Enum: `["pmull_12way_eor3", "pmull_4way", "armv8_crc32_scalar", "scalar_fallback"]` |
| `elapsed_nanoseconds` | `uint64` | Yes | Measured execution duration in nanoseconds | Non-negative integer |

---

## JSON Schema Mapping
The models defined above map 1:1 with the schema definitions located in:
- `contracts/crc32_computation_request.schema.json`
- `contracts/crc32_computation_response.schema.json`
