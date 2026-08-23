// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include <unistd.h>
#include "../../Sources/CTTZipBridge/include/ttzip.h"

static bool count_entries(const TTZipEntryMetadata *meta, void *user_data) {
    size_t *count = (size_t *)user_data;
    (*count)++;
    return true;
}

int main(void) {
    printf("⚡️ Running TTZip Native C11 SDK Test Suite...\n");

    // 1. Version check
    const char *ver = ttzip_version();
    assert(ver != NULL);
    assert(strlen(ver) > 0);
    printf("  [PASS] C SDK ttzip_version(): %s\n", ver);

    // 2. Hardware acceleration check
    bool hw = ttzip_is_hardware_accelerated();
    printf("  [PASS] C SDK hardware acceleration: %s\n", hw ? "ENABLED" : "DISABLED");

    // 3. CRC-32 and CRC-64
    const char *msg = "TTZip Ultra-Fast Native C SDK Test";
    uint32_t c32 = ttzip_crc32((const uint8_t *)msg, strlen(msg));
    assert(c32 != 0);
    printf("  [PASS] C SDK ttzip_crc32(): 0x%08X\n", c32);

    uint64_t c64 = ttzip_crc64((const uint8_t *)msg, strlen(msg));
    assert(c64 != 0);
    printf("  [PASS] C SDK ttzip_crc64(): 0x%016llX\n", (unsigned long long)c64);

    // 4. Archive Creation and Inspection
    const char *tmp_file = "/tmp/ttzip_c_sample.txt";
    FILE *f = fopen(tmp_file, "w");
    assert(f != NULL);
    fputs("Payload for C SDK test\n", f);
    fclose(f);

    const char *sources[] = { tmp_file };
    const char *archive_path = "/tmp/ttzip_c_sample.zip";

    TTZipCreateOptions create_opts;
    memset(&create_opts, 0, sizeof(create_opts));
    create_opts.format = TTZIP_ARCHIVE_FORMAT_ZIP;
    create_opts.level = 6;

    TTZipStatus st = ttzip_create_archive(sources, 1, archive_path, &create_opts);
    assert(st == TTZIP_STATUS_OK);
    printf("  [PASS] C SDK ttzip_create_archive() OK\n");

    size_t entry_count = 0;
    st = ttzip_inspect_archive(archive_path, NULL, true, count_entries, &entry_count);
    assert(st == TTZIP_STATUS_OK);
    assert(entry_count == 1);
    printf("  [PASS] C SDK ttzip_inspect_archive() counted %zu entry\n", entry_count);

    // 5. Extraction
    TTZipExtractOptions extract_opts;
    memset(&extract_opts, 0, sizeof(extract_opts));
    extract_opts.destination_path = "/tmp/ttzip_c_extracted";
    extract_opts.overwrite_existing = true;
    st = ttzip_extract_archive(archive_path, "/tmp/ttzip_c_extracted", &extract_opts);
    if (st != TTZIP_STATUS_OK) {
        fprintf(stderr, "❌ Extraction failed: %d (%s)\n", st, ttzip_status_string(st));
    }
    assert(st == TTZIP_STATUS_OK);
    printf("  [PASS] C SDK ttzip_extract_archive() OK\n");

    unlink(tmp_file);
    unlink(archive_path);

    printf("✅ All C SDK tests passed successfully!\n");
    return 0;
}
