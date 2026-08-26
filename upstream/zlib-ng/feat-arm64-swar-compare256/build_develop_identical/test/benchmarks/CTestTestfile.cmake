# CMake generated Testfile for 
# Source directory: /Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/test/benchmarks
# Build directory: /Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/build_develop_identical/test/benchmarks
# 
# This file includes the relevant testing commands required for 
# testing this directory and lists subdirectories to be tested as well.
add_test("benchmark_zlib" "/Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/build_develop_identical/test/benchmarks/benchmark_zlib" "--benchmark_min_time=0")
set_tests_properties("benchmark_zlib" PROPERTIES  _BACKTRACE_TRIPLES "/Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/test/benchmarks/CMakeLists.txt;99;add_test;/Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/test/benchmarks/CMakeLists.txt;0;")
subdirs("../../_deps/benchmark-build")
