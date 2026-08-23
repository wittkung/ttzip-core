# Data Model & Pipeline Architecture: Feature 009

## 1. 7Z Multi-block Zero-Copy Pipeline
```
[500MB Source File]
        │
   (mmap zero-copy)
        │
   [solid_buf (500MB, 32 Blocks)]
        │
  ┌─────┴───────────────────────────────────────┐
  │                                             │
[POSIX pthread KDF]                    [32-Core Parallel Dispatch]
  2^19 SHA-256 (6.8ms)                   - NEON is_zero_block check
  (concurrent with compression)          - In-place CRC32 computation
                                         - lzma_raw_buffer_encode
                                                │
                                         [Compressed Blocks (~70KB)]
                                                │
                                         [In-place ARMv8 AES-256 (5μs)]
                                                │
                                         [Single writev(out_fd)]
                                                │
                                         [7Z Header Write (0.1ms)]
```

## 2. TAR.ZST Direct Multi-threaded Streaming Architecture
```
[archive.tar.zst File]
        │
   (open + read)
        │
  [ZSTD_decompressStream (Direct C)]
        │
  [Direct TAR 512B Header Parser]
        │
  ┌─────┴─────────────────────────┐
  │                               │
[Single Large File (500MB)]    [Multi-Files (100 Files)]
  ftruncate + mmap write         open + writev + close
```
