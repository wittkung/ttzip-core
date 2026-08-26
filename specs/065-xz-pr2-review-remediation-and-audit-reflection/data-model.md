# Data Model: XZ PR 2 Verification Entities

## Entities

### TestVector
- `name`: string (e.g., "Standard ECMA-182 Golden Vector")
- `input_hex`: string (e.g., "313233343536373839" for "123456789")
- `size_bytes`: integer (e.g., 9)
- `initial_crc`: integer (0)
- `expected_crc64`: string (e.g., "0x6C40DF5F0B497347")

### BenchmarkRun
- `cpu_model`: string (e.g., "Apple M5 Max")
- `dataset_size_gb`: number (e.g., 3.12)
- `iterations`: integer (e.g., 50)
- `generic_time_sec`: number
- `generic_throughput_mbs`: number
- `pmull_time_sec`: number
- `pmull_throughput_mbs`: number
- `speedup_factor`: number
- `bit_exact_parity`: boolean
