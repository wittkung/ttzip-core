// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.

#ifndef CTTZipBridge_h
#define CTTZipBridge_h

#include <stddef.h>
#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// Platform File Kind (for format sniffing)
typedef enum {
    TTZIP_KIND_UNKNOWN = 0,
    TTZIP_KIND_ARCHIVE = 1,
    TTZIP_KIND_IMAGE   = 2,
    TTZIP_KIND_AUDIO   = 3,
    TTZIP_KIND_VIDEO   = 4,
    TTZIP_KIND_TEXT    = 5,
    TTZIP_KIND_BINARY  = 6
} ttzip_file_kind_t;

#ifdef __cplusplus
}
#endif

#endif /* CTTZipBridge_h */
