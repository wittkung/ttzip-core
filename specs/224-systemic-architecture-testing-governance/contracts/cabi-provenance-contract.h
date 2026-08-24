// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
// TTZip C-ABI Execution Provenance & Telemetry Contract

#ifndef TTZIP_PROVENANCE_CONTRACT_H
#define TTZIP_PROVENANCE_CONTRACT_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    TTZIP_ENGINE_UNKNOWN = 0,
    TTZIP_ENGINE_RUST_RAYON_PARALLEL_ZIP = 1,
    TTZIP_ENGINE_RUST_STREAMING_PARALLEL_ZIP = 2,
    TTZIP_ENGINE_RUST_ZERO_COPY_7Z_DECODER = 3,
    TTZIP_ENGINE_RUST_PURE_7Z_ENCODER = 4,
    TTZIP_ENGINE_RUST_TAR_STREAM = 5,
    TTZIP_ENGINE_RUST_IN_PLACE_ZIP = 6,
    TTZIP_ENGINE_RUST_IN_PLACE_7Z = 7,
    TTZIP_ENGINE_RUST_VFS_PARALLEL_SCANNER = 8,
    TTZIP_ENGINE_LIBARCHIVE_LEGACY = 100,
    TTZIP_ENGINE_CLI_7Z_FALLBACK = 101,
    TTZIP_ENGINE_SYSTEM_TAR_FALLBACK = 102
} TTZipEngineTag;

typedef struct {
    TTZipEngineTag engine_tag;
    uint32_t thread_count;
    uint64_t uncompressed_bytes;
    uint64_t compressed_bytes;
    uint64_t kernel_duration_nanos;
    bool is_fallback;
    char fallback_reason[128];
} TTZipExecutionProvenance;

bool ttzip_rust_get_last_execution_provenance(TTZipExecutionProvenance *out_provenance);
const char *ttzip_rust_engine_tag_name(TTZipEngineTag tag);

#ifdef __cplusplus
}
#endif

#endif // TTZIP_PROVENANCE_CONTRACT_H
