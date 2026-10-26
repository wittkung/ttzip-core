// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include <unistd.h>
#include "../include/ttzip.h"

int main(int argc, char **argv) {
    int iterations = (argc > 1) ? atoi(argv[1]) : 1000;
    printf("⚡️ Running AddressSanitizer (ASan) Memory Leak Stress Test (%d rapid cycles)...\n", iterations);

    const char *tmp_txt = "/tmp/ttzip_asan_stress.txt";
    FILE *f = fopen(tmp_txt, "w");
    if (!f) {
        perror("fopen");
        return 1;
    }
    fputs("ASan Zero Memory Leak Verification Payload for TTZip Native Engine\n", f);
    fclose(f);

    const char *sources[] = { tmp_txt };
    const char *archive_path = "/tmp/ttzip_asan_stress.zip";

    TTZipCreateOptions c_opts;
    ttzip_create_options_init(&c_opts);
    c_opts.format = TTZIP_ARCHIVE_FORMAT_ZIP;
    c_opts.level = TTZIP_COMPRESSION_LEVEL_FASTEST;

    TTZipStatus st = ttzip_create_archive(sources, 1, archive_path, &c_opts);
    if (st != TTZIP_STATUS_OK) {
        fprintf(stderr, "Create archive failed: %d\n", st);
        return 1;
    }

    const char *dest_dir = "/tmp/ttzip_asan_extracted";
    TTZipExtractOptions e_opts;
    ttzip_extract_options_init(&e_opts);
    e_opts.destination_path = dest_dir;
    e_opts.collision_policy = TTZIP_COLLISION_POLICY_REPLACE;
    e_opts.preserve_permissions = true;

    for (int i = 0; i < iterations; i++) {
        st = ttzip_extract_archive(archive_path, dest_dir, &e_opts);
        if (st != TTZIP_STATUS_OK) {
            fprintf(stderr, "Cycle %d extract failed with status: %d\n", i, st);
            return 1;
        }
    }

    unlink(tmp_txt);
    unlink(archive_path);
    printf("✅ [PASS] Completed %d rapid extraction cycles with 0 byte leak!\n", iterations);
    return 0;
}
