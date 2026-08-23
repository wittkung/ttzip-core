### Title
`examples: fix -Wimplicit-int-float-conversion in frameCompress.c and bench_functions.c on macOS CI`

---

### Description

#### 1. Background & Failure Context
In the GitHub Actions `Cross Platform` workflow (`.github/workflows/cross-platform.yml`), the `macOS` job builds and runs comprehensive tests under strict compilation flags:
```yaml
CFLAGS="-O3 -Werror -Wconversion -Wno-sign-conversion" make -j test V=1
```

Under modern Clang / Apple Clang (LLVM 15+), compiling the `examples` directory triggers fatal `-Wimplicit-int-float-conversion` errors in `examples/frameCompress.c` and `examples/bench_functions.c`:

```text
CC cachedObjs/.../frameCompress.o
frameCompress.c:446:40: error: implicit conversion from 'const unsigned long long' to 'double' may lose precision [-Werror,-Wimplicit-int-float-conversion]
  446 |             (double)ret.size_out / ret.size_in * 100);
      |                                  ~ ~~~~^~~~~~~
1 error generated.

CC cachedObjs/.../bench_functions.o
bench_functions.c:356:255: error: implicit conversion from 'uint64_t' (aka 'unsigned long long') to 'double' may lose precision [-Werror,-Wimplicit-int-float-conversion]
  356 | ... (double)time_taken__default * 100 / time_taken__default);
      |                                       ~ ^~~~~~~~~~~~~~~~~~~
(and 9 other identical warnings in bench_functions.c)
```

#### 2. Root Cause Analysis
In both `examples/frameCompress.c` (line 446) and `examples/bench_functions.c` (lines 356-368), 64-bit unsigned integers (`ret.size_in` and `time_taken_*`) are used as divisors in floating-point operations. Because IEEE 754 `double` has 53 bits of precision, Clang raises `-Wimplicit-int-float-conversion` under `-Wconversion -Werror` to protect against potential precision loss.

#### 3. Steps to Reproduce
Run on macOS (or any system with modern Clang):
```bash
make -C examples clean
CFLAGS="-O3 -Werror -Wconversion -Wno-sign-conversion" make -C examples test
```

#### 4. Proposed Fix
Explicitly cast 64-bit integer divisors to `(double)` in `examples/frameCompress.c` and `examples/bench_functions.c`:
- `examples/frameCompress.c`: `(double)ret.size_out / (double)ret.size_in * 100`
- `examples/bench_functions.c`: `(double)time_taken * 100.0 / (double)time_taken_default`

#### 5. Real Physical Verification
Tested and verified on macOS Sonoma with Apple Clang:
- `make -C examples clean && CFLAGS="-O3 -Werror -Wconversion -Wno-sign-conversion" make -C examples test` (100% Passed, exit code 0).
- `CFLAGS="-O3 -Werror -Wconversion -Wno-sign-conversion" make test` (100% Passed across all suites, exit code 0).

I'd be glad to submit a quick PR with this fix if you'd like!
