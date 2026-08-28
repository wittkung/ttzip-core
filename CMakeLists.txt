# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# TTZip: Unified Modern CMake Build System (CMake 3.20+)
# Exports: ttzip::ttzip_c (C11) and ttzip::ttzip_cpp (C++20 RAII)

cmake_minimum_required(VERSION 3.20)
project(ttzip VERSION 1.0.0 LANGUAGES C CXX)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

include(GNUInstallDirs)
include(CMakePackageConfigHelpers)

# -----------------------------------------------------------------------------
# Options
# -----------------------------------------------------------------------------
if(CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR)
    set(TTZIP_IS_TOP_LEVEL ON)
else()
    set(TTZIP_IS_TOP_LEVEL OFF)
endif()

option(TTZIP_BUILD_EXAMPLES "Build C11 and C++20 quickstart examples" ${TTZIP_IS_TOP_LEVEL})
option(TTZIP_BUILD_TESTS "Build C11 and C++20 test suites" ${TTZIP_IS_TOP_LEVEL})
option(TTZIP_INSTALL "Generate install targets for headers, libraries, and CMake config" ON)

# -----------------------------------------------------------------------------
# Transitive Dependencies Resolution
# -----------------------------------------------------------------------------
set(THREADS_PREFER_PTHREAD_FLAG ON)
find_package(Threads REQUIRED)

set(TTZIP_TRANSITIVE_LIBS "")
list(APPEND TTZIP_TRANSITIVE_LIBS Threads::Threads)

# LibArchive
find_library(TTZIP_LIBARCHIVE_LIB NAMES archive libarchive)
if(TTZIP_LIBARCHIVE_LIB)
    list(APPEND TTZIP_TRANSITIVE_LIBS ${TTZIP_LIBARCHIVE_LIB})
else()
    list(APPEND TTZIP_TRANSITIVE_LIBS archive)
endif()

# BZip2
find_library(TTZIP_BZIP2_LIB NAMES bz2 bzip2 libbz2)
if(TTZIP_BZIP2_LIB)
    list(APPEND TTZIP_TRANSITIVE_LIBS ${TTZIP_BZIP2_LIB})
else()
    list(APPEND TTZIP_TRANSITIVE_LIBS bz2)
endif()

# ZLIB
find_library(TTZIP_ZLIB_LIB NAMES z zlib libz)
if(TTZIP_ZLIB_LIB)
    list(APPEND TTZIP_TRANSITIVE_LIBS ${TTZIP_ZLIB_LIB})
else()
    list(APPEND TTZIP_TRANSITIVE_LIBS z)
endif()

# LZMA
find_library(TTZIP_LZMA_LIB NAMES lzma liblzma)
if(TTZIP_LZMA_LIB)
    list(APPEND TTZIP_TRANSITIVE_LIBS ${TTZIP_LZMA_LIB})
else()
    list(APPEND TTZIP_TRANSITIVE_LIBS lzma)
endif()

# Platform-specific frameworks / system libraries
if(APPLE)
    find_library(TTZIP_SECURITY_FRAMEWORK Security REQUIRED)
    find_library(TTZIP_COREFOUNDATION_FRAMEWORK CoreFoundation REQUIRED)
    list(APPEND TTZIP_TRANSITIVE_LIBS ${TTZIP_SECURITY_FRAMEWORK} ${TTZIP_COREFOUNDATION_FRAMEWORK})
elseif(UNIX AND NOT APPLE)
    find_library(TTZIP_M_LIB m)
    if(TTZIP_M_LIB)
        list(APPEND TTZIP_TRANSITIVE_LIBS ${TTZIP_M_LIB})
    endif()
    find_library(TTZIP_DL_LIB dl)
    if(TTZIP_DL_LIB)
        list(APPEND TTZIP_TRANSITIVE_LIBS ${TTZIP_DL_LIB})
    endif()
endif()

# -----------------------------------------------------------------------------
# Locate libttzip_engine Static Library
# -----------------------------------------------------------------------------
if(NOT TTZIP_ENGINE_LIBRARY)
    find_file(TTZIP_ENGINE_LIBRARY
        NAMES libttzip_engine.a libTTZipVendor.a
        PATHS
            ${CMAKE_CURRENT_SOURCE_DIR}/rust/target/release
            ${CMAKE_CURRENT_SOURCE_DIR}/rust/target/aarch64-apple-darwin/release
            ${CMAKE_CURRENT_SOURCE_DIR}/rust/target/x86_64-apple-darwin/release
            ${CMAKE_CURRENT_SOURCE_DIR}/rust/target/debug
            ${CMAKE_CURRENT_SOURCE_DIR}/Vendor/TTZipVendor.xcframework/macos-arm64
            ${CMAKE_INSTALL_PREFIX}/${CMAKE_INSTALL_LIBDIR}
            /usr/local/lib
            /opt/homebrew/lib
        NO_DEFAULT_PATH
    )
