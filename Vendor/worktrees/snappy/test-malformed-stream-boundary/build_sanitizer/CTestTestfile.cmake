# CMake generated Testfile for 
# Source directory: /Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/snappy/test-malformed-stream-boundary
# Build directory: /Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/snappy/test-malformed-stream-boundary/build_sanitizer
# 
# This file includes the relevant testing commands required for 
# testing this directory and lists subdirectories to be tested as well.
add_test(snappy_unittest "/Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/snappy/test-malformed-stream-boundary/build_sanitizer/snappy_unittest")
set_tests_properties(snappy_unittest PROPERTIES  WORKING_DIRECTORY "/Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/snappy/test-malformed-stream-boundary" _BACKTRACE_TRIPLES "/Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/snappy/test-malformed-stream-boundary/CMakeLists.txt;390;add_test;/Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/snappy/test-malformed-stream-boundary/CMakeLists.txt;0;")
subdirs("third_party/googletest")
