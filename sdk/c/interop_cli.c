// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

// C11 Headless Interop CLI Runner.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include "../../Sources/CTTZipBridge/include/ttzip.h"

static TTZipArchiveFormat parse_format(const char *fmt_str) {
    if (!fmt_str) return TTZIP_ARCHIVE_FORMAT_ZIP;
    if (strcasecmp(fmt_str, "zip") == 0) return TTZIP_ARCHIVE_FORMAT_ZIP;
    if (strcasecmp(fmt_str, "7z") == 0 || strcasecmp(fmt_str, "7zip") == 0 || strcasecmp(fmt_str, "sevenzip") == 0)
        return TTZIP_ARCHIVE_FORMAT_SEVEN_ZIP;
    if (strcasecmp(fmt_str, "tar") == 0) return TTZIP_ARCHIVE_FORMAT_TAR;
    if (strcasecmp(fmt_str, "tar.gz") == 0 || strcasecmp(fmt_str, "targz") == 0 || strcasecmp(fmt_str, "tgz") == 0 || strcasecmp(fmt_str, "gz") == 0)
        return TTZIP_ARCHIVE_FORMAT_TAR_GZ;
    if (strcasecmp(fmt_str, "tar.bz2") == 0 || strcasecmp(fmt_str, "tarbz2") == 0 || strcasecmp(fmt_str, "tbz2") == 0 || strcasecmp(fmt_str, "bz2") == 0)
        return TTZIP_ARCHIVE_FORMAT_TAR_BZ2;
    if (strcasecmp(fmt_str, "tar.xz") == 0 || strcasecmp(fmt_str, "tarxz") == 0 || strcasecmp(fmt_str, "txz") == 0 || strcasecmp(fmt_str, "xz") == 0)
        return TTZIP_ARCHIVE_FORMAT_TAR_XZ;
    if (strcasecmp(fmt_str, "tar.zst") == 0 || strcasecmp(fmt_str, "tarzst") == 0 || strcasecmp(fmt_str, "tar.zstd") == 0 || strcasecmp(fmt_str, "zst") == 0)
        return TTZIP_ARCHIVE_FORMAT_TAR_ZSTD;
    return TTZIP_ARCHIVE_FORMAT_ZIP;
}

static void print_usage(const char *prog) {
    fprintf(stderr, "Usage:\n");
    fprintf(stderr, "  %s --create <format> <src> <dst> [--password <pwd>]\n", prog);
    fprintf(stderr, "  %s --extract <src> <dst> [--password <pwd>]\n", prog);
    fprintf(stderr, "  %s --version\n", prog);
}

int main(int argc, char **argv) {
    if (argc < 2) {
        print_usage(argv[0]);
        return 2;
    }

    if (strcmp(argv[1], "--version") == 0) {
        printf("%s\n", ttzip_version());
        return 0;
    }

    const char *mode = NULL;
    const char *format_str = NULL;
    const char *src = NULL;
    const char *dst = NULL;
    const char *password = NULL;

    int i = 1;
    while (i < argc) {
        if (strcmp(argv[i], "--create") == 0) {
            mode = "create";
            if (i + 3 >= argc) {
                fprintf(stderr, "Error: --create requires <format> <src> <dst>\n");
                return 2;
            }
            format_str = argv[i + 1];
            src = argv[i + 2];
            dst = argv[i + 3];
            i += 4;
        } else if (strcmp(argv[i], "--extract") == 0) {
            mode = "extract";
            if (i + 2 >= argc) {
                fprintf(stderr, "Error: --extract requires <src> <dst>\n");
                return 2;
            }
            src = argv[i + 1];
            dst = argv[i + 2];
            i += 3;
        } else if (strcmp(argv[i], "--password") == 0) {
            if (i + 1 >= argc) {
                fprintf(stderr, "Error: --password requires an argument\n");
                return 2;
            }
            password = argv[i + 1];
            i += 2;
        } else {
            fprintf(stderr, "Unknown argument: %s\n", argv[i]);
            print_usage(argv[0]);
            return 2;
        }
    }

    if (!mode) {
        print_usage(argv[0]);
        return 2;
    }

    if (strcmp(mode, "create") == 0) {
        TTZipCreateOptions opts;
        memset(&opts, 0, sizeof(opts));
        opts.struct_size = sizeof(TTZipCreateOptions);
        opts.abi_version = 2;
        opts.format = parse_format(format_str);
        opts.level = TTZIP_COMPRESSION_LEVEL_NORMAL;
        if (password && strlen(password) > 0) {
            opts.encryption = TTZIP_ENCRYPTION_AES256;
            opts.password = password;
        } else {
            opts.encryption = TTZIP_ENCRYPTION_NONE;
            opts.password = NULL;
        }
        opts.solid_block_size_mb = 64;

        const char *sources[] = { src };
        TTZipStatus st = ttzip_create_archive(sources, 1, dst, &opts);
        if (st != TTZIP_STATUS_OK) {
            fprintf(stderr, "Archive creation failed: %d (%s)\n", st, ttzip_status_string(st));
            return 1;
        }
        return 0;
    } else if (strcmp(mode, "extract") == 0) {
        TTZipExtractOptions opts;
        memset(&opts, 0, sizeof(opts));
        opts.struct_size = sizeof(TTZipExtractOptions);
        opts.abi_version = 2;
        opts.destination_path = dst;
        opts.password = (password && strlen(password) > 0) ? password : NULL;
        opts.overwrite_existing = true;
        opts.preserve_permissions = true;
        opts.dry_run = false;

        TTZipStatus st = ttzip_extract_archive(src, dst, &opts);
        if (st != TTZIP_STATUS_OK) {
            fprintf(stderr, "Archive extraction failed: %d (%s)\n", st, ttzip_status_string(st));
            return 1;
        }
        return 0;
    }

    return 2;
}
