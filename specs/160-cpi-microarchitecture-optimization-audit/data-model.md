# Data Model: Comprehensive CPI & Microarchitectural Optimization Audit

**Feature ID**: `160-cpi-microarchitecture-optimization-audit`  
**Created**: 2026-08-20  
**Status**: Ready for Tasks  

---

## 1. Microarchitectural Entities & Structures

### 1.1 `ttzip_cpi_metric_t` (Microarchitectural Telemetry Data Point)
Represents a single microarchitectural measurement record for a codec or primitive kernel over an evaluation buffer.

| Field Name | C Type | JSON Schema Type | Description | Invariants & Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `kernel_name` | `char[32]` | `string` | Identifier of the kernel (e.g. `pmull_crc32_12way`) | Non-empty, max 31 chars + null |
| `buffer_bytes` | `size_t` | `integer` | Size of evaluated buffer in bytes | Minimum: 1, Maximum: $2^{31}-1$ |
| `elapsed_nanos` | `uint64_t` | `integer` | Total execution time in nanoseconds | Monotonic clock $> 0$ |
| `cycles_per_byte` | `double` | `number` | Calculated Cycles Per Byte ($\text{CPB}$) | $\ge 0.0$, finite |
| `throughput_mbs` | `double` | `number` | Throughput in Megabytes per second ($\text{MB/s}$) | $\ge 0.0$, finite |
| `estimated_ipc` | `double` | `number` | Estimated Instructions Per Cycle ($\text{IPC}$) | Range: $0.0 \sim 10.0$ |
| `estimated_cpi` | `double` | `number` | Estimated Cycles Per Instruction ($\text{CPI}$) | Range: $0.1 \sim 100.0$ |

### 1.2 `ttzip_prefetch_slot_t` (Cache-Line Aligned Atomic Prefetch Slot)
Represents a prefetch buffer slot engineered to eliminate L1D cache false sharing across CPU cores.

| Field Name | C Type | Alignment / Constraint | Description |
| :--- | :--- | :--- | :--- |
| `state` | `atomic_int` | 4 bytes | Slot lifecycle (`EMPTY`, `LOADING`, `READY`, `CONSUMING`) |
| `chunk_index` | `size_t` | 8 bytes | Sequential 0-based chunk ID |
| `data` | `uint8_t*` | 8 bytes | 64-byte aligned payload buffer pointer |
| `length` | `size_t` | 8 bytes | Valid data length in slot |
| `_pad` | `uint8_t[36]` | 36 bytes | Total struct size = exactly 64 bytes (`__attribute__((aligned(64)))`) |

---

## 2. Telemetry State Machine & Transformations

```
  [Raw Benchmark Run]
          │
          ▼
   (elapsed_nanos, bytes, nominal_freq_ghz)
          │
          ├─────────────────────────────────────────┐
          ▼                                         ▼
   [Throughput Model]                      [Microarchitecture Model]
   throughput_mbs = (bytes*1e9)/(nanos*2^20)   cycles = nanos * freq_ghz
   ratio_pct = (comp_bytes*100)/orig_bytes    cpb = cycles / bytes
          │                                         │
          └───────────────────┬─────────────────────┘
                              ▼
                    [ttzip_cpi_metric_t]
                              │
                              ▼
                   [Structured JSON Output]
```
