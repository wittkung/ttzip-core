# Implementation Plan: 149-cross-platform-cmake-and-c-sdk-quickstart

## Technical Context
- **Target Subsystems**: `CMakeLists.txt`, `examples/quickstart.c`, `scripts/local-ci.sh`.
- **Public ABI**: `Sources/CTTZipBridge/include/ttzip_api.h`.

## Constitution Check
- **Zero GPL-3**: Maintained.
- **Zero GCD**: Maintained.
- **0 Quota**: Maintained.

## Phase 0: Outline & Research
- - R001 [SUBAGENT:research] 《Cross-Platform CMake POSIX Linkage for Linux/Windows》: Dynamic vs static library detection for zlib, bzip2, pthreads.
- - R002 [SUBAGENT:research] 《Zero-Dependency C Quickstart API Architecture》: Minimalist 1-file C11 example showing all core SDK calls.

## Phase 1: Design & Contracts
- `contracts/quickstart-contract.json`
- `data-model.md`
- `quickstart.md`
