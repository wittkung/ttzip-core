# Research Findings: 149-cross-platform-cmake-and-c-sdk-quickstart

## R001: Cross-Platform CMake POSIX Linkage for Linux/Windows
- **Decision**: Update `CMakeLists.txt` to conditionally link `pthread`, `m`, `dl` under `UNIX AND NOT APPLE`, and use standard `find_package(ZLIB)` / `find_package(BZip2)` fallbacks.
- **Rationale**: Ensures standard Linux distributions (Debian, Ubuntu, Fedora, Alpine) build `libttzip.a` cleanly without missing system math or thread symbols.
- **Alternatives Considered**: Hardcoded `-lz -lbz2` (fails on custom sysroot or non-standard package managers).
- **Source**: CMake Official Documentation for `find_package(Threads)`.

## R002: Zero-Dependency C Quickstart API Architecture
- **Decision**: Provide `examples/quickstart.c` with zero external dependencies besides `stdio.h`, `stdlib.h`, `string.h`, and `ttzip_api.h`.
- **Rationale**: Demonstrates that external developers only need a single `#include <ttzip/ttzip_api.h>` to get all compression, archive, and VFS capabilities.
- **Alternatives Considered**: Multi-file example suite (more complex for a developer reading the repository for the first time).
- **Source**: `include/ttzip_api.h`.
