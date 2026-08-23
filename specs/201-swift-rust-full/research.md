# Research & Architectural Decisions: 201-swift-to-rust-full-architecture-sinking

## Decision 1: Complete Sinking of All CLI Subcommands into Rust (`ttzip-tui`)
- **Decision**: Implement all 18 CLI subcommands (`list`, `extract`, `create`, `cat`, `check`, `comment`, `convert`, `delete`, `diff`, `hash`, `info`, `lock`, `tree`, `update`, `recover`, `repair`, `split`, `join`, `bench`, `doctor`) directly inside `rust/ttzip-tui/src/cli/` using `clap` derive macros, `indicatif` progress bars, and `serde_json` structured output.
- **Rationale**: Standalone Rust executable provides sub-5ms cold start, zero reliance on Apple Swift runtime, and true cross-platform capability.
- **Source**: `rust/ttzip-tui/src/cli/args.rs`, `Sources/TTZipCLI/CLICommand.swift`.

## Decision 2: Core Algorithmic & Memory Sinking to `ttzip-glue`
- **Decision**: Sinks CRC64 (PMULL SIMD), Reed-Solomon Cauchy FEC, VFS LZ4 cache pools, and ZipExtraField parsers into `rust/ttzip-glue`.
- **Rationale**: Eliminates Swift ARC allocation overhead on large files and directory scans, achieving maximum throughput.
- **Source**: `rust/ttzip-glue/src/crypto/`, `rust/ttzip-glue/src/vfs/`, `rust/ttzip-glue/src/security/`.

## Decision 3: Swift FFI Boundary as Authoritative C-ABI Mapping
- **Decision**: Keep `Sources/TTZipCore/Bridge/` and `Facades/` as thin wrappers calling `ttzip-glue` exported C-ABI functions.
- **Rationale**: Preserves 100% backward compatibility with SwiftUI view models while eliminating duplicate algorithm implementations.
- **Source**: `Sources/TTZipCore/Bridge/ArchiveEngineBridge.swift`, `Sources/TTZipCore/Facades/TTZipEngineFacade.swift`.

## Decision 4: Preserving 100% of `TTZipApp` in Swift
- **Decision**: The 144 files in `Sources/TTZipApp` remain strictly in Swift.
- **Rationale**: Direct access to AppKit, AVKit, QuickLookUI, Sparkle, and macOS system capabilities without fragile FFI layers.
- **Source**: `Sources/TTZipApp/`.
