# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <kevintungs@163.com> and TTZip Contributors.
# All rights reserved.
#
# TTZipConfig.cmake — Config package alias / fallback

include(CMakeFindDependencyMacro)

set(THREADS_PREFER_PTHREAD_FLAG ON)
find_dependency(Threads REQUIRED)

if(EXISTS "${CMAKE_CURRENT_LIST_DIR}/ttzipTargets.cmake")
    include("${CMAKE_CURRENT_LIST_DIR}/ttzipTargets.cmake")
elseif(EXISTS "${CMAKE_CURRENT_LIST_DIR}/TTZipTargets.cmake")
    include("${CMAKE_CURRENT_LIST_DIR}/TTZipTargets.cmake")
else()
    # In-tree fallback to FindTTZip
    include("${CMAKE_CURRENT_LIST_DIR}/FindTTZip.cmake")
endif()

if(TARGET ttzip::ttzip_c AND NOT TARGET TTZip::Core)
    add_library(TTZip::Core ALIAS ttzip::ttzip_c)
endif()

if(TARGET ttzip::ttzip_c AND NOT TARGET TTZip::ttzip_c)
    add_library(TTZip::ttzip_c ALIAS ttzip::ttzip_c)
endif()

if(TARGET ttzip::ttzip_cpp AND NOT TARGET TTZip::ttzip_cpp)
    add_library(TTZip::ttzip_cpp ALIAS ttzip::ttzip_cpp)
endif()

if(COMMAND check_required_components)
    check_required_components(TTZip)
endif()
