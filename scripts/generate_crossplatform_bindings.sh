#!/usr/bin/env bash
# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause

set -euo pipefail

# ==============================================================================
# Cross-Platform Multi-Host UniFFI Binding Generation & Drift Verification Tool
#
# Generates and synchronizes strongly-typed foreign language bindings for:
#   1. macOS: Swift 6 bindings (targeting TTZipCore and TTZipApple)
#   2. Windows: C++20 and C# 12 / .NET 8 bindings (targeting WinUI 3)
#   3. Linux: C++20 bindings (targeting Qt 6 / Slint)
#
# Supports --check mode for CI interface drift gate enforcement.
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CORE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_ROOT="${BUILD_WORKSPACE_DIRECTORY:-$(cd "${CORE_DIR}/../../.." && pwd)}"
RUST_DIR="${CORE_DIR}/rust"
BINDINGS_DIR="${REPO_ROOT}/products/ttzip/bindings"
BINDINGS_CPP_DIR="${BINDINGS_DIR}/cpp"
BINDINGS_CSHARP_DIR="${BINDINGS_DIR}/csharp"
APPLE_DIR="${REPO_ROOT}/products/ttzip/apple"

CHECK_MODE=0
GEN_SWIFT=1
GEN_CPP=1
GEN_CSHARP=1

usage() {
    cat << EOF
Usage: $(basename "$0") [OPTIONS]

Options:
  --check          Dry-run drift verification; exits non-zero if bindings differ from disk
  --swift-only     Generate only macOS Swift 6 bindings
  --cpp-only       Generate only C++20 bindings (Windows WinUI 3 / Linux Qt6 / Slint)
  --csharp-only    Generate only C# 12 bindings (.NET 8 / WinUI 3)
  -h, --help       Show this help message
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --check)
            CHECK_MODE=1
            shift
            ;;
        --swift-only)
            GEN_SWIFT=1
            GEN_CPP=0
            GEN_CSHARP=0
            shift
            ;;
        --cpp-only)
            GEN_SWIFT=0
            GEN_CPP=1
            GEN_CSHARP=0
            shift
            ;;
        --csharp-only)
            GEN_SWIFT=0
            GEN_CPP=0
            GEN_CSHARP=1
            shift
            ;;
        -h|--help)
            usage
            ;;
        *)
            echo "❌ Unknown option: $1"
            usage
            ;;
    esac
done

echo "======================================================================"
echo "🚀 TTZip Cross-Platform Multi-Host UniFFI Binding Generator"
echo "   Mode: $( [ "${CHECK_MODE}" -eq 1 ] && echo "🔍 Check / CI Drift Gate" || echo "📦 Full Generation & Deploy" )"
echo "======================================================================"

# ------------------------------------------------------------------------------
# 1. Build Prerequisites (uniffi-bindgen tool and libttzip_engine cdylib)
#
# Both artifacts are rebuilt on every run and never picked from pre-existing
# cargo `target/` directories or the `bazel-bin` convenience symlink by mtime:
# those may predate the working tree, which silently regenerates stale bindings
# and turns the --check drift gate into a false green.
#
# - The cdylib defines the interface metadata and is built by Bazel (the single
#   source of truth) and resolved through `bazel cquery`.
# - The uniffi-bindgen tool is built by cargo: the Bazel crate universe pins
#   `uniffi` without its `cli` feature, so
#   //products/ttzip/core/rust/ttzip-bindgen:uniffi_bindgen_bin does not
#   compile. Both lockfiles pin the same uniffi version, so the generator and
#   the scaffolding embedded in the cdylib agree.
# ------------------------------------------------------------------------------
DYLIB_TARGET="//products/ttzip/core/rust/ttzip-engine:ttzip_engine_dylib"
BAZEL_CACHE_FLAG=""
if ! nc -z -G 1 127.0.0.1 50051 2>/dev/null; then
    BAZEL_CACHE_FLAG="--remote_cache="
fi

