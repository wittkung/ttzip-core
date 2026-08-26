# Data Model: Block-Splitting & Cost Evaluation

**Feature**: `123-block-splitting-and-cost-evaluation`
**Created**: 2026-08-19

---

## Entities & Type Definitions

### 1. `HuffmanCostEvaluationRequest`
Represents an invocation to evaluate static vs. dynamic bit costs.

| Field | Type | Required | Description | Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `litlen_symbol_count` | `uint64` | Yes | Count of literal/length symbols | Non-negative integer |
| `offset_symbol_count` | `uint64` | Yes | Count of distance/offset symbols | Non-negative integer |
| `chunk_size_bytes` | `uint64` | Yes | Size of current chunk in bytes | Non-negative integer |

### 2. `HuffmanCostEvaluationResponse`
Represents the cost calculation results.

| Field | Type | Required | Description | Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `static_bit_cost` | `uint64` | Yes | Total estimated bits under static Huffman codes | Non-negative integer |
| `dynamic_bit_cost` | `uint64` | Yes | Total estimated bits under dynamic Huffman codes (including header) | Non-negative integer |
| `selected_mode` | `string` | Yes | Chosen encoding mode (`static` or `dynamic`) | Enum: `static`, `dynamic` |

---

## JSON Schema Mapping
The models defined above map 1:1 with:
- `contracts/cost_evaluation_request.schema.json`
- `contracts/cost_evaluation_response.schema.json`
