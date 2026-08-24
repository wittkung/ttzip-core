# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.

cmake_minimum_required(VERSION 3.20)
project(TTZipCppExample LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if(NOT TARGET ttzip::ttzip_cpp)
    list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/../../cmake")
    find_package(ttzip CONFIG QUIET)
    if(NOT TARGET ttzip::ttzip_cpp)
        find_package(TTZip REQUIRED)
    endif()
endif()

add_executable(ttzip_cpp_demo main.cpp)
if(TARGET ttzip::ttzip_cpp)
    target_link_libraries(ttzip_cpp_demo PRIVATE ttzip::ttzip_cpp)
elseif(TARGET TTZip::Core)
    target_link_libraries(ttzip_cpp_demo PRIVATE TTZip::Core)
endif()
