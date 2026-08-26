// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

// Comprehensive C11 C-ABI Validation Test Suite.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include <unistd.h>
#include <sys/stat.h>
#include "../include/ttzip.h"

typedef struct {
    size_t count;
    char first_path[256];
    uint64_t total_uncompressed;
    uint32_t last_crc;
} InspectData;

static bool inspect_collector(const TTZipEntryMetadata *meta, void *user_data) {
    if (!meta || !user_data) return false;
    InspectData *data = (InspectData *)user_data;
    data->count++;
    data->total_uncompressed += meta->uncompressed_size;
    data->last_crc = meta->crc32;
    if (meta->path && data->first_path[0] == '\0') {
        strncpy(data->first_path, meta->path, sizeof(data->first_path) - 1);
    }
    return true;
}

typedef struct {
    size_t call_count;
    uint64_t last_processed;
    uint64_t last_total;
} ProgressData;

static bool progress_collector(uint64_t processed_bytes, uint64_t total_bytes, const char *current_entry, void *user_data) {
    if (!user_data) return true;
    ProgressData *data = (ProgressData *)user_data;
    data->call_count++;
    data->last_processed = processed_bytes;
    data->last_total = total_bytes;
    return true; // continue operation
}

static void create_test_file(const char *path, const char *content) {
    FILE *f = fopen(path, "wb");
    assert(f != NULL);
    fputs(content, f);
    fclose(f);
}

static char *read_test_file(const char *path) {
    FILE *f = fopen(path, "rb");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    fseek(f, 0, SEEK_SET);
    char *buf = (char *)malloc(sz + 1);
    assert(buf != NULL);
    size_t read_bytes = fread(buf, 1, sz, f);
    buf[read_bytes] = '\0';
    fclose(f);
    return buf;
}

