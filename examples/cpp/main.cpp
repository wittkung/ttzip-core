// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

// TTZip: Modern C++20 RAII SDK Standalone Quickstart Example.

#include <iostream>
#include <fstream>
#include <filesystem>
#include <span>
#include <ttzip.hpp>

int main() {
    std::cout << "⚡️ TTZip Modern C++20 SDK Quickstart (v" << ttzip::version() << ")\n";
    std::cout << "Hardware Acceleration: "
              << (ttzip::is_hardware_accelerated() ? "ENABLED" : "DISABLED") << "\n";

    // 1. Zero-copy SIMD CRC-32 via std::span
    std::string payload = "TTZip Modern C++20 RAII & SIMD Archiving Engine";
    std::span<const uint8_t> span(reinterpret_cast<const uint8_t*>(payload.data()), payload.size());
    uint32_t crc = ttzip::crc32(span);
    std::cout << "CRC-32 Checksum: 0x" << std::hex << std::uppercase << crc << std::dec << "\n";

    // 2. Setup temporary files for archive workflow demonstration
    namespace fs = std::filesystem;
    fs::path temp_dir = fs::temp_directory_path() / "ttzip_cpp_quickstart";
    fs::create_directories(temp_dir);

    fs::path sample_file = temp_dir / "sample.txt";
    {
        std::ofstream ofs(sample_file);
        ofs << "TTZip modern C++20 compression sample text content\n";
    }

    fs::path archive_file = temp_dir / "quickstart_demo.zip";
    if (fs::exists(archive_file)) {
        fs::remove(archive_file);
    }

    // 3. RAII Archive Writer fluent builder
    std::cout << "Creating archive: " << archive_file.string() << "\n";
    auto writer_res = ttzip::ArchiveWriter::create(archive_file, ttzip::ArchiveFormat::Zip);
    if (!writer_res) {
        std::cerr << "❌ Failed to initialize ArchiveWriter: " << writer_res.error() << "\n";
        return 1;
    }

    auto writer = std::move(writer_res.value());
    writer.add_file(sample_file)
          .set_level(ttzip::CompressionLevel::Normal);

    auto finish_res = writer.finish();
    if (!finish_res) {
        std::cerr << "❌ Failed to write archive: " << finish_res.error() << "\n";
        return 1;
    }
    std::cout << "  [OK] Archive created successfully.\n";

    // 4. RAII Archive Reader inspection
    std::cout << "Inspecting archive entries...\n";
    auto reader_res = ttzip::ArchiveReader::open(archive_file);
    if (!reader_res) {
        std::cerr << "❌ Failed to open archive: " << reader_res.error() << "\n";
        return 1;
    }

    const auto& reader = reader_res.value();
    for (const auto& entry : reader.entries()) {
        std::cout << "  - Entry: " << entry.path
                  << " (Size: " << entry.uncompressed_size << " bytes, CRC: 0x"
                  << std::hex << entry.crc32 << std::dec << ")\n";
    }

    // Cleanup
    fs::remove_all(temp_dir);
    std::cout << "✅ TTZip Modern C++20 Quickstart finished successfully.\n";
    return 0;
}
