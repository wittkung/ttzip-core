# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# FindTTZip.cmake — CMake Module to locate TTZip Native Engine & Transitive Dependencies

#[=======================================================================[.rst:
FindTTZip
---------

Finds the TTZip high-performance archiving and compression engine and headers.

Imported Targets
^^^^^^^^^^^^^^^^
``ttzip::ttzip_c``
  The TTZip C11 C-ABI interface library target.
``ttzip::ttzip_cpp``
  The TTZip Modern C++20 RAII interface library target.
``TTZip::Core``
  Legacy backward-compatible target.

Result Variables
^^^^^^^^^^^^^^^^
``TTZip_FOUND`` / ``TTZIP_FOUND``
  True if TTZip headers and libraries were found.
``TTZip_INCLUDE_DIRS``
  Include directories containing ``ttzip.h`` and ``ttzip.hpp``.
``TTZip_LIBRARIES``
  All libraries and frameworks required to link against TTZip.
``TTZip_VERSION``
  Version of the TTZip library found.
#]=======================================================================]

find_path(TTZip_INCLUDE_DIR
    NAMES ttzip.h ttzip.hpp
    HINTS
        ${TTZip_ROOT}
        ${TTZIP_ROOT}
        ENV TTZIP_ROOT
        ENV TTZIP_HOME
    PATHS
        ${CMAKE_CURRENT_LIST_DIR}/../Sources/CTTZipBridge/include
        ${CMAKE_CURRENT_LIST_DIR}/../../Sources/CTTZipBridge/include
        /usr/local/include
        /opt/homebrew/include
        ${CMAKE_INSTALL_PREFIX}/include
        ${CMAKE_CURRENT_SOURCE_DIR}/Sources/CTTZipBridge/include
)

find_library(TTZip_LIBRARY
    NAMES ttzip_engine libttzip_engine TTZipVendor libTTZipVendor ttzip
    HINTS
        ${TTZip_ROOT}
        ${TTZIP_ROOT}
        ENV TTZIP_ROOT
        ENV TTZIP_HOME
    PATHS
        ${CMAKE_CURRENT_LIST_DIR}/../rust/target/release
        ${CMAKE_CURRENT_LIST_DIR}/../rust/target/aarch64-apple-darwin/release
        ${CMAKE_CURRENT_LIST_DIR}/../rust/target/x86_64-apple-darwin/release
        ${CMAKE_CURRENT_LIST_DIR}/../rust/target/debug
        ${CMAKE_CURRENT_LIST_DIR}/../Vendor/TTZipVendor.xcframework/macos-arm64
        ${CMAKE_CURRENT_LIST_DIR}/../../rust/target/release
        ${CMAKE_CURRENT_LIST_DIR}/../../rust/target/aarch64-apple-darwin/release
        ${CMAKE_CURRENT_LIST_DIR}/../../rust/target/debug
        ${CMAKE_CURRENT_LIST_DIR}/../../Vendor/TTZipVendor.xcframework/macos-arm64
        /usr/local/lib
        /opt/homebrew/lib
        ${CMAKE_INSTALL_PREFIX}/lib
        ${CMAKE_CURRENT_SOURCE_DIR}/rust/target/release
        ${CMAKE_CURRENT_SOURCE_DIR}/rust/target/aarch64-apple-darwin/release
        ${CMAKE_CURRENT_SOURCE_DIR}/rust/target/debug
        ${CMAKE_CURRENT_SOURCE_DIR}/Vendor/TTZipVendor.xcframework/macos-arm64
)

# Transitive Dependencies
set(THREADS_PREFER_PTHREAD_FLAG ON)
find_package(Threads QUIET)

find_library(TTZip_ARCHIVE_LIB NAMES archive libarchive)
find_library(TTZip_BZ2_LIB NAMES bz2 bzip2 libbz2)
find_library(TTZip_Z_LIB NAMES z zlib libz)
find_library(TTZip_LZMA_LIB NAMES lzma liblzma)

set(TTZip_EXTRA_LIBS "")
if(Threads_FOUND)
    list(APPEND TTZip_EXTRA_LIBS Threads::Threads)
endif()

if(TTZip_ARCHIVE_LIB)
    list(APPEND TTZip_EXTRA_LIBS ${TTZip_ARCHIVE_LIB})
