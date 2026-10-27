# Phase 1 Validation Quickstart: libarchive Disk Space Pre-allocation

> **Purpose**: Executable validation scenarios with command, expected output, and failure diagnostics.

---

## Scenario 1: Standalone libarchive CMake Build with `ARCHIVE_EXTRACT_PREALLOCATE`

### Command
```bash
cd /Users/kevintung/Documents/dev/TTZip/Vendor/libarchive-upstream
mkdir -p build_test && cd build_test
cmake .. -DENABLE_TEST=ON -DENABLE_TAR=ON
make -j$(sysctl -n hw.ncpu)
```

### Expected Output
```
[100%] Built target libarchive
[100%] Built target libarchive_test
```

### Failure Diagnostic
- If `HAVE_F_PREALLOCATE` or `HAVE_POSIX_FALLOCATE` fails to compile: Verify `<fcntl.h>` header inclusion in `archive_write_disk_posix.c`.
- If CMake errors on duplicate flags: Check `archive.h` bitmask conflict with `ARCHIVE_EXTRACT_SAFE_WRITES (0x40000)`.

---

## Scenario 2: Test Suite Execution for `archive_write_disk` & Pre-allocation

### Command
```bash
cd /Users/kevintung/Documents/dev/TTZip/Vendor/libarchive-upstream/build_test
ctest -R "test_write_disk" --output-on-failure
```

### Expected Output
```
100% tests passed, 0 tests failed out of 1
```

### Failure Diagnostic
- If test hangs on `ftruncate` or `fcntl`: Check if `fst.fst_posmode` is `F_PEOFPOSMODE` and `fst.fst_offset` is 0.
- If permission denied: Ensure test runs in a directory with standard write privileges.

---

## Scenario 3: TTZip Native Engine Integration & Regression Test

### Command
```bash
cd /Users/kevintung/Documents/dev/TTZip
swift test --filter ArchiveIntegrationTests
```

### Expected Output
```
Executed 18 tests, with 0 failures (0 unexpected)
```

### Failure Diagnostic
- If Swift bridge compilation fails: Verify `archive.h` in `CTTZipBridge` matches `Vendor/libarchive-upstream/libarchive/archive.h`.
