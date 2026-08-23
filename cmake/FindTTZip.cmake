# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com> and TTZip Contributors.
# All rights reserved.
#
# FindTTZip.cmake — CMake Module to locate TTZip Native Engine

#[=======================================================================[.rst:
FindTTZip
-------

Finds the TTZip high-performance archiving and compression C-ABI library.

Imported Targets
^^^^^^^^^^^^^^^^
``TTZip::Core``
  The TTZip Core C-ABI library target.

Result Variables
^^^^^^^^^^^^^^^^
``TTZip_FOUND``
  True if TTZip was found.
``TTZip_INCLUDE_DIRS``
  Include directories containing ``ttzip.h``.
``TTZip_LIBRARIES``
  Libraries to link against.
``TTZip_VERSION``
  Version of the TTZip library found.
#]=======================================================================]

find_path(TTZip_INCLUDE_DIR
  NAMES ttzip.h
  PATHS
    /usr/local/include
    /opt/homebrew/include
    ${CMAKE_INSTALL_PREFIX}/include
)

find_library(TTZip_LIBRARY
  NAMES ttzip_glue ttzip TTZipVendor
  PATHS
    /usr/local/lib
    /opt/homebrew/lib
    ${CMAKE_INSTALL_PREFIX}/lib
)

include(FindPackageHandleStandardArgs)
find_package_handle_standard_args(TTZip
  REQUIRED_VARS TTZip_LIBRARY TTZip_INCLUDE_DIR
  VERSION_VAR TTZip_VERSION
)

if(TTZip_FOUND AND NOT TARGET TTZip::Core)
  add_library(TTZip::Core UNKNOWN IMPORTED)
  set_target_properties(TTZip::Core PROPERTIES
    IMPORTED_LOCATION "${TTZip_LIBRARY}"
    INTERFACE_INCLUDE_DIRECTORIES "${TTZip_INCLUDE_DIR}"
  )
endif()
