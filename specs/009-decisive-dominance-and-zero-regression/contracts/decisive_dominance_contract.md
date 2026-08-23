# Interface Contract: Decisive Dominance Pipeline

## 1. 7Z C Engine Contract
```c
int ttzip_create_7z_lzma2_native_c(
    const char* output_path,
    const char* const* input_paths,
    size_t input_count,
    int level,
    const char* password
);
```

## 2. TAR.ZST Direct C Engine Contract
```c
int ttzip_create_tar_zstd_direct_c(
    const char* output_path,
    const char* const* input_paths,
    size_t input_count,
    int level
);

int ttzip_extract_tar_zstd_direct_c(
    const char* archive_path,
    const char* destination_dir
);
```
