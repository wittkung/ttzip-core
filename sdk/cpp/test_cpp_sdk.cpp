// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.
// Comprehensive C++20 std::span / std::expected / RAII Archive Test Suite.

#include <iostream>
#include <fstream>
#include <sstream>
#include <cassert>
#include <vector>
#include <span>
#include <string>
#include <filesystem>
#include "../../Sources/CTTZipBridge/include/ttzip.hpp"

namespace fs = std::filesystem;

class ScopedTempDirectory {
public:
    ScopedTempDirectory(const std::string& prefix) {
        auto tmp = fs::temp_directory_path();
        path_ = tmp / (prefix + "_" + std::to_string(std::chrono::system_clock::now().time_since_epoch().count()));
        fs::create_directories(path_);
    }

    ~ScopedTempDirectory() {
        std::error_code ec;
        fs::remove_all(path_, ec);
    }

    const fs::path& path() const noexcept { return path_; }

private:
    fs::path path_;
};

static void write_file(const fs::path& p, const std::string& content) {
    fs::create_directories(p.parent_path());
    std::ofstream out(p, std::ios::binary);
    out << content;
    out.close();
}

static std::string read_file(const fs::path& p) {
    std::ifstream in(p, std::ios::binary);
    std::ostringstream ss;
    ss << in.rdbuf();
    return ss.str();
}

// 1. Test Version and Hardware Acceleration
static void test_version_and_hw_accel() {
    std::string ver = ttzip::version();
    assert(!ver.empty());
    std::cout << "  [PASS] C++ SDK version: " << ver << "\n";

    bool hw = ttzip::is_hardware_accelerated();
    std::cout << "  [PASS] C++ SDK hardware acceleration query: " << (hw ? "ENABLED" : "DISABLED") << "\n";
}

// 2. Test std::span zero-copy buffer slicing and SIMD checksums
static void test_span_zero_copy_checksums() {
    std::string message = "TTZip Modern C++20 std::span Zero-Copy Slicing and SIMD Checksums Benchmark 2026";
    std::span<const uint8_t> full_span(reinterpret_cast<const uint8_t*>(message.data()), message.size());

    // CRC-32 on whole span
    uint32_t full_crc = ttzip::crc32(full_span);
    assert(full_crc != 0);

    // Zero-copy subspan slicing and incremental CRC-32
    size_t mid = full_span.size() / 2;
    auto first_half = full_span.subspan(0, mid);
    auto second_half = full_span.subspan(mid);

    uint32_t seed = ttzip::crc32(first_half, 0);
    uint32_t chained_crc = ttzip::crc32(second_half, seed);
    assert(chained_crc == full_crc);

    // CRC-64 on whole span and subspan
    uint64_t full_crc64 = ttzip::crc64(full_span);
    assert(full_crc64 != 0);

    uint64_t chained_crc64 = ttzip::crc64(second_half, ttzip::crc64(first_half, 0));
    assert(chained_crc64 != 0);

    std::cout << "  [PASS] C++20 std::span zero-copy CRC-32 (0x" << std::hex << full_crc 
              << ") & CRC-64 (0x" << full_crc64 << std::dec << ")\n";
}

// 3. Test High-Level Functional APIs: compress_files, inspect_archive, extract_archive
static void test_high_level_archiving(const fs::path& temp_dir) {
    fs::path f1 = temp_dir / "sample1.txt";
    fs::path f2 = temp_dir / "subdir" / "sample2.log";
    std::string text1 = "Payload in sample 1 - C++20 Core";
    std::string text2 = "Payload in sample 2 - Subdirectory log entry";

    write_file(f1, text1);
    write_file(f2, text2);

    fs::path archive_path = temp_dir / "high_level.zip";
    fs::path extract_dir = temp_dir / "high_level_extracted";

    // 1. Compress
    ttzip::compress_files(
        {f1.string(), f2.parent_path().string()},
        archive_path.string(),
        6,
        "",
        ttzip::ArchiveFormat::Zip,
        2
    );
    assert(fs::exists(archive_path));
    assert(fs::file_size(archive_path) > 0);

    // 2. Inspect
    auto entries = ttzip::inspect_archive(archive_path.string());
    assert(!entries.empty());
    bool found_sample1 = false;
    for (const auto& e : entries) {
        if (e.path.find("sample1.txt") != std::string::npos) {
            found_sample1 = true;
            assert(!e.is_directory);
        }
    }
    assert(found_sample1);

    // 3. Extract
    ttzip::extract_archive(archive_path.string(), extract_dir.string(), "", 2);
    assert(fs::exists(extract_dir / "sample1.txt"));
    assert(read_file(extract_dir / "sample1.txt") == text1);

    std::cout << "  [PASS] C++ SDK high-level compress_files / inspect / extract OK\n";
}

