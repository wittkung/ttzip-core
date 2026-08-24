# ⚡️ TTZip C++20 & C11 Developer Guide

[![C++20](https://img.shields.io/badge/C%2B%2B-20%20RAII%20SDK-blue.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/Sources/CTTZipBridge/include/ttzip.hpp)
[![C11](https://img.shields.io/badge/C-11%20Canonical%20ABI%202.0-brightgreen.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/Sources/CTTZipBridge/include/ttzip.h)
[![Standards: SEI CERT C](https://img.shields.io/badge/Standard-SEI%20CERT%20C%20Compliant-purple.svg)](file:///Users/kevintung/Documents/dev/products/ttzip/core/ARCHITECTURE.md)

The TTZip C++ and C SDK provides high-performance, memory-safe native bindings for systems programming. It includes the **C++20 Modern Header-Only RAII SDK** (`ttzip.hpp`) utilizing `std::expected` and `std::span`, as well as the standardized **C11 ABI 2.0 Contract Header** (`ttzip.h`).

---

## 1. CMake Integration

Add TTZip to your `CMakeLists.txt`:

```cmake
cmake_minimum_required(VERSION 3.20)
project(MyArchiveApp LANGUAGES C CXX)

set(CMAKE_C_STANDARD 11)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# 1. Include TTZip headers
include_directories("${CMAKE_CURRENT_SOURCE_DIR}/core/Sources/CTTZipBridge/include")

# 2. Link against static or dynamic TTZip native library
add_executable(my_app main.cpp)
target_link_libraries(my_app PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/core/rust/target/release/libttzip_engine.a"
    archive bz2 z lzma
)

# On macOS, link Security framework
if (APPLE)
    target_link_libraries(my_app PRIVATE "-framework Security")
endif()
```

---

## 2. Modern C++20 RAII Guide (`ttzip.hpp`)

`ttzip.hpp` provides zero-cost abstractions over the native engine with monadic error handling (`ttzip::expected`):

### 2.1 RAII Archive Creation (`ttzip::ArchiveWriter`)

```cpp
#include "ttzip.hpp"
#include <iostream>

int main() {
    // Fluent builder pattern with RAII execution
    auto writer = ttzip::ArchiveWriter::create("dist/release_v1.zip", ttzip::ArchiveFormat::Zip)
        .value()
        .add_file("include/ttzip.hpp")
        .add_file("README.md")
        .set_level(ttzip::CompressionLevel::Maximum) // Level 9
        .set_threads(0);                             // 0 = Auto-detect cores

    auto result = writer.finish();
    if (!result.has_value()) {
        std::cerr << "Archive creation failed: " << result.error() << std::endl;
        return 1;
    }

    std::cout << "Archive created successfully at: dist/release_v1.zip\n";
    return 0;
}
```

### 2.2 RAII Archive Inspection & Safe Extraction (`ttzip::ArchiveReader`)

```cpp
#include "ttzip.hpp"
#include <iostream>

int main() {
    auto reader_result = ttzip::ArchiveReader::open("dist/release_v1.zip");
    if (!reader_result) {
        std::cerr << "Failed to open archive: " << reader_result.error() << std::endl;
        return 1;
    }

    const auto& reader = reader_result.value();
    std::cout << "Archive contains " << reader.entries().size() << " entries:\n";

    for (const auto& entry : reader.entries()) {
        std::cout << "  - " << entry.path
                  << " (" << entry.uncompressed_size << " bytes, CRC32: "
                  << std::hex << entry.crc32 << std::dec << ")\n";
    }

    // Extract all entries safely to destination directory
    auto extract_result = reader.extract_all("dist/extracted_output");
    if (!extract_result) {
        std::cerr << "Extraction failed: " << extract_result.error() << std::endl;
        return 1;
    }

    std::cout << "All files extracted successfully.\n";
    return 0;
}
```

### 2.3 Zero-Copy In-Memory Checksums (`std::span`)

```cpp
#include "ttzip.hpp"
#include <iostream>
#include <vector>

int main() {
    std::vector<uint8_t> buffer = {'T', 'T', 'Z', 'i', 'p', ' ', 'C', '+', '+', '2', '0'};

    // Hardware SIMD CRC-32 & CRC-64 over std::span
    uint32_t crc32_val = ttzip::crc32(std::span{buffer});
    uint64_t crc64_val = ttzip::crc64(std::span{buffer});

    std::cout << "SIMD CRC-32: 0x" << std::hex << crc32_val << "\n";
    std::cout << "SIMD CRC-64: 0x" << std::hex << crc64_val << std::dec << "\n";
    return 0;
}
```

---

## 3. Canonical C11 ABI 2.0 Guide (`ttzip.h`)

`ttzip.h` is the pure C11 header defining standardized ABI 2.0 structures and functions:

### 3.1 C11 Archive Compression

```c
#include "ttzip.h"
#include <stdio.h>
#include <string.h>

int main(void) {
    const char *sources[] = {
        "src/main.c",
        "include/ttzip.h"
    };
    const char *destination = "dist/c_archive.zip";

    TTZipCreateOptions options;
    memset(&options, 0, sizeof(options));
    options.struct_size = sizeof(TTZipCreateOptions);
    options.abi_version = 2;
    options.format = TTZIP_ARCHIVE_FORMAT_ZIP;
    options.level = TTZIP_COMPRESSION_LEVEL_NORMAL;
    options.thread_budget = 0; // Auto-detect

    TTZipStatus status = ttzip_create_archive(
        sources,
        2,
        destination,
        &options
    );

    if (status != TTZIP_STATUS_OK) {
        fprintf(stderr, "Archive creation failed: %s (code %d)\n",
                ttzip_status_string(status), status);
        return 1;
    }

    printf("C11 Archive created successfully: %s\n", destination);
    return 0;
}
```

### 3.2 C11 Archive Inspection with Callback

```c
#include "ttzip.h"
#include <stdio.h>

static bool inspect_entry_callback(const TTZipEntryMetadata *entry, void *user_data) {
    if (!entry) return false;
    printf("Entry: %-30s | Size: %10llu bytes | CRC32: %08X\n",
           entry->path,
           (unsigned long long)entry->uncompressed_size,
           entry->crc32);
    return true; // Return true to continue scanning
}

int main(void) {
    const char *archive_path = "dist/c_archive.zip";

    printf("Inspecting %s:\n", archive_path);
    TTZipStatus status = ttzip_inspect_archive(
        archive_path,
        NULL,  // Password (if any)
        true,  // Detect encoding
        inspect_entry_callback,
        NULL
    );

    if (status != TTZIP_STATUS_OK) {
        fprintf(stderr, "Inspection failed: %s\n", ttzip_status_string(status));
        return 1;
    }
    return 0;
}
```

### 3.3 C11 Archive Extraction

```c
#include "ttzip.h"
#include <stdio.h>
#include <string.h>

int main(void) {
    const char *archive_path = "dist/c_archive.zip";
    const char *destination = "dist/c_extracted/";

    TTZipExtractOptions options;
    memset(&options, 0, sizeof(options));
    options.struct_size = sizeof(TTZipExtractOptions);
    options.abi_version = 2;
    options.overwrite_existing = true;
    options.preserve_permissions = true;

    TTZipStatus status = ttzip_extract_archive(
        archive_path,
        destination,
        &options
    );

    if (status != TTZIP_STATUS_OK) {
        fprintf(stderr, "Extraction failed: %s\n", ttzip_status_string(status));
        return 1;
    }

    printf("Extracted successfully to: %s\n", destination);
    return 0;
}
```

---

## 4. Status Codes & Diagnostics

| `TTZipStatus` Enum | Numerical Value | Meaning |
| :--- | :---: | :--- |
| `TTZIP_STATUS_OK` | `0` | Operation succeeded |
| `TTZIP_STATUS_EOF` | `1` | End of stream reached |
| `TTZIP_STATUS_CANCELLED` | `2` | Cancelled via progress callback or token |
| `TTZIP_STATUS_ERR_INVALID_PARAM` | `-1` | Invalid argument or null pointer |
| `TTZIP_STATUS_ERR_FILE_NOT_FOUND` | `-2` | Source or archive file does not exist |
| `TTZIP_STATUS_ERR_MMAP_FAILED` | `-3` | APFS memory mapping failure |
| `TTZIP_STATUS_ERR_CORRUPT_HEADER` | `-4` | Malformed archive header or bad magic |
| `TTZIP_STATUS_ERR_INVALID_PASSWORD` | `-10` | Bad password or failed HMAC auth tag |
| `TTZIP_STATUS_ERR_SECURITY_VIOLATION` | `-30` | Path traversal (Zip Slip) attempt detected |
| `TTZIP_STATUS_ERR_PANIC_CAUGHT` | `-99` | Panic trapped at Rust FFI boundary |
