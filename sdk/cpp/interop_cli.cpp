// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

// Modern C++20 Headless Interop CLI Runner.

#include <iostream>
#include <string>
#include <vector>
#include <algorithm>
#include <cctype>
#include "../../Sources/CTTZipBridge/include/ttzip.hpp"

static std::string to_lower(std::string s) {
    std::transform(s.begin(), s.end(), s.begin(), [](unsigned char c) {
        return static_cast<char>(std::tolower(c));
    });
    return s;
}

static ttzip::ArchiveFormat parse_format(const std::string& fmt_raw) {
    std::string fmt = to_lower(fmt_raw);
    if (fmt == "zip") return ttzip::ArchiveFormat::Zip;
    if (fmt == "7z" || fmt == "7zip" || fmt == "sevenzip") return ttzip::ArchiveFormat::SevenZip;
    if (fmt == "tar") return ttzip::ArchiveFormat::Tar;
    if (fmt == "tar.gz" || fmt == "targz" || fmt == "tgz" || fmt == "gz") return ttzip::ArchiveFormat::TarGz;
    if (fmt == "tar.bz2" || fmt == "tarbz2" || fmt == "tbz2" || fmt == "bz2") return ttzip::ArchiveFormat::TarBz2;
    if (fmt == "tar.xz" || fmt == "tarxz" || fmt == "txz" || fmt == "xz") return ttzip::ArchiveFormat::TarXz;
    if (fmt == "tar.zst" || fmt == "tarzst" || fmt == "tar.zstd" || fmt == "zst") return ttzip::ArchiveFormat::TarZstd;
    return ttzip::ArchiveFormat::Zip;
}

static void print_usage(const char* prog) {
    std::cerr << "Usage:\n"
              << "  " << prog << " --create <format> <src> <dst> [--password <pwd>]\n"
              << "  " << prog << " --extract <src> <dst> [--password <pwd>]\n"
              << "  " << prog << " --version\n";
}

int main(int argc, char** argv) {
    if (argc < 2) {
        print_usage(argv[0]);
        return 2;
    }

    std::string arg1 = argv[1];
    if (arg1 == "--version") {
        std::cout << ttzip::version() << "\n";
        return 0;
    }

    std::string mode;
    std::string format_str;
    std::string src;
    std::string dst;
    std::string password;

    int i = 1;
    while (i < argc) {
        std::string arg = argv[i];
        if (arg == "--create") {
            mode = "create";
            if (i + 3 >= argc) {
                std::cerr << "Error: --create requires <format> <src> <dst>\n";
                return 2;
            }
            format_str = argv[i + 1];
            src = argv[i + 2];
            dst = argv[i + 3];
            i += 4;
        } else if (arg == "--extract") {
            mode = "extract";
            if (i + 2 >= argc) {
                std::cerr << "Error: --extract requires <src> <dst>\n";
                return 2;
            }
            src = argv[i + 1];
            dst = argv[i + 2];
            i += 3;
        } else if (arg == "--password") {
            if (i + 1 >= argc) {
                std::cerr << "Error: --password requires an argument\n";
                return 2;
            }
            password = argv[i + 1];
            i += 2;
        } else {
            std::cerr << "Unknown argument: " << arg << "\n";
            print_usage(argv[0]);
            return 2;
        }
    }

    if (mode.empty()) {
        print_usage(argv[0]);
        return 2;
    }

    try {
        if (mode == "create") {
            ttzip::ArchiveFormat fmt = parse_format(format_str);
            ttzip::compress_files({src}, dst, 6, password, fmt);
            return 0;
        } else if (mode == "extract") {
            ttzip::extract_archive(src, dst, password);
            return 0;
        }
    } catch (const std::exception& ex) {
        std::cerr << "Error: " << ex.what() << "\n";
        return 1;
    }

    return 2;
}