int main(void) {
    printf("⚡️ Running TTZip Comprehensive Native C11 C-ABI Test Suite...\n");

    // 1. Version check
    const char *ver = ttzip_version();
    assert(ver != NULL);
    assert(strlen(ver) > 0);
    printf("  [PASS] C SDK ttzip_version(): %s\n", ver);

    // 2. Hardware acceleration check
    bool hw = ttzip_is_hardware_accelerated();
    printf("  [PASS] C SDK hardware acceleration: %s\n", hw ? "ENABLED" : "DISABLED");

    // 3. SIMD CRC-32 & CRC-64 verification
    const char *msg = "TTZip Ultra-Fast Native C11 C-ABI Hardware SIMD Checksums Benchmark";
    size_t msg_len = strlen(msg);

    uint32_t c32 = ttzip_crc32((const uint8_t *)msg, msg_len);
    assert(c32 != 0);

    // Incremental CRC32 via ttzip_rust_crc32
    size_t half = msg_len / 2;
    uint32_t seed = ttzip_rust_crc32(0, (const uint8_t *)msg, half);
    uint32_t chained32 = ttzip_rust_crc32(seed, (const uint8_t *)(msg + half), msg_len - half);
    assert(chained32 == c32);

    uint64_t c64 = ttzip_crc64((const uint8_t *)msg, msg_len);
    assert(c64 != 0);
    printf("  [PASS] C SDK SIMD Checksums CRC-32 (0x%08X) & CRC-64 (0x%016llX)\n", c32, (unsigned long long)c64);

    // 4. Archive Creation with Progress Callback
    const char *tmp_file1 = "/tmp/ttzip_c_file1.txt";
    const char *tmp_file2 = "/tmp/ttzip_c_file2.log";
    const char *content1 = "C11 Native SDK Test File 1 Content\n";
    const char *content2 = "C11 Native SDK Test File 2 Log Content Line\n";

    create_test_file(tmp_file1, content1);
    create_test_file(tmp_file2, content2);

    const char *sources[] = { tmp_file1, tmp_file2 };
    const char *archive_path = "/tmp/ttzip_c_test_archive.zip";

    ProgressData prog_data = { 0, 0, 0 };
    TTZipCreateOptions create_opts;
    memset(&create_opts, 0, sizeof(create_opts));
    create_opts.struct_size = sizeof(TTZipCreateOptions);
    create_opts.abi_version = 2;
    create_opts.format = TTZIP_ARCHIVE_FORMAT_ZIP;
    create_opts.level = TTZIP_COMPRESSION_LEVEL_NORMAL;
    create_opts.progress_callback = progress_collector;
    create_opts.user_data = &prog_data;

    TTZipStatus st = ttzip_create_archive(sources, 2, archive_path, &create_opts);
    if (st != TTZIP_STATUS_OK) {
        fprintf(stderr, "❌ Archive creation failed: %d (%s)\n", st, ttzip_status_string(st));
    }
    assert(st == TTZIP_STATUS_OK);
    printf("  [PASS] C SDK ttzip_create_archive() OK (Progress callback count: %zu)\n", prog_data.call_count);

    // 5. Archive Metadata Inspection
    InspectData inspect_data = { 0, "", 0, 0 };
    st = ttzip_inspect_archive(archive_path, NULL, true, inspect_collector, &inspect_data);
    assert(st == TTZIP_STATUS_OK);
    assert(inspect_data.count >= 2);
    assert(inspect_data.total_uncompressed > 0);
    printf("  [PASS] C SDK ttzip_inspect_archive() verified %zu entries (total %llu bytes)\n",
           inspect_data.count, (unsigned long long)inspect_data.total_uncompressed);

    // 6. Archive Extraction
    const char *extract_dir = "/tmp/ttzip_c_extracted_dir";
    TTZipExtractOptions extract_opts;
    memset(&extract_opts, 0, sizeof(extract_opts));
    extract_opts.struct_size = sizeof(TTZipExtractOptions);
    extract_opts.abi_version = 2;
    extract_opts.destination_path = extract_dir;
    extract_opts.overwrite_existing = true;
    extract_opts.preserve_permissions = true;

    st = ttzip_extract_archive(archive_path, extract_dir, &extract_opts);
    if (st != TTZIP_STATUS_OK) {
        fprintf(stderr, "❌ Extraction failed: %d (%s)\n", st, ttzip_status_string(st));
    }
    assert(st == TTZIP_STATUS_OK);

    char extracted_path1[512];
    snprintf(extracted_path1, sizeof(extracted_path1), "%s/ttzip_c_file1.txt", extract_dir);
    char *read_back1 = read_test_file(extracted_path1);
    assert(read_back1 != NULL);
    assert(strcmp(read_back1, content1) == 0);
    free(read_back1);

    printf("  [PASS] C SDK ttzip_extract_archive() payload match OK\n");

    // 7. Options Struct Configurations & Compression Levels
    TTZipCompressionLevel levels[] = {
        TTZIP_COMPRESSION_LEVEL_STORE,
        TTZIP_COMPRESSION_LEVEL_FASTEST,
        TTZIP_COMPRESSION_LEVEL_FAST,
        TTZIP_COMPRESSION_LEVEL_NORMAL,
        TTZIP_COMPRESSION_LEVEL_MAXIMUM,
        TTZIP_COMPRESSION_LEVEL_ULTRA
    };
    for (int i = 0; i < 6; i++) {
        char lvl_archive[256];
        snprintf(lvl_archive, sizeof(lvl_archive), "/tmp/ttzip_c_lvl_%d.zip", i);
        TTZipCreateOptions lvl_opts;
        memset(&lvl_opts, 0, sizeof(lvl_opts));
        lvl_opts.struct_size = sizeof(TTZipCreateOptions);
        lvl_opts.abi_version = 2;
        lvl_opts.format = TTZIP_ARCHIVE_FORMAT_ZIP;
        lvl_opts.level = levels[i];
        lvl_opts.thread_budget = 1;
        lvl_opts.solid_block_size_mb = 64;

        TTZipStatus lvl_st = ttzip_create_archive(sources, 2, lvl_archive, &lvl_opts);
        assert(lvl_st == TTZIP_STATUS_OK);
        unlink(lvl_archive);
    }
    printf("  [PASS] C SDK compression level configurations (0-12) & options struct OK\n");

    // 8. Memory Buffer Codecs (DEFLATE, Zstandard, LZ4, Snappy, LZFSE)
    const char *codec_payload = "TTZip In-Memory Buffer Codec SIMD Acceleration Payload 2026! Repeat text for compression ratio.\n";
    size_t raw_len = strlen(codec_payload);
    uint8_t comp_buf[4096];
    uint8_t decomp_buf[4096];
    size_t comp_len = 0;
    size_t decomp_len = 0;

    // DEFLATE
    st = ttzip_rust_deflate_compress((const uint8_t *)codec_payload, raw_len, comp_buf, sizeof(comp_buf), 6, &comp_len);
    assert(st == TTZIP_STATUS_OK && comp_len > 0);
    st = ttzip_rust_deflate_decompress(comp_buf, comp_len, decomp_buf, sizeof(decomp_buf), &decomp_len);
    assert(st == TTZIP_STATUS_OK && decomp_len == raw_len);
    assert(memcmp(decomp_buf, codec_payload, raw_len) == 0);

    // ZSTD
    st = ttzip_rust_zstd_compress((const uint8_t *)codec_payload, raw_len, comp_buf, sizeof(comp_buf), 3, &comp_len);
    assert(st == TTZIP_STATUS_OK && comp_len > 0);
    st = ttzip_rust_zstd_decompress(comp_buf, comp_len, decomp_buf, sizeof(decomp_buf), &decomp_len);
    assert(st == TTZIP_STATUS_OK && decomp_len == raw_len);
    assert(memcmp(decomp_buf, codec_payload, raw_len) == 0);

    // LZ4
    st = ttzip_rust_lz4_compress((const uint8_t *)codec_payload, raw_len, comp_buf, sizeof(comp_buf), &comp_len);
    assert(st == TTZIP_STATUS_OK && comp_len > 0);
    st = ttzip_rust_lz4_decompress(comp_buf, comp_len, decomp_buf, sizeof(decomp_buf), &decomp_len);
    assert(st == TTZIP_STATUS_OK && decomp_len == raw_len);
    assert(memcmp(decomp_buf, codec_payload, raw_len) == 0);

    // Snappy
    st = ttzip_rust_snappy_compress((const uint8_t *)codec_payload, raw_len, comp_buf, sizeof(comp_buf), &comp_len);
    assert(st == TTZIP_STATUS_OK && comp_len > 0);
    st = ttzip_rust_snappy_decompress(comp_buf, comp_len, decomp_buf, sizeof(decomp_buf), &decomp_len);
    assert(st == TTZIP_STATUS_OK && decomp_len == raw_len);
    assert(memcmp(decomp_buf, codec_payload, raw_len) == 0);

    // LZFSE
    st = ttzip_rust_lzfse_compress((const uint8_t *)codec_payload, raw_len, comp_buf, sizeof(comp_buf), &comp_len);
    assert(st == TTZIP_STATUS_OK && comp_len > 0);
    st = ttzip_rust_lzfse_decompress(comp_buf, comp_len, decomp_buf, sizeof(decomp_buf), &decomp_len);
    assert(st == TTZIP_STATUS_OK && decomp_len == raw_len);
    assert(memcmp(decomp_buf, codec_payload, raw_len) == 0);

    printf("  [PASS] C SDK in-memory buffer codecs (DEFLATE, ZSTD, LZ4, Snappy, LZFSE) OK\n");

    // 9. Password-Protected Archive (AES-256)
    const char *pwd_archive = "/tmp/ttzip_c_encrypted.zip";
    const char *pwd_extract_valid = "/tmp/ttzip_c_pwd_valid";
    const char *pwd_extract_invalid = "/tmp/ttzip_c_pwd_invalid";
    const char *secret_pass = "TTZipSecretKey2026!";
    const char *wrong_pass = "WrongSecretPass!";

    TTZipCreateOptions pwd_create_opts;
    memset(&pwd_create_opts, 0, sizeof(pwd_create_opts));
    pwd_create_opts.struct_size = sizeof(TTZipCreateOptions);
    pwd_create_opts.abi_version = 2;
    pwd_create_opts.format = TTZIP_ARCHIVE_FORMAT_ZIP;
    pwd_create_opts.level = TTZIP_COMPRESSION_LEVEL_NORMAL;
    pwd_create_opts.encryption = TTZIP_ENCRYPTION_AES256;
    pwd_create_opts.password = secret_pass;

    st = ttzip_create_archive(sources, 2, pwd_archive, &pwd_create_opts);
    assert(st == TTZIP_STATUS_OK);

    // Extract with correct password
    TTZipExtractOptions pwd_ext_valid;
    memset(&pwd_ext_valid, 0, sizeof(pwd_ext_valid));
    pwd_ext_valid.struct_size = sizeof(TTZipExtractOptions);
    pwd_ext_valid.abi_version = 2;
    pwd_ext_valid.destination_path = pwd_extract_valid;
    pwd_ext_valid.password = secret_pass;
    pwd_ext_valid.overwrite_existing = true;
    st = ttzip_extract_archive(pwd_archive, pwd_extract_valid, &pwd_ext_valid);
    assert(st == TTZIP_STATUS_OK);

    // Extract with wrong password
    TTZipExtractOptions pwd_ext_invalid;
    memset(&pwd_ext_invalid, 0, sizeof(pwd_ext_invalid));
    pwd_ext_invalid.struct_size = sizeof(TTZipExtractOptions);
    pwd_ext_invalid.abi_version = 2;
    pwd_ext_invalid.destination_path = pwd_extract_invalid;
    pwd_ext_invalid.password = wrong_pass;
    pwd_ext_invalid.overwrite_existing = true;
    st = ttzip_extract_archive(pwd_archive, pwd_extract_invalid, &pwd_ext_invalid);
    assert(st != TTZIP_STATUS_OK);

    unlink(pwd_archive);
    printf("  [PASS] C SDK AES-256 password protection & authentication error handling OK\n");

    // 10. Error Status String Validation
    assert(strcmp(ttzip_status_string(TTZIP_STATUS_OK), "OK") == 0);
    assert(strcmp(ttzip_status_string(TTZIP_STATUS_ERR_FILE_NOT_FOUND), "File Not Found") == 0);
    assert(strcmp(ttzip_status_string(TTZIP_STATUS_ERR_SECURITY_VIOLATION), "Security Violation") == 0);
    printf("  [PASS] C SDK ttzip_status_string() error mappings OK\n");

    // Cleanup
    unlink(tmp_file1);
    unlink(tmp_file2);
    unlink(archive_path);
    unlink(extracted_path1);

    printf("✅ All C11 C-ABI validation tests passed successfully!\n");
    return 0;
}