echo "--> [BUILD] Building cdylib via Bazel..."
(cd "${REPO_ROOT}" && bazel build ${BAZEL_CACHE_FLAG} "${DYLIB_TARGET}")
DYLIB_REL="$(cd "${REPO_ROOT}" && bazel cquery ${BAZEL_CACHE_FLAG} --output=files "${DYLIB_TARGET}" 2>/dev/null | grep -E '\.dylib$' | head -n 1 || true)"
DYLIB="${DYLIB_REL:+${REPO_ROOT}/${DYLIB_REL}}"

echo "--> [BUILD] Building uniffi-bindgen via cargo..."
cargo build --quiet --manifest-path "${RUST_DIR}/Cargo.toml" -p ttzip-bindgen --bin uniffi-bindgen
BINDGEN_BIN="${RUST_DIR}/target/debug/uniffi-bindgen"

if [ -z "${DYLIB}" ] || [ ! -x "${BINDGEN_BIN}" ] || [ ! -f "${DYLIB}" ]; then
    echo "❌ Error: Failed to resolve executable uniffi-bindgen or cdylib."
    exit 1
fi

echo "--> [OK] Using uniffi-bindgen: ${BINDGEN_BIN}"
echo "--> [OK] Using cdylib: ${DYLIB}"

# ------------------------------------------------------------------------------
# 2. Generate Base Scaffolding via uniffi-bindgen in Staging Sandbox
# ------------------------------------------------------------------------------
STAGING_DIR="$(mktemp -d /tmp/ttzip_bindings_staging.XXXXXX)"
trap 'rm -rf "${STAGING_DIR}"' EXIT

mkdir -p "${STAGING_DIR}/swift" "${STAGING_DIR}/cpp" "${STAGING_DIR}/csharp"

echo "--> [GEN] Running uniffi-bindgen generate..."
(
    cd "${RUST_DIR}"
    "${BINDGEN_BIN}" generate \
        --library "${DYLIB}" \
        --language swift \
        --out-dir "${STAGING_DIR}/swift" \
        --no-format \
        --metadata-no-deps
)

# ------------------------------------------------------------------------------
# 3. Process macOS Swift 6 Bindings
# ------------------------------------------------------------------------------
if [ "${GEN_SWIFT}" -eq 1 ]; then
    echo "--> [SWIFT] Post-processing Swift 6 concurrency and Sendable invariants..."
    if [ -f "${CORE_DIR}/scripts/postprocess_uniffi_swift.py" ] && [ -f "${STAGING_DIR}/swift/ttzip_engine.swift" ]; then
        python3 "${CORE_DIR}/scripts/postprocess_uniffi_swift.py" "${STAGING_DIR}/swift/ttzip_engine.swift"
    fi
fi

# ------------------------------------------------------------------------------
# 4. Generate C++20 Bindings for Windows (WinUI 3) & Linux (Qt 6 / Slint)
# ------------------------------------------------------------------------------
if [ "${GEN_CPP}" -eq 1 ]; then
    echo "--> [CPP] Synthesizing strongly-typed C++20 header bindings..."
    cp -f "${STAGING_DIR}/swift/ttzip_engineFFI.h" "${STAGING_DIR}/cpp/ttzip_engineFFI.h"

    cat << 'EOF' > "${STAGING_DIR}/cpp/ttzip_engine.hpp"
// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

#ifndef TTZIP_ENGINE_HPP
#define TTZIP_ENGINE_HPP

#include "ttzip_engineFFI.h"

#include <cstdint>
#include <cstring>
#include <memory>
#include <optional>
#include <span>
#include <stdexcept>
#include <string>
#include <string_view>
#include <vector>

