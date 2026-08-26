# CMake generated Testfile for 
# Source directory: /Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/snappy/fix-darwin-universal-binary
# Build directory: /Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/snappy/fix-darwin-universal-binary/build_release
# 
# This file includes the relevant testing commands required for 
# testing this directory and lists subdirectories to be tested as well.
add_test(snappy_unittest "/Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/snappy/fix-darwin-universal-binary/build_release/snappy_unittest")
set_tests_properties(snappy_unittest PROPERTIES  WORKING_DIRECTORY "/Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/snappy/fix-darwin-universal-binary" _BACKTRACE_TRIPLES "/Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/snappy/fix-darwin-universal-binary/CMakeLists.txt;390;add_test;/Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/snappy/fix-darwin-universal-binary/CMakeLists.txt;0;")
subdirs("third_party/googletest")
