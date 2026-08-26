# Data Model: 7Z Comprehensive Conquest

## 1. 7z Multi-Core Chunk Pipeline Context

```c
typedef struct {
    const uint8_t* in_data;
    size_t in_size;
    uint8_t* out_data;
    size_t out_cap;
    size_t out_size;
    int status;
    
    // AES In-Place context
    const uint8_t* aes_key;
    const uint8_t* aes_iv;
    int enable_encryption;
} ttzip_7z_chunk_task_t;
```

## 2. 7z Compression Stream Job

```c
typedef struct {
    size_t num_chunks;
    ttzip_7z_chunk_task_t* chunks;
    pthread_t* workers;
    int p_cores;
    size_t total_input_bytes;
} ttzip_7z_stream_job_t;
```
