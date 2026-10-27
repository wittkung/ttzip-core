// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include <unistd.h>
#include <sys/stat.h>
#include <ftw.h>
#include <limits.h>
#include "../include/ttzip.h"

static int remove_entry_cb(
    const char *fpath,
    const struct stat *sb,
    int typeflag,
    struct FTW *ftwbuf
) {
    (void)sb;
    (void)typeflag;
    (void)ftwbuf;
    return remove(fpath);
}

static void remove_sandbox_tree(const char *sandbox_dir) {
    if (!sandbox_dir || sandbox_dir[0] == '\0') {
        return;
    }
    (void)nftw(sandbox_dir, remove_entry_cb, 64, FTW_DEPTH | FTW_PHYS);
}

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
    (void)current_entry;
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

    // Create self-owned unique temporary sandbox directory (never touch shared /tmp paths)
    char sandbox_template[] = "/tmp/ttzip_c_sdk_XXXXXX";
    char *sandbox_dir = mkdtemp(sandbox_template);
    assert(sandbox_dir != NULL);

    // 4. Archive Creation with Progress Callback
    char tmp_file1[PATH_MAX];
    char tmp_file2[PATH_MAX];
    char archive_path[PATH_MAX];
    snprintf(tmp_file1, sizeof(tmp_file1), "%s/ttzip_c_file1.txt", sandbox_dir);
    snprintf(tmp_file2, sizeof(tmp_file2), "%s/ttzip_c_file2.log", sandbox_dir);
    snprintf(archive_path, sizeof(archive_path), "%s/ttzip_c_test_archive.zip", sandbox_dir);
    const char *content1 = "C11 Native SDK Test File 1 Content\n";
    const char *content2 = "C11 Native SDK Test File 2 Log Content Line\n";

    create_test_file(tmp_file1, content1);
    create_test_file(tmp_file2, content2);

    const char *sources[] = { tmp_file1, tmp_file2 };

    ProgressData prog_data = { 0, 0, 0 };
    TTZipCreateOptions create_opts;
    ttzip_create_options_init(&create_opts);
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
    char extract_dir[PATH_MAX];
    snprintf(extract_dir, sizeof(extract_dir), "%s/ttzip_c_extracted_dir", sandbox_dir);
    TTZipExtractOptions extract_opts;
    ttzip_extract_options_init(&extract_opts);
    extract_opts.destination_path = extract_dir;
    extract_opts.collision_policy = TTZIP_COLLISION_POLICY_REPLACE;
    extract_opts.preserve_permissions = true;

    st = ttzip_extract_archive(archive_path, extract_dir, &extract_opts);
    if (st != TTZIP_STATUS_OK) {
        fprintf(stderr, "❌ Extraction failed: %d (%s)\n", st, ttzip_status_string(st));
    }
    assert(st == TTZIP_STATUS_OK);

    char extracted_path1[PATH_MAX];
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
        char lvl_archive[PATH_MAX];
        snprintf(lvl_archive, sizeof(lvl_archive), "%s/ttzip_c_lvl_%d.zip", sandbox_dir, i);
        TTZipCreateOptions lvl_opts;
        ttzip_create_options_init(&lvl_opts);
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

    // Brotli
    st = ttzip_rust_brotli_compress((const uint8_t *)codec_payload, raw_len, comp_buf, sizeof(comp_buf), 6, 22, &comp_len);
    assert(st == TTZIP_STATUS_OK && comp_len > 0);
    st = ttzip_rust_brotli_decompress(comp_buf, comp_len, decomp_buf, sizeof(decomp_buf), &decomp_len);
    assert(st == TTZIP_STATUS_OK && decomp_len == raw_len);
    assert(memcmp(decomp_buf, codec_payload, raw_len) == 0);

    // Fast-LZMA2
    st = ttzip_rust_fl2_compress((const uint8_t *)codec_payload, raw_len, comp_buf, sizeof(comp_buf), 3, 1, &comp_len);
    assert(st == TTZIP_STATUS_OK && comp_len > 0);
    st = ttzip_rust_fl2_decompress(comp_buf, comp_len, decomp_buf, sizeof(decomp_buf), 1, &decomp_len);
    assert(st == TTZIP_STATUS_OK && decomp_len == raw_len);
    assert(memcmp(decomp_buf, codec_payload, raw_len) == 0);

    // Bzip2
    st = ttzip_rust_bzip2_compress((const uint8_t *)codec_payload, raw_len, comp_buf, sizeof(comp_buf), 6, &comp_len);
    assert(st == TTZIP_STATUS_OK && comp_len > 0);
    st = ttzip_rust_bzip2_decompress(comp_buf, comp_len, decomp_buf, sizeof(decomp_buf), &decomp_len);
    assert(st == TTZIP_STATUS_OK && decomp_len == raw_len);
    assert(memcmp(decomp_buf, codec_payload, raw_len) == 0);

    printf("  [PASS] C SDK in-memory buffer codecs (DEFLATE, ZSTD, LZ4, Snappy, LZFSE, Brotli, FL2, Bzip2) OK\n");

    // 8.1 Advanced Hashes & Ciphers Verification
    uint64_t xxh64 = ttzip_rust_xxh3_64((const uint8_t *)codec_payload, raw_len, 0);
    assert(xxh64 != 0);

    uint8_t xxh128[16];
    assert(ttzip_rust_xxh3_128((const uint8_t *)codec_payload, raw_len, 0, xxh128) == 0);

    uint8_t blake3_out[32];
    assert(ttzip_rust_blake3((const uint8_t *)codec_payload, raw_len, blake3_out) == 0);

    uint8_t sha256_out[32];
    assert(ttzip_rust_sha256((const uint8_t *)codec_payload, raw_len, sha256_out) == 0);

    uint8_t md5_out[16];
    assert(ttzip_rust_md5((const uint8_t *)codec_payload, raw_len, md5_out) == 0);

    uint8_t sha1_out[20];
    assert(ttzip_rust_sha1((const uint8_t *)codec_payload, raw_len, sha1_out) == 0);

    // ChaCha20-Poly1305 AEAD
    uint8_t chacha_key[32] = {0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20};
    uint8_t chacha_nonce[12] = {0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b};
    uint8_t chacha_cipher[512];
    uint8_t chacha_tag[16];
    uint8_t chacha_plain[512];

    assert(ttzip_rust_chacha20_poly1305_encrypt(chacha_key, chacha_nonce, (const uint8_t *)codec_payload, raw_len, NULL, 0, chacha_cipher, chacha_tag) == 0);
    assert(ttzip_rust_chacha20_poly1305_decrypt(chacha_key, chacha_nonce, chacha_cipher, raw_len, NULL, 0, chacha_tag, chacha_plain) == 0);
    assert(memcmp(chacha_plain, codec_payload, raw_len) == 0);

    printf("  [PASS] C SDK hardware hashes (XXH3, BLAKE3, SHA256, MD5, SHA1, ChaCha20-Poly1305) OK\n");

    // 9. Password-Protected Archive (AES-256)
    char pwd_archive[PATH_MAX];
    char pwd_extract_valid[PATH_MAX];
    char pwd_extract_invalid[PATH_MAX];
    snprintf(pwd_archive, sizeof(pwd_archive), "%s/ttzip_c_encrypted.zip", sandbox_dir);
    snprintf(pwd_extract_valid, sizeof(pwd_extract_valid), "%s/ttzip_c_pwd_valid", sandbox_dir);
    snprintf(pwd_extract_invalid, sizeof(pwd_extract_invalid), "%s/ttzip_c_pwd_invalid", sandbox_dir);
    const char *secret_pass = "TTZipSecretKey2026!";
    const char *wrong_pass = "WrongSecretPass!";

    TTZipCreateOptions pwd_create_opts;
    ttzip_create_options_init(&pwd_create_opts);
    pwd_create_opts.format = TTZIP_ARCHIVE_FORMAT_ZIP;
    pwd_create_opts.level = TTZIP_COMPRESSION_LEVEL_NORMAL;
    pwd_create_opts.encryption = TTZIP_ENCRYPTION_AES256;
    pwd_create_opts.password = secret_pass;

    st = ttzip_create_archive(sources, 2, pwd_archive, &pwd_create_opts);
    assert(st == TTZIP_STATUS_OK);

    // Extract with correct password
    TTZipExtractOptions pwd_ext_valid;
    ttzip_extract_options_init(&pwd_ext_valid);
    pwd_ext_valid.destination_path = pwd_extract_valid;
    pwd_ext_valid.password = secret_pass;
    pwd_ext_valid.collision_policy = TTZIP_COLLISION_POLICY_REPLACE;
    st = ttzip_extract_archive(pwd_archive, pwd_extract_valid, &pwd_ext_valid);
    assert(st == TTZIP_STATUS_OK);

    // Extract with wrong password
    TTZipExtractOptions pwd_ext_invalid;
    ttzip_extract_options_init(&pwd_ext_invalid);
    pwd_ext_invalid.destination_path = pwd_extract_invalid;
    pwd_ext_invalid.password = wrong_pass;
    pwd_ext_invalid.collision_policy = TTZIP_COLLISION_POLICY_REPLACE;
    st = ttzip_extract_archive(pwd_archive, pwd_extract_invalid, &pwd_ext_invalid);
    assert(st != TTZIP_STATUS_OK);

    unlink(pwd_archive);
    printf("  [PASS] C SDK AES-256 password protection & authentication error handling OK\n");

    // 10. Error Status String Validation
    assert(strcmp(ttzip_status_string(TTZIP_STATUS_OK), "OK") == 0);
    assert(strcmp(ttzip_status_string(TTZIP_STATUS_ERR_FILE_NOT_FOUND), "File Not Found") == 0);
    assert(strcmp(ttzip_status_string(TTZIP_STATUS_ERR_SECURITY_VIOLATION), "Security Violation") == 0);
    printf("  [PASS] C SDK ttzip_status_string() error mappings OK\n");

    // Cleanup self-owned sandbox directory
    remove_sandbox_tree(sandbox_dir);

    printf("✅ All C11 C-ABI validation tests passed successfully!\n");
    return 0;
}
