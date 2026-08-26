# Quickstart & Verification Guide: XZ PR #241 Remediation

**Feature**: `specs/214-xz-arm64-crc64-remediation`  
**Target Repository**: `Vendor/worktrees/xz/pr2-arm64-crc64`

---

## 1. Prerequisites

- CMake 3.14+ or GNU Autotools (autoconf, automake, libtool)
- Clang / Apple Clang or GCC targeting ARM64
- macOS Apple Silicon or Linux AArch64 machine

---

## 2. CMake Validation Workflow

```bash
cd /Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/xz/pr2-arm64-crc64

# 1. Clean Build Directory
rm -rf build-cmake && mkdir build-cmake

# 2. Configure with Full PMULL CRC64 Support
cmake -B build-cmake -S . \
  -DCMAKE_BUILD_TYPE=Release \
  -DXZ_ARM64_CRC64=ON \
  -DBUILD_TESTING=ON

# 3. Compile
cmake --build build-cmake -j8

# 4. Execute Unit Tests
ctest --test-dir build-cmake --output-on-failure
```

---

## 3. Autotools Validation Workflow

```bash
cd /Users/kevintung/Documents/dev/TTZip/Vendor/worktrees/xz/pr2-arm64-crc64

# 1. Regenerate Configure Scripts
./autogen.sh

# 2. Configure with Debug/Sanitizers
./configure --enable-debug CFLAGS="-O3 -g -Wall -Wextra -Werror"

# 3. Compile and Run Test Suite
make -j8
make check
```

---

## 4. Standalone Micro & Correctness Verification

```bash
# Run isolated test_check binary directly
./build-cmake/tests/test_check
```
**Expected Output**:
```
All CRC32 and CRC64 tests passed.
```
