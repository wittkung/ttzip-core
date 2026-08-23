# Data Model: 7Z 500MB Peak Architecture

```c
typedef struct {
    size_t chunk_index;
    const uint8_t* in_ptr;
    size_t in_size;
    uint8_t* out_ptr;
    size_t out_cap;
    size_t out_len;
    uint32_t dict_size;
    int status;
} ttzip_500m_chunk_job_t;
```
