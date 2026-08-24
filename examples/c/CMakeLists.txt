# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.

cmake_minimum_required(VERSION 3.20)
project(TTZipCExample LANGUAGES C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

if(NOT TARGET ttzip::ttzip_c)
    list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/../../cmake")
    find_package(ttzip CONFIG QUIET)
    if(NOT TARGET ttzip::ttzip_c)
        find_package(TTZip REQUIRED)
    endif()
endif()

add_executable(ttzip_c_demo main.c)
if(TARGET ttzip::ttzip_c)
    target_link_libraries(ttzip_c_demo PRIVATE ttzip::ttzip_c)
elseif(TARGET TTZip::Core)
    target_link_libraries(ttzip_c_demo PRIVATE TTZip::Core)
endif()

if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/advanced_example.c")
    add_executable(ttzip_c_advanced advanced_example.c)
    if(TARGET ttzip::ttzip_c)
        target_link_libraries(ttzip_c_advanced PRIVATE ttzip::ttzip_c)
    elseif(TARGET TTZip::Core)
        target_link_libraries(ttzip_c_advanced PRIVATE TTZip::Core)
    endif()
endif()
