// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: C11 Native C-ABI SDK Standalone Quickstart Example.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <ttzip.h>

static bool inspect_callback(const TTZipEntryMetadata *entry, void *user_data) {
    (void)user_data;
    if (!entry) return false;
    printf("  - Entry: %s (Size: %llu bytes, CRC: 0x%08X)\n",
           entry->path ? entry->path : "unknown",
           (unsigned long long)entry->uncompressed_size,
           entry->crc32);
    return true;
}

int main(void) {
    printf("⚡️ TTZip C11 Native SDK Quickstart (v%s)\n", ttzip_version());
    printf("Hardware Acceleration: %s\n",
           ttzip_is_hardware_accelerated() ? "ENABLED" : "DISABLED");

    // 1. Compute SIMD CRC-32 Checksum
    const char *payload = "TTZip C11 Native C-ABI Ultra-Fast Checksums";
    size_t payload_len = strlen(payload);
    uint32_t crc = ttzip_crc32((const uint8_t *)payload, payload_len);
    printf("CRC-32 Checksum: 0x%08X\n", crc);

    // 2. Prepare temporary file for archiving
    const char *sample_path = "/tmp/ttzip_c_sample.txt";
    FILE *f = fopen(sample_path, "wb");
    if (f) {
        fputs("TTZip C11 Native SDK Sample Content\n", f);
        fclose(f);
    }

    const char *archive_path = "/tmp/ttzip_c_quickstart_demo.zip";
    unlink(archive_path);

    // 3. Create Archive
    const char *sources[] = { sample_path };
    TTZipCreateOptions opts;
    memset(&opts, 0, sizeof(opts));
    opts.struct_size = sizeof(TTZipCreateOptions);
    opts.abi_version = 2;
    opts.format = TTZIP_ARCHIVE_FORMAT_ZIP;
    opts.level = TTZIP_COMPRESSION_LEVEL_NORMAL;

    printf("Creating archive: %s\n", archive_path);
    TTZipStatus status = ttzip_create_archive(sources, 1, archive_path, &opts);
    if (status != TTZIP_STATUS_OK) {
        fprintf(stderr, "❌ Archive creation failed: %s\n", ttzip_status_string(status));
        unlink(sample_path);
        return 1;
    }
    printf("  [OK] Archive created successfully.\n");

    // 4. Inspect Archive
    printf("Inspecting archive entries...\n");
    status = ttzip_inspect_archive(archive_path, NULL, true, inspect_callback, NULL);
    if (status != TTZIP_STATUS_OK) {
        fprintf(stderr, "❌ Archive inspection failed: %s\n", ttzip_status_string(status));
        unlink(sample_path);
        unlink(archive_path);
        return 1;
    }

    // Cleanup
    unlink(sample_path);
    unlink(archive_path);

    printf("✅ TTZip C11 Quickstart finished successfully.\n");
    return 0;
}
