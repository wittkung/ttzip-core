// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.
// Official C++20 RAII Modern Header-Only SDK.

#ifndef TTZIP_HPP
#define TTZIP_HPP

#include "ttzip.h"

#include <cstdint>
#include <cstring>
#include <filesystem>
#include <functional>
#include <memory>
#include <span>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#if defined(__cpp_lib_expected) && __cpp_lib_expected >= 202202L
#include <expected>
namespace ttzip {
template <typename T, typename E>
using expected = std::expected<T, E>;
template <typename E>
using unexpected = std::unexpected<E>;
} // namespace ttzip
#else
namespace ttzip {
template <typename E>
class unexpected {
public:
    constexpr explicit unexpected(E e) : error_val(std::move(e)) {}
    constexpr const E& error() const noexcept { return error_val; }
    constexpr E& error() noexcept { return error_val; }
    constexpr const E& value() const noexcept { return error_val; }
private:
    E error_val;
};

template <typename T, typename E>
class expected {
public:
    constexpr expected(const T& val) : has_val(true), val(val) {}
    constexpr expected(T&& val) : has_val(true), val(std::move(val)) {}
    constexpr expected(const unexpected<E>& unexp) : has_val(false), err(unexp.error()) {}
    constexpr expected(unexpected<E>&& unexp) : has_val(false), err(std::move(unexp.error())) {}

    ~expected() {
        if (has_val) val.~T();
        else err.~E();
    }

    constexpr bool has_value() const noexcept { return has_val; }
    constexpr explicit operator bool() const noexcept { return has_val; }

    constexpr const T& value() const& {
        if (!has_val) throw std::runtime_error("expected has no value");
        return val;
    }
    constexpr T& value() & {
        if (!has_val) throw std::runtime_error("expected has no value");
        return val;
    }
    constexpr const T* operator->() const noexcept { return &val; }
    constexpr T* operator->() noexcept { return &val; }
    constexpr const T& operator*() const& noexcept { return val; }
    constexpr T& operator*() & noexcept { return val; }

    constexpr const E& error() const& noexcept { return err; }
    constexpr E& error() & noexcept { return err; }

private:
    bool has_val;
    union {
        T val;
        E err;
    };
};

template <typename E>
class expected<void, E> {
public:
    constexpr expected() noexcept : has_val(true) {}
    constexpr expected(const unexpected<E>& unexp) : has_val(false), err(unexp.error()) {}
    constexpr expected(unexpected<E>&& unexp) : has_val(false), err(std::move(unexp.error())) {}

    ~expected() {
        if (!has_val) err.~E();
    }

    constexpr bool has_value() const noexcept { return has_val; }
    constexpr explicit operator bool() const noexcept { return has_val; }

    constexpr void value() const {
        if (!has_val) throw std::runtime_error("expected has no value");
    }

    constexpr const E& error() const& noexcept { return err; }
    constexpr E& error() & noexcept { return err; }

private:
    bool has_val;
    union {
        char dummy;
        E err;
    };
};
} // namespace ttzip
#endif