// 4. Test RAII ArchiveWriter with std::expected
static void test_raii_archive_writer(const fs::path& temp_dir) {
    fs::path doc_file = temp_dir / "writer_input.txt";
    std::string doc_content = "RAII ArchiveWriter Fluent API Test Payload";
    write_file(doc_file, doc_content);

    fs::path archive_path = temp_dir / "raii_output.zip";

    // Test error case on empty writer
    auto empty_writer = ttzip::ArchiveWriter::create(archive_path, ttzip::ArchiveFormat::Zip);
    assert(empty_writer.has_value());
    auto empty_res = empty_writer->finish();
    assert(!empty_res.has_value()); // Should fail because no source files were added

    // Fluent creation
    auto writer = ttzip::ArchiveWriter::create(archive_path, ttzip::ArchiveFormat::Zip);
    assert(writer.has_value());
    writer->add_file(doc_file)
           .set_level(ttzip::CompressionLevel::Fastest)
           .set_threads(1);

    auto finish_res = writer->finish();
    assert(finish_res.has_value());
    assert(fs::exists(archive_path));

    std::cout << "  [PASS] C++20 RAII ArchiveWriter std::expected fluent builder OK\n";
}

// 5. Test RAII ArchiveReader with std::expected
static void test_raii_archive_reader(const fs::path& temp_dir) {
    fs::path non_existent = temp_dir / "does_not_exist.zip";
    auto fail_reader = ttzip::ArchiveReader::open(non_existent);
    assert(!fail_reader.has_value()); // Must fail gracefully with unexpected error message

    // Create a valid archive to read
    fs::path sample = temp_dir / "reader_src.txt";
    std::string content = "RAII ArchiveReader std::expected verification payload";
    write_file(sample, content);

    fs::path valid_archive = temp_dir / "valid_archive.zip";
    ttzip::compress_files({sample.string()}, valid_archive.string());

    auto reader = ttzip::ArchiveReader::open(valid_archive);
    assert(reader.has_value());
    const auto& entries = reader->entries();
    assert(!entries.empty());

    fs::path dest = temp_dir / "raii_reader_extracted";
    auto ext_res = reader->extract_all(dest);
    assert(ext_res.has_value());
    assert(fs::exists(dest / "reader_src.txt"));
    assert(read_file(dest / "reader_src.txt") == content);

    std::cout << "  [PASS] C++20 RAII ArchiveReader std::expected open & extract_all OK\n";
}

// 6. Test Multiple Formats Matrix & Compression Levels
static void test_multi_format_matrix(const fs::path& temp_dir) {
    fs::path src_file = temp_dir / "matrix_doc.txt";
    std::string payload = "Multi-format validation (ZIP, 7Z, TAR, TAR.GZ, TAR.BZ2, TAR.XZ, TAR.ZSTD) C++20 test\n";
    write_file(src_file, payload);

    struct FormatSpec {
        ttzip::ArchiveFormat format;
        ttzip::CompressionLevel level;
        std::string filename;
    };

    std::vector<FormatSpec> formats = {
        { ttzip::ArchiveFormat::Zip, ttzip::CompressionLevel::Fastest, "matrix.zip" },
        { ttzip::ArchiveFormat::SevenZip, ttzip::CompressionLevel::Normal, "matrix.7z" },
        { ttzip::ArchiveFormat::Tar, ttzip::CompressionLevel::Store, "matrix.tar" },
        { ttzip::ArchiveFormat::TarGz, ttzip::CompressionLevel::Fast, "matrix.tar.gz" },
        { ttzip::ArchiveFormat::TarBz2, ttzip::CompressionLevel::Normal, "matrix.tar.bz2" },
        { ttzip::ArchiveFormat::TarXz, ttzip::CompressionLevel::Maximum, "matrix.tar.xz" },
        { ttzip::ArchiveFormat::TarZstd, ttzip::CompressionLevel::Ultra, "matrix.tar.zst" }
    };

    for (const auto& spec : formats) {
        fs::path arc = temp_dir / spec.filename;
        fs::path out_dir = temp_dir / ("out_" + spec.filename);

        ttzip::compress_files({src_file.string()}, arc.string(), static_cast<int>(spec.level), "", spec.format);
        assert(fs::exists(arc));

        auto entries = ttzip::inspect_archive(arc.string());
        assert(!entries.empty());

        ttzip::extract_archive(arc.string(), out_dir.string());
        assert(fs::exists(out_dir / "matrix_doc.txt"));
        assert(read_file(out_dir / "matrix_doc.txt") == payload);
    }

    std::cout << "  [PASS] C++ SDK multi-format matrix & compression level configurations OK\n";
}