namespace ttzip::bindings {

constexpr uint32_t EXPECTED_UNIFFI_CONTRACT_VERSION = 26;

enum class ArchiveFormat : int32_t {
    Auto = 0,
    Zip = 1,
    SevenZip = 2,
    Tar = 3,
    TarGz = 4,
    TarBz2 = 5,
    TarXz = 6,
    TarZstd = 7,
    Dmg = 8,
    Lzfse = 9,
    Snappy = 10,
    Unknown = 99
};

enum class CompressionLevel : int32_t {
    Store = 0,
    Fastest = 1,
    Fast = 3,
    Normal = 6,
    Maximum = 9,
    Ultra = 11
};

enum class TTZipStatus : int32_t {
    Ok = 0,
    Eof = 1,
    Cancelled = 2,
    ErrInvalidParam = -1,
    ErrFileNotFound = -2,
    ErrMmapFailed = -3,
    ErrCorruptHeader = -4,
    ErrInvalidOffset = -5,
    ErrArchiveInitFailed = -6,
    ErrOpenFailed = -7,
    ErrPathTooLong = -8,
    ErrOutOfMemory = -9,
    ErrInvalidPassword = -10,
    ErrExtractionFailed = -11,
    ErrCompressionFailed = -12,
    ErrSecurityViolation = -30,
    ErrPanicCaught = -99
};

struct SniffMetadata {
    std::string format_name;
    std::string mime_type;
    bool is_archive{false};
    bool is_sfx{false};
    uint64_t sfx_offset{0};
    uint32_t confidence{0};
};

class TTZipException : public std::runtime_error {
public:
    explicit TTZipException(const std::string& message, int32_t error_code = -1)
        : std::runtime_error(message), error_code_(error_code) {}

    [[nodiscard]] int32_t error_code() const noexcept { return error_code_; }

private:
    int32_t error_code_;
};

inline void validate_contract() {
    uint32_t ver = ffi_ttzip_engine_uniffi_contract_version();
    if (ver != EXPECTED_UNIFFI_CONTRACT_VERSION) {
        throw TTZipException(
            "UniFFI scaffolding contract version mismatch: expected " +
            std::to_string(EXPECTED_UNIFFI_CONTRACT_VERSION) + ", got " + std::to_string(ver)
        );
    }

    if (uniffi_ttzip_engine_checksum_func_detect_archive_format() != 2812 ||
        uniffi_ttzip_engine_checksum_func_create_archive_stream() != 6966 ||
        uniffi_ttzip_engine_checksum_func_extract_archive_stream() != 63284 ||
        uniffi_ttzip_engine_checksum_func_inspect_archive_entries() != 63804 ||
        uniffi_ttzip_engine_checksum_func_verify_archive_integrity() != 56393) {
        throw TTZipException("UniFFI API checksum verification failed. Foreign host bindings drift detected.");
    }
}

inline bool is_abi_compatible() noexcept {
    try {
        validate_contract();
        return true;
    } catch (...) {
        return false;
    }
}

} // namespace ttzip::bindings

#endif // TTZIP_ENGINE_HPP
EOF
fi

# ------------------------------------------------------------------------------
# 5. Generate C# 12 / .NET 8 Bindings for Windows (WinUI 3)
# ------------------------------------------------------------------------------
if [ "${GEN_CSHARP}" -eq 1 ]; then
    echo "--> [CSHARP] Synthesizing strongly-typed C# 12 P/Invoke bindings..."

    cat << 'EOF' > "${STAGING_DIR}/csharp/TTZipEngine.cs"
// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

using System;
using System.Runtime.InteropServices;

namespace TTZip.Bindings;

public enum ArchiveFormat : int
{
    Auto = 0,
    Zip = 1,
    SevenZip = 2,
    Tar = 3,
    TarGz = 4,
    TarBz2 = 5,
    TarXz = 6,
    TarZstd = 7,
    Dmg = 8,
    Lzfse = 9,
    Snappy = 10,
    Unknown = 99
}

public enum CompressionLevel : int
{
    Store = 0,
    Fastest = 1,
    Fast = 3,
    Normal = 6,
    Maximum = 9,
    Ultra = 11
}

public enum TTZipStatus : int
{
    Ok = 0,
    Eof = 1,
    Cancelled = 2,
    ErrInvalidParam = -1,
    ErrFileNotFound = -2,
    ErrMmapFailed = -3,
    ErrCorruptHeader = -4,
    ErrInvalidOffset = -5,
    ErrArchiveInitFailed = -6,
    ErrOpenFailed = -7,
    ErrPathTooLong = -8,
    ErrOutOfMemory = -9,
    ErrInvalidPassword = -10,
    ErrExtractionFailed = -11,
    ErrCompressionFailed = -12,
    ErrSecurityViolation = -30,
    ErrPanicCaught = -99
}

