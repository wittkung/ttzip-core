#ifndef TTZIP_RUST_SWIFT_C_ABI_H
#define TTZIP_RUST_SWIFT_C_ABI_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    const char *name;
    uint64_t uncompressed_size;
    uint64_t compressed_size;
    uint32_t crc32;
    int64_t mtime;
    bool is_directory;
    bool is_encrypted;
    uint16_t compression_method;
} TTZipEntryMetadata;

typedef struct {
    bool is_valid;
    const char *format_name;
    size_t total_entries;
    size_t corrupted_entries_count;
    const char *error_message;
} TTZipIntegrityReport;

typedef struct {
    uint64_t processed_bytes;
    uint64_t total_bytes;
    uint64_t processed_files;
    uint64_t total_files;
    double throughput_mb_s;
    float percent_complete;
} TTZipProgressUpdate;

typedef void (*TTZipProgressCallback)(const TTZipProgressUpdate *update, void *context);

int32_t ttzip_core_init(void);
void ttzip_core_shutdown(void);

#ifdef __cplusplus
}
#endif

#endif /* TTZIP_RUST_SWIFT_C_ABI_H */
