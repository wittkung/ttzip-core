# Quickstart & Verification Guide: TTZip CLI & Full Multi-Language SDK Evolution

- **Feature ID**: `005-cli-and-multi-language-sdk-architecture-evolution`
- **Pipeline Mode**: `[Full SDD]`
- **Status**: `READY_FOR_EXECUTION`
- **Created**: 2026-08-24

---

## 1. Overview

This guide details executable validation scenarios to verify that all architectural remediations and multi-language SDKs operate natively, zero-copy, and with bounded memory.

---

## 2. Verification Scenarios

### Scenario 1: CLI Bounded-Memory & Streaming Pipe Archiving
Prove that the CLI handles multi-gigabyte archives within $\le 64\text{MB}$ peak RSS and supports UNIX piping.

```bash
# 1. Pipe Stdin Archive Creation
tar cf - Sources | ./rust/target/release/ttzip create - -f 7z > /tmp/backup.7z

# 2. Extract Streaming to Stdout
./rust/target/release/ttzip extract /tmp/backup.7z -o /tmp/extracted_out

# 3. Verify Memory Bound on Large File (50GB virtual or physical)
/usr/bin/time -l ./rust/target/release/ttzip hash /tmp/large_50gb.tar
# Assert: peak resident set size <= 67108864 bytes (64MB)

# 4. Verify Unicode CJK / Emoji Listing
./rust/target/release/ttzip list tests/fixtures/cjk_and_emoji.zip
# Assert: Exits with 0 and correctly displays Chinese/Emoji names without panic
```

### Scenario 2: Canonical C-ABI 2.0 Universal Free & Error Out-Pointers
Prove that native FFI callers safely free allocations via `ttzip_free` and receive error descriptors without TLS.

```bash
# Run C-ABI 2.0 integration harness under ASan & TSan
cargo test -p ttzip-engine --test cabi_universal_free_asan
cargo test -p ttzip-engine --test cabi_thread_safe_error_diagnostics
```

### Scenario 3: Swift 6 Strict Concurrency & Actor Validation
Prove that `TTZipCore` builds cleanly under Swift 6 strict concurrency with zero stack overflows and zero task chain leaks.

```bash
cd core
# 1. Strict Concurrency Compilation
swift build -Xswiftc -strict-concurrency=complete

# 2. Non-Recursive Protocol Test
swift test --filter ArchiveProtocolDefaultRecursionTests

# 3. Memory Leak & Task Chaining Test (10,000 operations)
swift test --filter CommandHistoryManagerLeakTests
```

### Scenario 4: Python SDK 2.0 GIL-Free Multi-Threading & Buffer Protocol
Prove that multi-threaded Python compression scales linearly without GIL contention.

```bash
# Run pytest with multi-threaded benchmarks
pytest core/rust/ttzip-python/tests/test_gil_free_parallel.py -v
pytest core/rust/ttzip-python/tests/test_zstd_stream_rfc8878.py -v
```

### Scenario 5: Java 22+ Real Native FFM SDK (Zero Subprocess)
Prove that Java FFM executes directly against the C-ABI dynamic library without spawning CLI subprocesses.

```bash
cd core/sdk/jvm
# Run Java unit tests in a clean environment where 'ttzip' binary is not in PATH
mvn test
# Assert: All tests pass using java.lang.foreign.Arena and DowncallHandle
```

### Scenario 6: Dart / Flutter Real Native FFI SDK (Zero Subprocess)
Prove that Dart FFI executes natively in background Isolates.

```bash
cd core/sdk/dart
# Run Dart unit tests
dart test
# Assert: All tests pass using dart:ffi DynamicLibrary without Process.run
```

### Scenario 7: C++20 Header-Only RAII Library (`ttzip.hpp`)
Prove that C++20 developers can use RAII archive wrappers with `std::span` and `std::expected`.

```bash
cd core/sdk/cpp
clang++ -std=c++20 -I../../Sources/CTTZipBridge/include test_cpp_sdk.cpp -L../../rust/target/release -lttzip_glue -o test_cpp
./test_cpp
```

### Scenario 8: Contract Validation Gate
Ensure all JSON contract schemas strictly adhere to Spec Kit rules.

```bash
bash .specify/scripts/bash/lint-contracts.sh specs/005-cli-and-multi-language-sdk-architecture-evolution/contracts
```
