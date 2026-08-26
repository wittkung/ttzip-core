# Data Model & Interface Specifications: Feature 172

## 1. Converged C Bridge Exports (`Sources/CTTZipBridge/CTTZipBridge.c`)

```c
// POSIX Fast Spawn Wrapper
int ttzip_core_posix_spawn_fast(const char* bin_path, char* const argv[], const char* work_dir);

// Reed-Solomon Erasure Coding Engine
int ttzip_rs_create_cauchy_matrix(uint8_t* matrix, int k, int m);
int ttzip_rs_encode_neon(const uint8_t* const* data_blocks, uint8_t* const* parity_blocks, int k, int m, size_t block_size);
int ttzip_rs_decode_neon(uint8_t* const* blocks, const int* erased_indices, int num_erased, int k, int m, size_t block_size);

// Zopfli Ultra Deflate Wrapper
size_t ttzip_zopfli_compress_block_with_history(
    const uint8_t* in, size_t insize,
    uint8_t* out, size_t out_capacity,
    int num_iterations
);

// CRC64 Checksum (ARM64 PMULL / Scalar)
uint64_t ttzip_crc64(uint64_t crc, const uint8_t* buf, size_t len);

// Magic Header Sniffer
int ttzip_magic_sniff_buffer(const uint8_t* buf, size_t len, ttzip_file_info_t* out_info);
```

## 2. Rust Unified C-ABI Exports (`ttzip_rust_glue.h`)

All core functionalities (CRC32, Adler32, AES256, SHA256, Deflate, Zstd, LZMA2, LZFSE, Snappy, Inspect, Extract, Create) are provided by `ttzip_rust_*` C-ABI functions.
