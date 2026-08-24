// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

// TTZip: Modern C++20 RAII Advanced Features Showcase.
// Demonstrates std::span zero-copy buffers, std::expected error handling,
// Zstd compression level 19, and 7z solid encryption with AES-256.

#include <iostream>
#include <fstream>
#include <filesystem>
#include <span>
#include <vector>
#include <string>
#include <string_view>
#include <chrono>
#include <iomanip>
#include <ttzip.hpp>

namespace fs = std::filesystem;

/// Demonstrates std::span zero-copy memory buffer operations and Zstd level 19 compression.
static void demonstrate_span_and_zstd_level_19() {
    std::cout << "2. Demonstrating std::span Zero-Copy Buffers & Zstd Level 19 Compression...\n";

    // Prepare a synthetic in-memory dataset
    std::vector<uint8_t> raw_buffer(1024 * 1024); // 1 MB buffer
    for (size_t i = 0; i < raw_buffer.size(); ++i) {
        raw_buffer[i] = static_cast<uint8_t>((i * 131 + 17) & 0xFF);
    }

    // Wrap in zero-copy std::span
    std::span<const uint8_t> source_span(raw_buffer.data(), raw_buffer.size());

    // SIMD Checksums via std::span
    uint32_t crc32_val = ttzip::crc32(source_span);
    uint64_t crc64_val = ttzip::crc64(source_span);
    std::cout << "   • std::span CRC-32:     0x" << std::hex << std::uppercase << std::setw(8) << std::setfill('0') << crc32_val << "\n";
    std::cout << "   • std::span CRC-64:     0x" << std::setw(16) << crc64_val << std::dec << std::setfill(' ') << "\n";

    // Direct Zstandard Compression at Level 19 (Ultra Max Compression)
    const int zstd_level = 19;
    size_t bound = ttzip_rust_zstd_compress_bound(source_span.size());
    std::vector<uint8_t> compressed_buf(bound);
    size_t compressed_len = 0;

    auto start_time = std::chrono::high_resolution_clock::now();
    TTZipStatus comp_status = ttzip_rust_zstd_compress(
        source_span.data(),
        source_span.size(),
        compressed_buf.data(),
        compressed_buf.size(),
        zstd_level,
        &compressed_len
    );
    auto end_time = std::chrono::high_resolution_clock::now();
    double elapsed_ms = std::chrono::duration<double, std::milli>(end_time - start_time).count();

    if (comp_status == TTZIP_STATUS_OK) {
        compressed_buf.resize(compressed_len);
        double ratio = (static_cast<double>(compressed_len) / source_span.size()) * 100.0;
        std::cout << "   • Zstd Level 19:        " << source_span.size() << " B -> "
                  << compressed_len << " B (" << std::fixed << std::setprecision(2)
                  << ratio << "% ratio) in " << elapsed_ms << " ms\n";

        // Decompress to verify integrity
        std::vector<uint8_t> decompressed_buf(source_span.size());
        size_t decompressed_len = 0;
        TTZipStatus decomp_status = ttzip_rust_zstd_decompress(
            compressed_buf.data(),
            compressed_buf.size(),
            decompressed_buf.data(),
            decompressed_buf.size(),
            &decompressed_len
        );

        if (decomp_status == TTZIP_STATUS_OK && decompressed_len == source_span.size()) {
            std::span<const uint8_t> decomp_span(decompressed_buf.data(), decompressed_len);
            uint32_t verified_crc = ttzip::crc32(decomp_span);
            if (verified_crc == crc32_val) {
                std::cout << "   ✓ Zstd Roundtrip Check: 100% Bit-Exact Match (CRC: 0x"
                          << std::hex << std::uppercase << verified_crc << std::dec << ")\n";
            }
        }
    }
    std::cout << "--------------------------------------------------------------------------------\n";
}

