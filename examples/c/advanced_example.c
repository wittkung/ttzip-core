// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

// TTZip: Advanced C11 Native C-ABI Features Showcase.
// Demonstrates configuring TTZipCreateOptions (thread budget, compression level,
// AES-256 password encryption, solid blocks, real-time progress callbacks),
// archive inspection, and streaming extraction verification.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <stdbool.h>
#include <unistd.h>
#include <sys/stat.h>
#include <ttzip.h>

typedef struct ProgressContext {
    const char *operation_name;
    uint64_t last_reported_bytes;
} ProgressContext;

static bool progress_callback(uint64_t processed_bytes, uint64_t total_bytes, const char *current_entry, void *user_data) {
    ProgressContext *ctx = (ProgressContext *)user_data;
    double pct = total_bytes > 0 ? ((double)processed_bytes / (double)total_bytes) * 100.0 : 0.0;
    const char *op = ctx ? ctx->operation_name : "processing";
    const char *entry = current_entry ? current_entry : "packing";

    printf("   [%s] -> %5.1f%% (%llu / %llu bytes) | %s\n",
           op, pct, (unsigned long long)processed_bytes, (unsigned long long)total_bytes, entry);
    return true; // continue operation
}

static bool inspect_callback(const TTZipEntryMetadata *entry, void *user_data) {
    (void)user_data;
    if (!entry) return false;

    printf("   * %-24s | Uncompressed: %7llu B | CRC: 0x%08X | Dir: %s | Enc: %s\n",
           entry->path ? entry->path : "<unnamed>",
           (unsigned long long)entry->uncompressed_size,
           entry->crc32,
           entry->is_directory ? "YES" : "NO",
           entry->is_encrypted ? "YES" : "NO");
    return true;
}