namespace ttzip {

enum class ArchiveFormat : int32_t {
    Auto = TTZIP_ARCHIVE_FORMAT_AUTO,
    Zip = TTZIP_ARCHIVE_FORMAT_ZIP,
    SevenZip = TTZIP_ARCHIVE_FORMAT_SEVEN_ZIP,
    Tar = TTZIP_ARCHIVE_FORMAT_TAR,
    TarGz = TTZIP_ARCHIVE_FORMAT_TAR_GZ,
    TarBz2 = TTZIP_ARCHIVE_FORMAT_TAR_BZ2,
    TarXz = TTZIP_ARCHIVE_FORMAT_TAR_XZ,
    TarZstd = TTZIP_ARCHIVE_FORMAT_TAR_ZSTD,
    Dmg = TTZIP_ARCHIVE_FORMAT_DMG,
    Lzfse = TTZIP_ARCHIVE_FORMAT_LZFSE,
    Snappy = TTZIP_ARCHIVE_FORMAT_SNAPPY
};

enum class CompressionLevel : int32_t {
    Store = TTZIP_COMPRESSION_LEVEL_STORE,
    Fastest = TTZIP_COMPRESSION_LEVEL_FASTEST,
    Fast = TTZIP_COMPRESSION_LEVEL_FAST,
    Normal = TTZIP_COMPRESSION_LEVEL_NORMAL,
    Maximum = TTZIP_COMPRESSION_LEVEL_MAXIMUM,
    Ultra = TTZIP_COMPRESSION_LEVEL_ULTRA
};

struct EntryMetadata {
    std::string path;
    uint64_t uncompressed_size{0};
    uint64_t compressed_size{0};
    uint32_t crc32{0};
    int64_t mtime_epoch_secs{0};
    uint32_t mode{0};
    bool is_directory{false};
    bool is_encrypted{false};
    uint16_t compression_method{0};
    std::string detected_encoding;
};

struct ArchiveProgress {
    uint64_t processed_bytes{0};
    uint64_t total_bytes{0};
    double fraction_completed{0.0};
    std::string current_entry_path;
    int current_entry_index{0};
    int total_entries{0};
    std::string phase{"processing"};
    double throughput_mbs{0.0};
};

using ProgressCallback = std::function<bool(const ArchiveProgress&)>;

/// Returns the underlying TTZip engine version.
inline std::string version() {
    const char* v = ttzip_rust_version();
    return v ? std::string(v) : "1.0.0";
}

/// Returns true if ARM NEON/Crypto or x86 AVX2/AES-NI acceleration is active.
inline bool is_hardware_accelerated() {
    return ttzip_rust_is_hardware_accelerated();
}

/// Fast SIMD-accelerated CRC-32 (>40 GB/s on Apple Silicon / AVX-512).
inline uint32_t crc32(std::span<const uint8_t> data, uint32_t seed = 0) {
    return ttzip_rust_crc32(seed, data.data(), data.size());
}

/// Fast SIMD-accelerated CRC-64.
inline uint64_t crc64(std::span<const uint8_t> data, uint64_t seed = 0) {
    return ttzip_rust_crc64(seed, data.data(), data.size());
}

/// High-level inspect archive entries.
inline std::vector<EntryMetadata> inspect_archive(
    const std::string& archive_path,
    const std::string& password = ""
) {
    std::vector<EntryMetadata> result;

    struct Context {
        std::vector<EntryMetadata>* out;
    } ctx{&result};

    auto inspect_cb = [](const TTZipEntryMetadata* meta, void* user_data) -> bool {
        if (!meta || !user_data) return false;
        auto* context = static_cast<Context*>(user_data);
        EntryMetadata item;
        item.path = meta->path ? meta->path : "";
        item.uncompressed_size = meta->uncompressed_size;
        item.compressed_size = meta->compressed_size;
        item.crc32 = meta->crc32;
        item.mtime_epoch_secs = meta->mtime_epoch_secs;
        item.mode = meta->mode;
        item.is_directory = meta->is_directory;
        item.is_encrypted = meta->is_encrypted;
        item.compression_method = meta->compression_method;
        item.detected_encoding = meta->detected_encoding ? meta->detected_encoding : "";
        context->out->push_back(std::move(item));
        return true;
    };

    TTZipStatus status = ttzip_rust_inspect_archive(
        archive_path.c_str(),
        password.empty() ? nullptr : password.c_str(),
        true,
        inspect_cb,
        &ctx
    );

    if (status != TTZIP_STATUS_OK) {
        throw std::runtime_error("ttzip::inspect_archive failed with status: " + std::to_string(static_cast<int>(status)));
    }
    return result;
}

/// High-level archive creation.
inline void compress_files(
    const std::vector<std::string>& sources,
    const std::string& destination,
    int level = 6,
    const std::string& password = "",
    ArchiveFormat format = ArchiveFormat::Auto,
    uint32_t threads = 0
) {
    std::vector<const char*> c_sources;
    c_sources.reserve(sources.size());
    for (const auto& s : sources) {
        c_sources.push_back(s.c_str());
    }

    TTZipCreateOptions opts{};
    opts.struct_size = sizeof(TTZipCreateOptions);
    opts.abi_version = 2;
    opts.format = static_cast<TTZipArchiveFormat>(format);
    opts.level = static_cast<TTZipCompressionLevel>(level);
    opts.encryption = password.empty() ? TTZIP_ENCRYPTION_NONE : TTZIP_ENCRYPTION_AES256;
    opts.password = password.empty() ? nullptr : password.c_str();
    opts.thread_budget = threads;
    opts.solid_block_size_mb = 64;

    TTZipStatus status = ttzip_rust_create_archive(
        c_sources.data(),
        c_sources.size(),
        destination.c_str(),
        &opts
    );

    if (status != TTZIP_STATUS_OK) {
        throw std::runtime_error("ttzip::compress_files failed with status: " + std::to_string(static_cast<int>(status)));
    }
}

/// High-level archive extraction.
inline void extract_archive(
    const std::string& archive_path,
    const std::string& destination,
    const std::string& password = "",
    uint32_t threads = 0
) {
    TTZipExtractOptions opts{};
    opts.struct_size = sizeof(TTZipExtractOptions);
    opts.abi_version = 2;
    opts.destination_path = destination.c_str();
    opts.password = password.empty() ? nullptr : password.c_str();
    opts.thread_budget = threads;
    opts.overwrite_existing = true;
    opts.preserve_permissions = true;
    opts.dry_run = false;

    TTZipStatus status = ttzip_rust_extract_archive(
        archive_path.c_str(),
        destination.c_str(),
        &opts
    );

    if (status != TTZIP_STATUS_OK) {
        throw std::runtime_error("ttzip::extract_archive failed with status: " + std::to_string(static_cast<int>(status)));
    }
}

/// RAII Modern Archive Reader.
class ArchiveReader {
public:
    static expected<ArchiveReader, std::string> open(
        const std::filesystem::path& path,
        const std::string& password = ""
    ) {
        if (!std::filesystem::exists(path)) {
            return unexpected<std::string>("Archive file not found: " + path.string());
        }
        ArchiveReader reader(path.string(), password);
        try {
            reader.cached_entries = inspect_archive(reader.archive_path, reader.password);
            return reader;
        } catch (const std::exception& e) {
            return unexpected<std::string>(e.what());
        }
    }

