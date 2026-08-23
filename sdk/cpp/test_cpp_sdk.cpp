// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.

#include <iostream>
#include <fstream>
#include <cassert>
#include <unistd.h>
#include "../../Sources/CTTZipBridge/include/ttzip.hpp"

int main() {
    std::cout << "⚡️ Running TTZip Modern C++20 SDK Test Suite...\n";

    // 1. Version
    std::string ver = ttzip::version();
    assert(!ver.empty());
    std::cout << "  [PASS] C++ SDK version: " << ver << "\n";

    // 2. CRC32
    std::string msg = "TTZip C++20 Modern Test Message";
    std::span<const uint8_t> span(reinterpret_cast<const uint8_t*>(msg.data()), msg.size());
    uint32_t c32 = ttzip::crc32(span);
    assert(c32 != 0);
    std::cout << "  [PASS] C++ SDK CRC-32: 0x" << std::hex << c32 << std::dec << "\n";

    // 3. Compression & Inspection
    std::string tmp_file = "/tmp/ttzip_cpp_sample.txt";
    std::ofstream out(tmp_file);
    out << "Modern C++20 Archiving with TTZip\n";
    out.close();

    std::string archive_path = "/tmp/ttzip_cpp_sample.zip";
    ttzip::compress_files({tmp_file}, archive_path, 6);
    std::cout << "  [PASS] C++ SDK compress_files() OK\n";

    auto entries = ttzip::inspect_archive(archive_path);
    assert(entries.size() == 1);
    std::cout << "  [PASS] C++ SDK inspect_archive() found " << entries.size() << " entry: " << entries[0].path << "\n";

    // 4. Extraction
    ttzip::extract_archive(archive_path, "/tmp/ttzip_cpp_extracted");
    std::cout << "  [PASS] C++ SDK extract_archive() OK\n";

    unlink(tmp_file.c_str());
    unlink(archive_path.c_str());

    std::cout << "✅ All C++ SDK tests passed successfully!\n";
    return 0;
}
