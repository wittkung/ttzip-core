# Data Model: Single-Core LZ77 Vector Match Finder

**Feature**: `121-single-core-lz77-vector-match-finder`
**Created**: 2026-08-19

---

## Entities & Type Definitions

### 1. `LZ77MatchFinderRequest`
Represents an invocation of the LZ77 match finder kernel.

| Field | Type | Required | Description | Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `input_size_bytes` | `uint64` | Yes | Total length of uncompressed input slice | Non-negative integer |
| `history_size_bytes` | `uint64` | Yes | Available preceding contiguous history in bytes | Range `[0, 32768]` |
| `tier_level` | `integer` | Yes | Compression tier identifier (1 for Fast) | Range `[1, 7]` |

### 2. `LZ77MatchFinderResponse`
Represents the result of LZ77 parsing.

| Field | Type | Required | Description | Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `tokens_emitted` | `uint64` | Yes | Total number of literal and match tokens produced | Non-negative integer |
| `literal_count` | `uint64` | Yes | Count of single-byte literal tokens | Non-negative integer |
| `match_count` | `uint64` | Yes | Count of length/distance match tokens | Non-negative integer |
| `elapsed_nanoseconds` | `uint64` | Yes | Measured execution duration in nanoseconds | Non-negative integer |

---

## JSON Schema Mapping
The models defined above map 1:1 with the schema definitions located in:
- `contracts/lz77_match_finder_request.schema.json`
- `contracts/lz77_match_finder_response.schema.json`