    const std::vector<EntryMetadata>& entries() const noexcept {
        return cached_entries;
    }

    expected<void, std::string> extract_all(const std::filesystem::path& destination_dir) const {
        try {
            std::filesystem::create_directories(destination_dir);
            extract_archive(archive_path, destination_dir.string(), password);
            return {};
        } catch (const std::exception& e) {
            return unexpected<std::string>(e.what());
        }
    }

private:
    ArchiveReader(std::string path, std::string pwd)
        : archive_path(std::move(path)), password(std::move(pwd)) {}

    std::string archive_path;
    std::string password;
    std::vector<EntryMetadata> cached_entries;
};

/// RAII Modern Archive Writer.
class ArchiveWriter {
public:
    static expected<ArchiveWriter, std::string> create(
        const std::filesystem::path& destination,
        ArchiveFormat format = ArchiveFormat::Zip
    ) {
        return ArchiveWriter(destination.string(), format);
    }

    ArchiveWriter& add_file(const std::filesystem::path& file_path) {
        sources.push_back(file_path.string());
        return *this;
    }

    ArchiveWriter& set_level(CompressionLevel lvl) noexcept {
        level = lvl;
        return *this;
    }

    ArchiveWriter& set_password(std::string pwd) noexcept {
        password = std::move(pwd);
        return *this;
    }

    ArchiveWriter& set_threads(uint32_t t) noexcept {
        threads = t;
        return *this;
    }

    expected<void, std::string> finish() {
        if (sources.empty()) {
            return unexpected<std::string>("No source files provided for archive creation");
        }
        try {
            compress_files(
                sources,
                destination_path,
                static_cast<int>(level),
                password,
                format,
                threads
            );
            return {};
        } catch (const std::exception& e) {
            return unexpected<std::string>(e.what());
        }
    }

private:
    ArchiveWriter(std::string dest, ArchiveFormat fmt)
        : destination_path(std::move(dest)), format(fmt) {}

    std::string destination_path;
    ArchiveFormat format{ArchiveFormat::Zip};
    CompressionLevel level{CompressionLevel::Normal};
    std::string password;
    uint32_t threads{0};
    std::vector<std::string> sources;
};

} // namespace ttzip

#endif // TTZIP_HPP