else()
    list(APPEND TTZip_EXTRA_LIBS archive)
endif()

if(TTZip_BZ2_LIB)
    list(APPEND TTZip_EXTRA_LIBS ${TTZip_BZ2_LIB})
else()
    list(APPEND TTZip_EXTRA_LIBS bz2)
endif()

if(TTZip_Z_LIB)
    list(APPEND TTZip_EXTRA_LIBS ${TTZip_Z_LIB})
else()
    list(APPEND TTZip_EXTRA_LIBS z)
endif()

if(TTZip_LZMA_LIB)
    list(APPEND TTZip_EXTRA_LIBS ${TTZip_LZMA_LIB})
else()
    list(APPEND TTZip_EXTRA_LIBS lzma)
endif()

if(APPLE)
    find_library(TTZip_SECURITY_FRAMEWORK Security)
    find_library(TTZip_COREFOUNDATION_FRAMEWORK CoreFoundation)
    if(TTZip_SECURITY_FRAMEWORK)
        list(APPEND TTZip_EXTRA_LIBS ${TTZip_SECURITY_FRAMEWORK})
    endif()
    if(TTZip_COREFOUNDATION_FRAMEWORK)
        list(APPEND TTZip_EXTRA_LIBS ${TTZip_COREFOUNDATION_FRAMEWORK})
    endif()
elseif(UNIX AND NOT APPLE)
    find_library(TTZip_M_LIB m)
    find_library(TTZip_DL_LIB dl)
    if(TTZip_M_LIB)
        list(APPEND TTZip_EXTRA_LIBS ${TTZip_M_LIB})
    endif()
    if(TTZip_DL_LIB)
        list(APPEND TTZip_EXTRA_LIBS ${TTZip_DL_LIB})
    endif()
endif()

include(FindPackageHandleStandardArgs)
find_package_handle_standard_args(TTZip
    REQUIRED_VARS TTZip_LIBRARY TTZip_INCLUDE_DIR
    VERSION_VAR TTZip_VERSION
)

if(TTZip_FOUND)
    set(TTZIP_FOUND TRUE)
    set(TTZip_INCLUDE_DIRS "${TTZip_INCLUDE_DIR}")
    set(TTZIP_INCLUDE_DIRS "${TTZip_INCLUDE_DIR}")
    set(TTZip_LIBRARIES "${TTZip_LIBRARY};${TTZip_EXTRA_LIBS}")
    set(TTZIP_LIBRARIES "${TTZip_LIBRARIES}")

    # Target: ttzip::ttzip_c
    if(NOT TARGET ttzip::ttzip_c)
        add_library(ttzip::ttzip_c UNKNOWN IMPORTED)
        set_target_properties(ttzip::ttzip_c PROPERTIES
            IMPORTED_LOCATION "${TTZip_LIBRARY}"
            INTERFACE_INCLUDE_DIRECTORIES "${TTZip_INCLUDE_DIR}"
            INTERFACE_LINK_LIBRARIES "${TTZip_EXTRA_LIBS}"
            INTERFACE_COMPILE_FEATURES "c_std_11"
        )
    endif()

    # Target: ttzip::ttzip_cpp
    if(NOT TARGET ttzip::ttzip_cpp)
        add_library(ttzip::ttzip_cpp UNKNOWN IMPORTED)
        set_target_properties(ttzip::ttzip_cpp PROPERTIES
            IMPORTED_LOCATION "${TTZip_LIBRARY}"
            INTERFACE_INCLUDE_DIRECTORIES "${TTZip_INCLUDE_DIR}"
            INTERFACE_LINK_LIBRARIES "ttzip::ttzip_c"
            INTERFACE_COMPILE_FEATURES "cxx_std_20"
        )
    endif()

    # Backward compatibility aliases
    if(NOT TARGET TTZip::Core)
        add_library(TTZip::Core ALIAS ttzip::ttzip_c)
    endif()
    if(NOT TARGET TTZip::ttzip_c)
        add_library(TTZip::ttzip_c ALIAS ttzip::ttzip_c)
    endif()
    if(NOT TARGET TTZip::ttzip_cpp)
        add_library(TTZip::ttzip_cpp ALIAS ttzip::ttzip_cpp)
    endif()
endif()