endif()

if(NOT TTZIP_ENGINE_LIBRARY OR NOT EXISTS "${TTZIP_ENGINE_LIBRARY}")
    # Fallback search in standard paths
    find_library(TTZIP_ENGINE_LIBRARY NAMES ttzip_engine TTZipVendor ttzip)
endif()

if(NOT TTZIP_ENGINE_LIBRARY)
    message(WARNING "libttzip_engine.a not found. Please build Rust engine (scripts/build_rust.sh) or specify -DTTZIP_ENGINE_LIBRARY=<path>")
else()
    message(STATUS "Found TTZip Native Engine Library: ${TTZIP_ENGINE_LIBRARY}")
endif()

# -----------------------------------------------------------------------------
# Target: ttzip_c (C11 Interface Target)
# -----------------------------------------------------------------------------
add_library(ttzip_c INTERFACE)
add_library(ttzip::ttzip_c ALIAS ttzip_c)

# Legacy alias
add_library(TTZip::ttzip_c ALIAS ttzip_c)
add_library(TTZip::Core ALIAS ttzip_c)

target_compile_features(ttzip_c INTERFACE c_std_11)

target_include_directories(ttzip_c INTERFACE
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/sdk/include>
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/Sources/CTTZipBridge/include>
    $<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDEDIR}>
    $<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDEDIR}/ttzip>
)

if(TTZIP_ENGINE_LIBRARY)
    target_link_libraries(ttzip_c INTERFACE
        $<BUILD_INTERFACE:${TTZIP_ENGINE_LIBRARY}>
        $<INSTALL_INTERFACE:$<INSTALL_PREFIX>/${CMAKE_INSTALL_LIBDIR}/libttzip_engine.a>
        ${TTZIP_TRANSITIVE_LIBS}
    )
else()
    target_link_libraries(ttzip_c INTERFACE ${TTZIP_TRANSITIVE_LIBS})
endif()

# -----------------------------------------------------------------------------
# Target: ttzip_cpp (C++20 RAII Interface Target)
# -----------------------------------------------------------------------------
add_library(ttzip_cpp INTERFACE)
add_library(ttzip::ttzip_cpp ALIAS ttzip_cpp)

# Legacy alias
add_library(TTZip::ttzip_cpp ALIAS ttzip_cpp)

target_compile_features(ttzip_cpp INTERFACE cxx_std_20)

target_include_directories(ttzip_cpp INTERFACE
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/sdk/include>
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/Sources/CTTZipBridge/include>
    $<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDEDIR}>
    $<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDEDIR}/ttzip>
)

target_link_libraries(ttzip_cpp INTERFACE ttzip_c)

