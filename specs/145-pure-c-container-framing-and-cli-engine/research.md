# Research Findings: 145-pure-c-container-framing-and-cli-engine

## R001: Pure C CLI Design & Argument Parsing
- **Decision**: Implement a clean POSIX `getopt_long` / manual sub-command parser (`-c`, `-x`, `-l`, `-t`, `-b`, `-v`, `-h`) in `cli/main.c`.
- **Rationale**: Keeps CLI 100% self-contained in standard C11 without external dependencies, compiling cleanly on MSVC, GCC, and Clang.
- **Alternatives Considered**: Using GNU `argp` (non-portable to MSVC) or third-party C++ libraries (adds C++ runtime overhead).
- **Source**: `cli/main.c`, standard C11 library specifications.

## R002: x86_64 CPUID Runtime Probing
- **Decision**: Use `__cpuid` / `__cpuidex` on MSVC and `<cpuid.h>` `__get_cpuid` / inline assembly on GCC/Clang inside `ttzip_platform_detect.c`.
- **Rationale**: Provides cross-platform, branchless CPU feature flags (`TTZIP_CPU_SSE42`, `TTZIP_CPU_AVX2`, `TTZIP_CPU_AVX512`, `TTZIP_CPU_PCLMUL`, `TTZIP_CPU_AESNI`).
- **Alternatives Considered**: Static compile-time flags only (fails to run on older CPUs or misses optimizations on newer CPUs).
- **Source**: `Sources/CTTZipBridge/include/ttzip_platform.h`, `Sources/CTTZipBridge/ttzip_platform_detect.c`.