// 7. Test In-Memory Buffer Codecs (DEFLATE, ZSTD, LZ4, Snappy, LZFSE)
static void test_memory_buffer_codecs() {
    std::string text = "Modern C++20 Zero-Copy std::span In-Memory Buffer Codec Compression Verification 2026!\n";
    std::span<const uint8_t> src_span(reinterpret_cast<const uint8_t*>(text.data()), text.size());

    std::vector<uint8_t> comp_buf(4096);
    std::vector<uint8_t> decomp_buf(4096);
    size_t comp_len = 0;
    size_t decomp_len = 0;

    // DEFLATE
    TTZipStatus st = ttzip_rust_deflate_compress(src_span.data(), src_span.size(), comp_buf.data(), comp_buf.size(), 6, &comp_len);
    assert(st == TTZIP_STATUS_OK && comp_len > 0);
    st = ttzip_rust_deflate_decompress(comp_buf.data(), comp_len, decomp_buf.data(), decomp_buf.size(), &decomp_len);
    assert(st == TTZIP_STATUS_OK && decomp_len == src_span.size());
    assert(std::memcmp(decomp_buf.data(), src_span.data(), src_span.size()) == 0);

    // ZSTD
    st = ttzip_rust_zstd_compress(src_span.data(), src_span.size(), comp_buf.data(), comp_buf.size(), 3, &comp_len);
    assert(st == TTZIP_STATUS_OK && comp_len > 0);
    st = ttzip_rust_zstd_decompress(comp_buf.data(), comp_len, decomp_buf.data(), decomp_buf.size(), &decomp_len);
    assert(st == TTZIP_STATUS_OK && decomp_len == src_span.size());
    assert(std::memcmp(decomp_buf.data(), src_span.data(), src_span.size()) == 0);

    // LZ4
    st = ttzip_rust_lz4_compress(src_span.data(), src_span.size(), comp_buf.data(), comp_buf.size(), &comp_len);
    assert(st == TTZIP_STATUS_OK && comp_len > 0);
    st = ttzip_rust_lz4_decompress(comp_buf.data(), comp_len, decomp_buf.data(), decomp_buf.size(), &decomp_len);
    assert(st == TTZIP_STATUS_OK && decomp_len == src_span.size());
    assert(std::memcmp(decomp_buf.data(), src_span.data(), src_span.size()) == 0);

    // Snappy
    st = ttzip_rust_snappy_compress(src_span.data(), src_span.size(), comp_buf.data(), comp_buf.size(), &comp_len);
    assert(st == TTZIP_STATUS_OK && comp_len > 0);
    st = ttzip_rust_snappy_decompress(comp_buf.data(), comp_len, decomp_buf.data(), decomp_buf.size(), &decomp_len);
    assert(st == TTZIP_STATUS_OK && decomp_len == src_span.size());
    assert(std::memcmp(decomp_buf.data(), src_span.data(), src_span.size()) == 0);

    // LZFSE
    st = ttzip_rust_lzfse_compress(src_span.data(), src_span.size(), comp_buf.data(), comp_buf.size(), &comp_len);
    assert(st == TTZIP_STATUS_OK && comp_len > 0);
    st = ttzip_rust_lzfse_decompress(comp_buf.data(), comp_len, decomp_buf.data(), decomp_buf.size(), &decomp_len);
    assert(st == TTZIP_STATUS_OK && decomp_len == src_span.size());
    assert(std::memcmp(decomp_buf.data(), src_span.data(), src_span.size()) == 0);

    std::cout << "  [PASS] C++20 in-memory buffer codecs (DEFLATE, ZSTD, LZ4, Snappy, LZFSE) OK\n";
}

// 8. Test AES-256 Password Protection
static void test_password_protection(const fs::path& temp_dir) {
    fs::path sec_file = temp_dir / "secret.txt";
    std::string secret = "C++20 AES-256 Encrypted Archive Payload 2026";
    write_file(sec_file, secret);

    fs::path enc_zip = temp_dir / "encrypted_cpp.zip";
    std::string correct_pwd = "CPPSecretPassword2026!";
    std::string wrong_pwd = "WrongSecretPassword!";

    // Create encrypted archive
    ttzip::compress_files({sec_file.string()}, enc_zip.string(), 6, correct_pwd, ttzip::ArchiveFormat::Zip);
    assert(fs::exists(enc_zip));

    // Extract with correct password
    fs::path valid_dest = temp_dir / "cpp_decrypted_valid";
    ttzip::extract_archive(enc_zip.string(), valid_dest.string(), correct_pwd);
    assert(fs::exists(valid_dest / "secret.txt"));
    assert(read_file(valid_dest / "secret.txt") == secret);

    // Extract with wrong password -> should throw
    fs::path invalid_dest = temp_dir / "cpp_decrypted_invalid";
    bool failed_as_expected = false;
    try {
        ttzip::extract_archive(enc_zip.string(), invalid_dest.string(), wrong_pwd);
    } catch (const std::exception&) {
        failed_as_expected = true;
    }
    assert(failed_as_expected);

    std::cout << "  [PASS] C++ SDK AES-256 password protection & authentication error OK\n";
}

int main() {
    std::cout << "⚡️ Running TTZip Modern C++20 Comprehensive SDK Test Suite...\n";
    ScopedTempDirectory temp_dir("ttzip_cpp20_suite");

    test_version_and_hw_accel();
    test_span_zero_copy_checksums();
    test_high_level_archiving(temp_dir.path());
    test_raii_archive_writer(temp_dir.path());
    test_raii_archive_reader(temp_dir.path());
    test_multi_format_matrix(temp_dir.path());
    test_memory_buffer_codecs();
    test_password_protection(temp_dir.path());

    std::cout << "✅ All C++20 std::span / std::expected / RAII tests passed successfully!\n";
    return 0;
}