# -----------------------------------------------------------------------------
# Installation and CMake Config Package Generation
# -----------------------------------------------------------------------------
if(TTZIP_INSTALL)
    # 1. Install Header files
    install(
        FILES
            ${CMAKE_CURRENT_SOURCE_DIR}/sdk/include/ttzip.h
            ${CMAKE_CURRENT_SOURCE_DIR}/sdk/include/ttzip.hpp
            ${CMAKE_CURRENT_SOURCE_DIR}/Sources/CTTZipBridge/include/ttzip_engineFFI.h
        DESTINATION ${CMAKE_INSTALL_INCLUDEDIR}
    )
    install(
        FILES
            ${CMAKE_CURRENT_SOURCE_DIR}/sdk/include/ttzip.h
            ${CMAKE_CURRENT_SOURCE_DIR}/sdk/include/ttzip.hpp
            ${CMAKE_CURRENT_SOURCE_DIR}/Sources/CTTZipBridge/include/ttzip_engineFFI.h
        DESTINATION ${CMAKE_INSTALL_INCLUDEDIR}/ttzip
    )

    # 2. Install Static Engine Archive if available
    if(TTZIP_ENGINE_LIBRARY AND EXISTS "${TTZIP_ENGINE_LIBRARY}")
        install(
            FILES "${TTZIP_ENGINE_LIBRARY}"
            DESTINATION ${CMAKE_INSTALL_LIBDIR}
            RENAME "libttzip_engine.a"
        )
    endif()

    # 3. Export Targets
    install(
        TARGETS ttzip_c ttzip_cpp
        EXPORT ttzipTargets
        INCLUDES DESTINATION ${CMAKE_INSTALL_INCLUDEDIR}
    )

    install(
        EXPORT ttzipTargets
        FILE ttzipTargets.cmake
        NAMESPACE ttzip::
        DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/ttzip
    )

    # 4. Generate CMake Package Config
    set(TTZIP_CONFIG_INSTALL_DIR "${CMAKE_INSTALL_LIBDIR}/cmake/ttzip")

    configure_package_config_file(
        "${CMAKE_CURRENT_SOURCE_DIR}/cmake/ttzipConfig.cmake.in"
        "${CMAKE_CURRENT_BINARY_DIR}/ttzipConfig.cmake"
        INSTALL_DESTINATION "${TTZIP_CONFIG_INSTALL_DIR}"
        PATH_VARS CMAKE_INSTALL_INCLUDEDIR CMAKE_INSTALL_LIBDIR
    )

    configure_package_config_file(
        "${CMAKE_CURRENT_SOURCE_DIR}/cmake/ttzipConfig.cmake.in"
        "${CMAKE_CURRENT_BINARY_DIR}/TTZipConfig.cmake"
        INSTALL_DESTINATION "${TTZIP_CONFIG_INSTALL_DIR}"
        PATH_VARS CMAKE_INSTALL_INCLUDEDIR CMAKE_INSTALL_LIBDIR
    )

    write_basic_package_version_file(
        "${CMAKE_CURRENT_BINARY_DIR}/ttzipConfigVersion.cmake"
        VERSION ${PROJECT_VERSION}
        COMPATIBILITY SameMajorVersion
    )

    write_basic_package_version_file(
        "${CMAKE_CURRENT_BINARY_DIR}/TTZipConfigVersion.cmake"
        VERSION ${PROJECT_VERSION}
        COMPATIBILITY SameMajorVersion
    )

    install(
        FILES
            "${CMAKE_CURRENT_BINARY_DIR}/ttzipConfig.cmake"
            "${CMAKE_CURRENT_BINARY_DIR}/ttzipConfigVersion.cmake"
            "${CMAKE_CURRENT_BINARY_DIR}/TTZipConfig.cmake"
            "${CMAKE_CURRENT_BINARY_DIR}/TTZipConfigVersion.cmake"
        DESTINATION "${TTZIP_CONFIG_INSTALL_DIR}"
    )

    # 5. Generate and install pkg-config file
    if(APPLE)
        set(PC_LIBS_PRIVATE "-larchive -lbz2 -lz -llzma -lpthread -framework Security -framework CoreFoundation")
    else()
        set(PC_LIBS_PRIVATE "-larchive -lbz2 -lz -llzma -lpthread -ldl -lm")
    endif()

    set(PREFIX "${CMAKE_INSTALL_PREFIX}")
    set(EXEC_PREFIX "\${prefix}")
    set(LIBDIR "\${exec_prefix}/${CMAKE_INSTALL_LIBDIR}")
    set(INCLUDEDIR "\${prefix}/${CMAKE_INSTALL_INCLUDEDIR}")
    set(VERSION "${PROJECT_VERSION}")
    set(LIBS_PRIVATE "${PC_LIBS_PRIVATE}")

    configure_file(
        "${CMAKE_CURRENT_SOURCE_DIR}/ttzip.pc.in"
        "${CMAKE_CURRENT_BINARY_DIR}/ttzip.pc"
        @ONLY
    )

    install(
        FILES "${CMAKE_CURRENT_BINARY_DIR}/ttzip.pc"
        DESTINATION "${CMAKE_INSTALL_LIBDIR}/pkgconfig"
    )
endif()

# -----------------------------------------------------------------------------
# Subdirectories: Examples and Tests
# -----------------------------------------------------------------------------
if(TTZIP_BUILD_EXAMPLES)
    if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/examples/c/CMakeLists.txt")
        add_subdirectory(examples/c)
    endif()
    if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/examples/cpp/CMakeLists.txt")
        add_subdirectory(examples/cpp)
    endif()
endif()

if(TTZIP_BUILD_TESTS)
    if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/sdk/c/test_c_sdk.c")
        add_executable(test_c_sdk sdk/c/test_c_sdk.c)
        target_link_libraries(test_c_sdk PRIVATE ttzip::ttzip_c)
    endif()
    if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/sdk/cpp/test_cpp_sdk.cpp")
        add_executable(test_cpp_sdk sdk/cpp/test_cpp_sdk.cpp)
        target_link_libraries(test_cpp_sdk PRIVATE ttzip::ttzip_cpp)
    endif()
endif()