public static class TTZipEngine
{
    private const string NativeLibName = "ttzip_engine";
    public const uint ExpectedContractVersion = 26;

    [DllImport(NativeLibName, EntryPoint = "ffi_ttzip_engine_uniffi_contract_version", CallingConvention = CallingConvention.Cdecl)]
    public static extern uint UniffiContractVersion();

    [DllImport(NativeLibName, EntryPoint = "uniffi_ttzip_engine_checksum_func_detect_archive_format", CallingConvention = CallingConvention.Cdecl)]
    public static extern ushort ChecksumDetectArchiveFormat();

    [DllImport(NativeLibName, EntryPoint = "uniffi_ttzip_engine_checksum_func_create_archive_stream", CallingConvention = CallingConvention.Cdecl)]
    public static extern ushort ChecksumCreateArchiveStream();

    [DllImport(NativeLibName, EntryPoint = "uniffi_ttzip_engine_checksum_func_extract_archive_stream", CallingConvention = CallingConvention.Cdecl)]
    public static extern ushort ChecksumExtractArchiveStream();

    [DllImport(NativeLibName, EntryPoint = "uniffi_ttzip_engine_checksum_func_inspect_archive_entries", CallingConvention = CallingConvention.Cdecl)]
    public static extern ushort ChecksumInspectArchiveEntries();

    [DllImport(NativeLibName, EntryPoint = "uniffi_ttzip_engine_checksum_func_verify_archive_integrity", CallingConvention = CallingConvention.Cdecl)]
    public static extern ushort ChecksumVerifyArchiveIntegrity();

    public static void ValidateContract()
    {
        uint ver = UniffiContractVersion();
        if (ver != ExpectedContractVersion)
        {
            throw new InvalidOperationException(
                $"UniFFI contract version mismatch: expected {ExpectedContractVersion}, got {ver}."
            );
        }

        if (ChecksumDetectArchiveFormat() != 2812 ||
            ChecksumCreateArchiveStream() != 6966 ||
            ChecksumExtractArchiveStream() != 63284 ||
            ChecksumInspectArchiveEntries() != 63804 ||
            ChecksumVerifyArchiveIntegrity() != 56393)
        {
            throw new InvalidOperationException(
                "UniFFI API checksum validation failed. Interface drift detected between native core and C# bindings."
            );
        }
    }

    public static bool IsAbiCompatible()
    {
        try
        {
            ValidateContract();
            return true;
        }
        catch
        {
            return false;
        }
    }
}
EOF

    cat << 'EOF' > "${STAGING_DIR}/csharp/TTZip.Bindings.csproj"
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFrameworks>net8.0;net8.0-windows10.0.19041.0</TargetFrameworks>
    <Nullable>enable</Nullable>
    <AllowUnsafeBlocks>true</AllowUnsafeBlocks>
    <ImplicitUsings>enable</ImplicitUsings>
    <LangVersion>12.0</LangVersion>
    <AssemblyName>TTZip.Bindings</AssemblyName>
    <RootNamespace>TTZip.Bindings</RootNamespace>
    <GenerateDocumentationFile>true</GenerateDocumentationFile>
  </PropertyGroup>
</Project>
EOF
fi

# ------------------------------------------------------------------------------
# 6. Synchronization or Drift Verification Gate
# ------------------------------------------------------------------------------
DRIFT_COUNT=0
UPDATED_COUNT=0

