# Contract: C Bridge 7Z In-Place Pipeline API

## Function Signatures

```c
/**
 * @brief Compress raw buffer using multi-core LZMA2 and in-place AES-256 pipeline
 * 
 * @param output_path Target .7z archive path
 * @param input_data Pointer to continuous uncompressed buffer
 * @param input_size Total uncompressed size in bytes
 * @param level Compression level (1 = fastest, 6 = normal, 9 = ultra)
 * @param password Optional encryption password (NULL for no encryption)
 * @return int 0 on success, negative error code on failure
 */
int ttzip_7z_compress_buffer_direct(
    const char* output_path,
    const uint8_t* input_data,
    size_t input_size,
    int level,
    const char* password
);
```
