# Contract: 7Z 500MB In-Process Compression Pipeline

```c
int ttzip_lzma2_compress_500m_fast_c(
    const char* output_path,
    const char* const* input_paths,
    size_t input_count,
    int level,
    const char* password
);
```
