// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: Modern C++20 SDK Interface Wrapper.

#ifndef TTZIP_HPP
#define TTZIP_HPP

#include "ttzip.h"
#include <string>
#include <vector>
#include <stdexcept>
#include <string_view>
#include <span>

namespace ttzip {

class TTZipException : public std::runtime_error {
public:
    explicit TTZipException(const std::string& msg, TTZipStatus status = TTZIP_STATUS_ERR_PANIC_CAUGHT)
        : std::runtime_error(msg), status_(status) {}

    TTZipStatus status() const noexcept { return status_; }

private:
    TTZipStatus status_;
};

struct EntryInfo {
    std::string path;
    uint64_t uncompressed_size;
    uint64_t compressed_size;
    uint32_t crc32;
    int64_t mtime_epoch_secs;
    bool is_directory;
    bool is_encrypted;
};

inline void compress_files(
    const std::vector<std::string>& source_paths,
    const std::string& destination_path,
    int32_t level = 6,
    const std::string& password = ""
) {
    std::vector<const char*> c_paths;
    c_paths.reserve(source_paths.size());
    for (const auto& p : source_paths) {
        c_paths.push_back(p.c_str());
    }

    TTZipCreateOptions opts{};
    opts.format = TTZIP_ARCHIVE_FORMAT_ZIP;
    opts.level = static_cast<TTZipCompressionLevel>(level);
    opts.password = password.empty() ? nullptr : password.c_str();

    TTZipStatus status = ttzip_create_archive(
        c_paths.data(),
        c_paths.size(),
        destination_path.c_str(),
        &opts
    );

    if (status != TTZIP_STATUS_OK) {
        throw TTZipException("Failed to create archive, error status: " + std::to_string(status), status);
    }
}

inline void extract_archive(
    const std::string& archive_path,
    const std::string& destination_path,
    const std::string& password = ""
) {
    TTZipExtractOptions opts{};
    opts.destination_path = destination_path.c_str();
    opts.overwrite_existing = true;
    opts.password = password.empty() ? nullptr : password.c_str();

    TTZipStatus status = ttzip_extract_archive(
        archive_path.c_str(),
        destination_path.c_str(),
        &opts
    );

    if (status != TTZIP_STATUS_OK) {
        throw TTZipException("Failed to extract archive, error status: " + std::to_string(status), status);
    }
}

inline std::vector<EntryInfo> inspect_archive(const std::string& archive_path, const std::string& password = "") {
    std::vector<EntryInfo> entries;

    auto callback = [](const TTZipEntryMetadata* meta, void* user_data) -> bool {
        if (!meta || !user_data) return false;
        auto* list = static_cast<std::vector<EntryInfo>*>(user_data);
        EntryInfo info;
        info.path = meta->path ? meta->path : "";
        info.uncompressed_size = meta->uncompressed_size;
        info.compressed_size = meta->compressed_size;
        info.crc32 = meta->crc32;
        info.mtime_epoch_secs = meta->mtime_epoch_secs;
        info.is_directory = meta->is_directory;
        info.is_encrypted = meta->is_encrypted;
        list->push_back(std::move(info));
        return true;
    };

    TTZipStatus status = ttzip_inspect_archive(
        archive_path.c_str(),
        password.empty() ? nullptr : password.c_str(),
        true,
        callback,
        &entries
    );

    if (status != TTZIP_STATUS_OK) {
        throw TTZipException("Failed to inspect archive, error status: " + std::to_string(status), status);
    }

    return entries;
}

inline uint32_t crc32(std::span<const uint8_t> data) {
    return ttzip_crc32(data.data(), data.size());
}

inline std::string version() {
    const char* v = ttzip_version();
    return v ? std::string(v) : "1.0.0";
}

} // namespace ttzip

#endif // TTZIP_HPP