sync_or_check_file() {
    local src="$1"
    local dst="$2"

    if [ ! -f "${src}" ]; then
        echo "❌ Internal error: Generated source missing: ${src}"
        exit 1
    fi

    if [ "${CHECK_MODE}" -eq 1 ]; then
        if [ ! -f "${dst}" ]; then
            echo "❌ [DRIFT] Destination file is missing on disk: ${dst}"
            DRIFT_COUNT=$((DRIFT_COUNT + 1))
        elif ! cmp -s "${src}" "${dst}"; then
            echo "❌ [DRIFT] Interface drift detected in: ${dst}"
            diff -u "${dst}" "${src}" || true
            DRIFT_COUNT=$((DRIFT_COUNT + 1))
        else
            echo "  [OK] Up-to-date: ${dst}"
        fi
    else
        mkdir -p "$(dirname "${dst}")"
        if [ ! -f "${dst}" ] || ! cmp -s "${src}" "${dst}"; then
            cp -f "${src}" "${dst}"
            echo "--> [DEPLOY] Synchronized: ${dst}"
            UPDATED_COUNT=$((UPDATED_COUNT + 1))
        else
            echo "  [UNCHANGED] ${dst}"
        fi
    fi
}

echo "--> [VERIFY] Reconciling staged bindings with repository disk trees..."

if [ "${GEN_SWIFT}" -eq 1 ]; then
    sync_or_check_file "${STAGING_DIR}/swift/ttzip_engine.swift" "${CORE_DIR}/Sources/TTZipCore/Generated/ttzip_engine.swift"
    sync_or_check_file "${STAGING_DIR}/swift/ttzip_engineFFI.h" "${CORE_DIR}/Sources/TTZipCore/Generated/ttzip_engineFFI.h"
    sync_or_check_file "${STAGING_DIR}/swift/ttzip_engineFFI.h" "${CORE_DIR}/Sources/TTZipEngineFFI/include/ttzip_engineFFI.h"
    sync_or_check_file "${STAGING_DIR}/swift/ttzip_engineFFI.modulemap" "${CORE_DIR}/Sources/TTZipCore/Generated/ttzip_engineFFI.modulemap"

    # Synchronize to vendor xcframework header path if framework exists
    for hdr_dir in "${CORE_DIR}/Frameworks/TTZipVendor.xcframework"/*/Headers; do
        if [ -d "${hdr_dir}" ]; then
            sync_or_check_file "${STAGING_DIR}/swift/ttzip_engineFFI.h" "${hdr_dir}/ttzip_engineFFI.h"
        fi
    done
    for hdr_dir in "${APPLE_DIR}/Frameworks/TTZipVendor.xcframework"/*/Headers; do
        if [ -d "${hdr_dir}" ]; then
            sync_or_check_file "${STAGING_DIR}/swift/ttzip_engineFFI.h" "${hdr_dir}/ttzip_engineFFI.h"
        fi
    done
fi

if [ "${GEN_CPP}" -eq 1 ]; then
    sync_or_check_file "${STAGING_DIR}/cpp/ttzip_engineFFI.h" "${BINDINGS_CPP_DIR}/ttzip_engineFFI.h"
    sync_or_check_file "${STAGING_DIR}/cpp/ttzip_engine.hpp" "${BINDINGS_CPP_DIR}/ttzip_engine.hpp"
fi

if [ "${GEN_CSHARP}" -eq 1 ]; then
    sync_or_check_file "${STAGING_DIR}/csharp/TTZipEngine.cs" "${BINDINGS_CSHARP_DIR}/TTZipEngine.cs"
    sync_or_check_file "${STAGING_DIR}/csharp/TTZip.Bindings.csproj" "${BINDINGS_CSHARP_DIR}/TTZip.Bindings.csproj"
fi

echo "======================================================================"
if [ "${CHECK_MODE}" -eq 1 ]; then
    if [ "${DRIFT_COUNT}" -gt 0 ]; then
        echo "❌ [GATE FAILED] Detected ${DRIFT_COUNT} drifting binding file(s)."
        echo "   Run './products/ttzip/core/scripts/generate_crossplatform_bindings.sh' to re-sync."
        exit 1
    else
        echo "✅ [GATE PASSED] All cross-platform foreign bindings are 100% in sync with zero drift."
        exit 0
    fi
else
    echo "✅ [SUCCESS] Foreign bindings synchronized successfully (${UPDATED_COUNT} file(s) updated)."
    exit 0
fi