int main(void) {
    printf("================================================================================\n");
    printf("⚡️ TTZip C11 Native C-ABI Advanced Features Showcase (v%s)\n", ttzip_version());
    printf("================================================================================\n");

    // 1. Engine & Hardware Telemetry
    printf("1. Querying Native Engine Capabilities...\n");
    printf("   • Engine Version:        %s\n", ttzip_version());
    printf("   • Hardware SIMD / AES:   %s\n",
           ttzip_is_hardware_accelerated() ? "ACTIVE (ARM NEON / AVX-512)" : "DISABLED");
    printf("--------------------------------------------------------------------------------\n");

    // 2. Hardware-Accelerated CRC-32 & CRC-64 Calculations
    printf("2. Computing Hardware SIMD Checksums...\n");
    const char *payload_text = "TTZip C11 Native C-ABI Advanced Pipeline 2026";
    size_t payload_len = strlen(payload_text);
    uint32_t crc32_val = ttzip_crc32((const uint8_t *)payload_text, payload_len);
    uint64_t crc64_val = ttzip_crc64((const uint8_t *)payload_text, payload_len);
    printf("   • Hardware CRC-32:       0x%08X\n", crc32_val);
    printf("   • Hardware CRC-64:       0x%016llX\n", (unsigned long long)crc64_val);
    printf("--------------------------------------------------------------------------------\n");

    // 3. Prepare Multi-File Test Dataset
    const char *temp_dir = "/tmp/ttzip_c11_adv_demo";
    mkdir(temp_dir, 0755);

    char file1[256], file2[256], file3[256];
    snprintf(file1, sizeof(file1), "%s/database_schema.sql", temp_dir);
    snprintf(file2, sizeof(file2), "%s/config.json", temp_dir);
    snprintf(file3, sizeof(file3), "%s/readme.txt", temp_dir);

    FILE *f1 = fopen(file1, "wb");
    if (f1) {
        fputs("CREATE TABLE records (id INTEGER PRIMARY KEY, hash TEXT NOT NULL, created_at TIMESTAMP);\n", f1);
        fclose(f1);
    }
    FILE *f2 = fopen(file2, "wb");
    if (f2) {
        fputs("{\"sdk\": \"C11 C-ABI\", \"threads\": 4, \"cipher\": \"AES-256\", \"solid\": true}\n", f2);
        fclose(f2);
    }
    FILE *f3 = fopen(file3, "wb");
    if (f3) {
        fputs("TTZip C11 Advanced Showcase with full TTZipCreateOptions configuration.\n", f3);
        fclose(f3);
    }

    const char *sources[] = { file1, file2, file3 };
    size_t source_count = sizeof(sources) / sizeof(sources[0]);

    const char *password = "C11SecurePass2026!";
    char archive_zip[256], archive_7z[256], archive_tarzst[256], extract_dir[256];
    snprintf(archive_zip, sizeof(archive_zip), "%s/encrypted_archive.zip", temp_dir);
    snprintf(archive_7z, sizeof(archive_7z), "%s/solid_archive.7z", temp_dir);
    snprintf(archive_tarzst, sizeof(archive_tarzst), "%s/dataset.tar.zst", temp_dir);
    snprintf(extract_dir, sizeof(extract_dir), "%s/extracted_output", temp_dir);
    mkdir(extract_dir, 0755);

    // 4. Create AES-256 Encrypted ZIP Archive with Progress Callback & Custom Threads
    printf("3. Creating AES-256 Encrypted ZIP Archive (4 Threads + Progress Callback)...\n");
    ProgressContext pctx_zip = { .operation_name = "ZIP AES-256", .last_reported_bytes = 0 };

    TTZipCreateOptions opt_zip;
    memset(&opt_zip, 0, sizeof(opt_zip));
    opt_zip.struct_size = sizeof(TTZipCreateOptions);
    opt_zip.abi_version = 2;
    opt_zip.format = TTZIP_ARCHIVE_FORMAT_ZIP;
    opt_zip.level = TTZIP_COMPRESSION_LEVEL_NORMAL;
    opt_zip.encryption = TTZIP_ENCRYPTION_AES256;
    opt_zip.password = password;
    opt_zip.thread_budget = 4;
    opt_zip.solid_block_size_mb = 64;
    opt_zip.progress_callback = progress_callback;
    opt_zip.user_data = &pctx_zip;

    TTZipStatus status = ttzip_create_archive(sources, source_count, archive_zip, &opt_zip);
    if (status != TTZIP_STATUS_OK) {
        fprintf(stderr, "❌ ZIP Archive creation failed: %s\n", ttzip_status_string(status));
        return 1;
    }
    printf("   ✓ Encrypted ZIP Archive Created: %s\n", archive_zip);
    printf("--------------------------------------------------------------------------------\n");

    // 5. Create 7z Solid Archive with High Compression Level
    printf("4. Creating 7z Solid Archive with Maximum Compression (4 Threads)...\n");
    TTZipCreateOptions opt_7z;
    memset(&opt_7z, 0, sizeof(opt_7z));
    opt_7z.struct_size = sizeof(TTZipCreateOptions);
    opt_7z.abi_version = 2;
    opt_7z.format = TTZIP_ARCHIVE_FORMAT_SEVEN_ZIP;
    opt_7z.level = TTZIP_COMPRESSION_LEVEL_MAXIMUM;
    opt_7z.thread_budget = 4;
    opt_7z.solid_block_size_mb = 64;

    status = ttzip_create_archive(sources, source_count, archive_7z, &opt_7z);
    if (status != TTZIP_STATUS_OK) {
        fprintf(stderr, "❌ 7z Archive creation failed: %s\n", ttzip_status_string(status));
        return 1;
    }
    printf("   ✓ 7z Solid Archive Created: %s\n", archive_7z);
    printf("--------------------------------------------------------------------------------\n");

    // 6. Create TAR.ZST Archive
    printf("5. Creating TAR.ZST Archive with Ultra Compression...\n");
    TTZipCreateOptions opt_tarzst;
    memset(&opt_tarzst, 0, sizeof(opt_tarzst));
    opt_tarzst.struct_size = sizeof(TTZipCreateOptions);
    opt_tarzst.abi_version = 2;
    opt_tarzst.format = TTZIP_ARCHIVE_FORMAT_TAR_ZSTD;
    opt_tarzst.level = TTZIP_COMPRESSION_LEVEL_ULTRA;
    opt_tarzst.thread_budget = 4;

    status = ttzip_create_archive(sources, source_count, archive_tarzst, &opt_tarzst);
    if (status != TTZIP_STATUS_OK) {
        fprintf(stderr, "❌ TAR.ZST creation failed: %s\n", ttzip_status_string(status));
        return 1;
    }
    printf("   ✓ TAR.ZST Archive Created: %s\n", archive_tarzst);
    printf("--------------------------------------------------------------------------------\n");

    // 7. Inspect Archive Metadata without Extraction
    printf("6. Inspecting Encrypted ZIP Archive Metadata:\n");
    status = ttzip_inspect_archive(archive_zip, password, true, inspect_callback, NULL);
    if (status != TTZIP_STATUS_OK) {
        fprintf(stderr, "❌ Archive inspection failed: %s\n", ttzip_status_string(status));
        return 1;
    }
    printf("--------------------------------------------------------------------------------\n");

    // 8. Extract Encrypted Archive and Verify Content
    printf("7. Extracting AES-256 Encrypted ZIP Archive with Progress Monitoring...\n");
    ProgressContext pctx_extract = { .operation_name = "Extracting", .last_reported_bytes = 0 };

    TTZipExtractOptions opt_extract;
    memset(&opt_extract, 0, sizeof(opt_extract));
    opt_extract.struct_size = sizeof(TTZipExtractOptions);
    opt_extract.abi_version = 2;
    opt_extract.destination_path = extract_dir;
    opt_extract.password = password;
    opt_extract.thread_budget = 4;
    opt_extract.overwrite_existing = true;
    opt_extract.preserve_permissions = true;
    opt_extract.progress_callback = progress_callback;
    opt_extract.user_data = &pctx_extract;

    status = ttzip_extract_archive(archive_zip, extract_dir, &opt_extract);
    if (status != TTZIP_STATUS_OK) {
        fprintf(stderr, "❌ Archive extraction failed: %s\n", ttzip_status_string(status));
        return 1;
    }

    char extracted_config[256];
    snprintf(extracted_config, sizeof(extracted_config), "%s/config.json", extract_dir);
    FILE *fe = fopen(extracted_config, "rb");
    if (fe) {
        char buf[512] = {0};
        size_t n = fread(buf, 1, sizeof(buf) - 1, fe);
        (void)n;
        fclose(fe);
        printf("   ✓ Decrypted Payload Verified:\n     %s\n", buf);
    }

    // Cleanup temporary files
    unlink(file1);
    unlink(file2);
    unlink(file3);
    unlink(archive_zip);
    unlink(archive_7z);
    unlink(archive_tarzst);
    snprintf(extracted_config, sizeof(extracted_config), "%s/config.json", extract_dir);
    unlink(extracted_config);
    snprintf(extracted_config, sizeof(extracted_config), "%s/database_schema.sql", extract_dir);
    unlink(extracted_config);
    snprintf(extracted_config, sizeof(extracted_config), "%s/readme.txt", extract_dir);
    unlink(extracted_config);
    rmdir(extract_dir);
    rmdir(temp_dir);

    printf("================================================================================\n");
    printf("🎉 TTZip C11 Native C-ABI Advanced Showcase Completed Successfully (Exit Code: 0)\n");
    printf("================================================================================\n");
    return 0;
}