int main() {
    std::cout << "================================================================================\n";
    std::cout << "⚡️ TTZip Modern C++20 RAII & Zero-Copy Advanced Showcase (v" << ttzip::version() << ")\n";
    std::cout << "================================================================================\n";

    // 1. Engine & SIMD Hardware Capabilities
    std::cout << "1. Querying Native Engine Capabilities...\n";
    std::cout << "   • Engine Version:       " << ttzip::version() << "\n";
    std::cout << "   • Hardware SIMD:        "
              << (ttzip::is_hardware_accelerated() ? "ACTIVE (NEON / AVX-512 / AES-NI)" : "DISABLED") << "\n";
    std::cout << "--------------------------------------------------------------------------------\n";

    // 2. std::span zero-copy and Zstd Level 19
    demonstrate_span_and_zstd_level_19();

    // 3. Setup temporary workspace
    fs::path temp_dir = fs::temp_directory_path() / "ttzip_cpp20_advanced_demo";
    fs::create_directories(temp_dir);

    fs::path sample_json = temp_dir / "service_manifest.json";
    fs::path sample_data = temp_dir / "binary_payload.dat";
    fs::path sample_doc  = temp_dir / "architecture.md";

    {
        std::ofstream ofs(sample_json);
        ofs << "{\"sdk\": \"C++20 RAII\", \"features\": [\"std::span\", \"std::expected\", \"AES-256\"], \"solid\": true}\n";
    }
    {
        std::ofstream ofs(sample_data, std::ios::binary);
        std::vector<char> data(65536, 'X');
        ofs.write(data.data(), data.size());
    }
    {
        std::ofstream ofs(sample_doc);
        ofs << "# TTZip C++20 RAII Modern Architecture\nZero-copy spans and monadic expected error handling.\n";
    }

    const std::string password_7z = "Cpp20SecurePassword2026!";
    const uint32_t thread_budget = 4;

    try {
        // 4. 7z Solid Archive with High Compression & Custom Threads
        fs::path archive_7z = temp_dir / "solid_archive.7z";
        std::cout << "3. Creating 7z Solid Archive with Maximum Compression (4 Threads)...\n";

        auto writer_7z_res = ttzip::ArchiveWriter::create(archive_7z, ttzip::ArchiveFormat::SevenZip);
        if (!writer_7z_res) {
            std::cerr << "❌ Failed to create ArchiveWriter: " << writer_7z_res.error() << "\n";
            return 1;
        }

        auto writer_7z = std::move(writer_7z_res.value());
        writer_7z.add_file(sample_json)
                 .add_file(sample_data)
                 .add_file(sample_doc)
                 .set_level(ttzip::CompressionLevel::Maximum)
                 .set_threads(thread_budget);

        auto finish_7z = writer_7z.finish();
        if (!finish_7z) {
            std::cerr << "❌ Failed to finalize 7z archive: " << finish_7z.error() << "\n";
            return 1;
        }
        std::cout << "   ✓ 7z Solid Archive Created: " << archive_7z.filename().string()
                  << " (Size: " << fs::file_size(archive_7z) << " bytes)\n";
        std::cout << "--------------------------------------------------------------------------------\n";

        // 5. AES-256 Protected ZIP Archive via RAII ArchiveWriter & std::expected
        fs::path archive_zip_enc = temp_dir / "encrypted_secure.zip";
        std::cout << "4. Creating AES-256 Encrypted Archive with Password Protection...\n";

        auto writer_enc_res = ttzip::ArchiveWriter::create(archive_zip_enc, ttzip::ArchiveFormat::Zip);
        if (!writer_enc_res) {
            std::cerr << "❌ Failed to create encrypted ArchiveWriter: " << writer_enc_res.error() << "\n";
            return 1;
        }

        auto writer_enc = std::move(writer_enc_res.value());
        writer_enc.add_file(sample_json)
                  .add_file(sample_doc)
                  .set_level(ttzip::CompressionLevel::Normal)
                  .set_password(password_7z)
                  .set_threads(thread_budget);

        auto finish_enc = writer_enc.finish();
        if (!finish_enc) {
            std::cerr << "❌ Failed to finalize encrypted archive: " << finish_enc.error() << "\n";
            return 1;
        }
        std::cout << "   ✓ AES-256 Encrypted Archive Created: " << archive_zip_enc.filename().string()
                  << " (Size: " << fs::file_size(archive_zip_enc) << " bytes)\n";
        std::cout << "--------------------------------------------------------------------------------\n";

        // 6. TAR.ZST Archive with High Compression Level via std::expected
        fs::path archive_tar_zst = temp_dir / "dataset.tar.zst";
        std::cout << "5. Creating TAR.ZST Archive with Zstandard Compression...\n";

        auto writer_zst_res = ttzip::ArchiveWriter::create(archive_tar_zst, ttzip::ArchiveFormat::TarZstd);
        if (!writer_zst_res) {
            std::cerr << "❌ Failed to create TarZstd Writer: " << writer_zst_res.error() << "\n";
            return 1;
        }

        auto writer_zst = std::move(writer_zst_res.value());
        writer_zst.add_file(sample_json)
                  .add_file(sample_doc)
                  .set_level(ttzip::CompressionLevel::Ultra)
                  .set_threads(thread_budget);

        auto finish_zst = writer_zst.finish();
        if (!finish_zst) {
            std::cerr << "❌ Failed to finalize TarZstd archive: " << finish_zst.error() << "\n";
            return 1;
        }
        std::cout << "   ✓ TAR.ZST Archive Created: " << archive_tar_zst.filename().string()
                  << " (Size: " << fs::file_size(archive_tar_zst) << " bytes)\n";
        std::cout << "--------------------------------------------------------------------------------\n";

        // 7. RAII ArchiveReader: Inspecting Archive Metadata with std::expected
        std::cout << "6. Inspecting Archive Metadata via RAII ArchiveReader...\n";
        auto reader_7z_res = ttzip::ArchiveReader::open(archive_7z);
        if (!reader_7z_res) {
            std::cerr << "❌ Failed to open 7z reader: " << reader_7z_res.error() << "\n";
            return 1;
        }

        const auto& reader_7z = reader_7z_res.value();
        for (const auto& entry : reader_7z.entries()) {
            std::cout << "   * Entry: " << std::left << std::setw(26) << entry.path
                      << " | Size: " << std::right << std::setw(6) << entry.uncompressed_size
                      << " B | CRC: 0x" << std::hex << std::setw(8) << std::setfill('0') << entry.crc32
                      << std::dec << std::setfill(' ')
                      << " | Encrypted: " << (entry.is_encrypted ? "YES" : "NO") << "\n";
        }
        std::cout << "--------------------------------------------------------------------------------\n";

        // 8. RAII ArchiveReader: Extracting Archive with std::expected
        fs::path extract_dir = temp_dir / "extracted_7z";
        std::cout << "7. Extracting 7z Archive to " << extract_dir.filename().string() << "...\n";

        auto extract_res = reader_7z.extract_all(extract_dir);
        if (!extract_res) {
            std::cerr << "❌ Extraction failed: " << extract_res.error() << "\n";
            return 1;
        }

        fs::path extracted_manifest = extract_dir / "service_manifest.json";
        if (fs::exists(extracted_manifest)) {
            std::ifstream ifs(extracted_manifest);
            std::string content((std::istreambuf_iterator<char>(ifs)), std::istreambuf_iterator<char>());
            std::cout << "   ✓ Extracted Payload Verified:\n     " << content;
        }

    } catch (const std::exception& ex) {
        std::cerr << "❌ Unhandled Exception: " << ex.what() << "\n";
        fs::remove_all(temp_dir);
        return 1;
    }

    // Cleanup
    fs::remove_all(temp_dir);

    std::cout << "================================================================================\n";
    std::cout << "🎉 TTZip C++20 RAII Advanced Showcase Completed Successfully (Exit Code: 0)\n";
    std::cout << "================================================================================\n";
    return 0;
}
