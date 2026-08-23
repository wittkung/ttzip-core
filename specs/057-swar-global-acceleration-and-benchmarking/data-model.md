# Phase 1 Data Model: Global SWAR & Pattern Acceleration

## 1. Encoding Detection Model
```c
// Return constant literal string: "UTF-8" or "GB18030"
const char* ttzip_detect_encoding_fast(const uint8_t* bytes, size_t len);
```

## 2. Format Sniffing Model
```c
typedef enum {
    TTZIP_NATIVE_FMT_UNKNOWN = 0,
    TTZIP_NATIVE_FMT_ZIP     = 1,
    TTZIP_NATIVE_FMT_7Z      = 2,
    TTZIP_NATIVE_FMT_TAR     = 3,
    TTZIP_NATIVE_FMT_GZ      = 4,
    TTZIP_NATIVE_FMT_ZSTD    = 5,
    TTZIP_NATIVE_FMT_LZ4     = 6,
    TTZIP_NATIVE_FMT_XZ      = 7,
    TTZIP_NATIVE_FMT_BZ2     = 8,
} ttzip_native_fmt_t;

ttzip_native_fmt_t ttzip_detect_format_from_header(const uint8_t* buffer, size_t len);
```

## 3. SWAR Bit-Mask Invariants
* `ASCII_MASK_64 = 0x8080808080808080ULL`
* 当 `(word64 & ASCII_MASK_64) == 0` 时，8 字节全部属于 `0x00 ~ 0x7F` 的合法 ASCII 字节。
