# Contract: 7Z Grand Slam Pipeline Interface

## 1. C-Bridge API Contract

```c
#ifndef TTZIP_7Z_GRAND_SLAM_CONTRACT_H
#define TTZIP_7Z_GRAND_SLAM_CONTRACT_H

#include <stddef.h>
#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

int ttzip_create_7z_archive_native_parallel(
    const char* output_path,
    const char** input_paths,
    size_t num_inputs,
    int level,
    const char* password,
    void (*progress_callback)(double progress, void* user_data),
    void* user_data
);

#ifdef __cplusplus
}
#endif

#endif // TTZIP_7Z_GRAND_SLAM_CONTRACT_H
```

## 2. Invariants & Performance Contract

- **Zero-Allocation**: No dynamic heap allocations inside `dispatch_apply` workers.
- **P-Core and E-Core Full Saturation**: 100% core pipeline utilization without idle stall.
- **Throughput Guarantees**:
  - 500MB L1: $\ge 5,800\text{ MB/s}$ (Debug) / $\ge 6,500\text{ MB/s}$ (Release)
  - 500MB L1 AES: $\ge 5,600\text{ MB/s}$ (Debug) / $\ge 6,200\text{ MB/s}$ (Release)
  - 10MB Logs L1: $\ge 2,800\text{ MB/s}$ (Debug) / $\ge 3,500\text{ MB/s}$ (Release)
