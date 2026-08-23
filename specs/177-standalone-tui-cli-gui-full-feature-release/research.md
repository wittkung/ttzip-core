# Phase 0 Research: 177-standalone-tui-cli-gui-full-feature-release

## Research Item R001: Standalone CLI Subcommands & Ratatui Interactive Architecture
- **Decision**: Extend `rust/ttzip-tui/src/cli/` with `recover`, `repair`, `split`, `join`, and `--volume-size` / Snappy / Brotli support, backed by high-performance `ttzip-glue` engines.
- **Rationale**: 
  - Exposes the full power of Rust core (in-memory multi-core password dictionary recovery >150k keys/s, NEON SIMD damaged archive repair, stream splitting) in a standalone, zero-external-dependency terminal binary (`bin/ttzip`).
  - Transparent volume chain detection (`detect_volume_chain`) allows users to inspect or unpack split archives with a single command.
- **Alternatives Considered**: 
  - *Keep advanced features exclusive to Swift GUI*: Prevents headless server and cross-platform terminal use.
  - *External toolchains (e.g. john/hashcat)*: Breaks zero-dependency single-binary architecture.
- **Source**: 
  - `rust/ttzip-tui/src/cli/args.rs:L1-120`
  - `rust/ttzip-tui/src/cli/handlers.rs:L1-286`
  - `rust/ttzip-glue/src/crypto/recovery.rs:L1-141`
  - `rust/ttzip-glue/src/archive/repair.rs:L1-268`

---

## Research Item R002: Terminal 2D Pareto Scatter & Andrew Convex Hull Braille Plotter
- **Decision**: Implement `TerminalBrailleCanvas` using Unicode 8-dot Braille characters (`0x2800`..`0x28FF`) providing $2 \times 4$ subpixel resolution per character cell with $\log_{10}$ X-axis projection and Bresenham line drawing in `rust/ttzip-tui/src/cli/braille_plotter.rs`.
- **Rationale**: 
  - Braille characters scale terminal resolution by $8\times$, rendering clear non-dominated Pareto frontiers and Andrew's Upper Convex Hull lines without external graphics protocols (Sixel/Kitty).
  - Works seamlessly across all standard terminals, SSH sessions, and CI text logs.
- **Alternatives Considered**: 
  - *Sixel / Kitty graphics*: Incompatible with standard Terminal.app, CI runners, and text reports.
  - *Plain text tables only*: Obscures non-linear trade-offs between throughput and compression ratio.
- **Source**: 
  - `rust/ttzip-glue/src/bench/pareto.rs:L1-209`
  - `rust/ttzip-glue/src/bench/mips.rs:L1-228`
  - Unicode Standard 15.0 Chapter 21.1 "Braille Patterns"

---

## Research Item R003: SwiftUI macOS VFS 16-Way LZ4 Cache & QuickLook Early Termination
- **Decision**: Integrate Rust 16-way sharded `VFSLz4CachePool` with SwiftUI `ArchiveExplorerView` and implement early-termination stream decoding for 7z solid single-item QuickLook preview.
- **Rationale**: 
  - 16-way sharded cache eliminates lock contention during fast scrolling and multi-thread prefetching.
  - 7z early termination stops stream decompression immediately after reaching target file end offset, cutting single-item preview latency from seconds to <10ms with 0 bytes written to disk.
- **Alternatives Considered**: 
  - *Full solid folder decompression*: Causes massive RAM spikes on 1GB+ blocks.
  - *Temporary disk extraction*: Severe disk I/O amplification and slow UI response.
- **Source**: 
  - `Sources/TTZipCore/VFS/VFSLz4CachePool.swift:L21-140`
  - `rust/ttzip-glue/src/sevenz/decoder/stream.rs:L20-96`
  - `Sources/TTZipCore/SevenZip/SevenZipSeekTable.swift:L84-127`

---

## Research Item R004: Local 0-Cloud Quota Release Packaging Pipeline
- **Decision**: Implement `./scripts/package_local_release.sh` to locally build universal static libraries, compile release binaries, assemble `TTZip.app`, package standalone CLI tarballs, generate Homebrew Formula, and produce SHA-256 checksums.
- **Rationale**: 
  - Completely eliminates dependence on GitHub Actions cloud quota, allowing 100% reproducible local releases on Apple Silicon and Intel macOS hardware.
- **Alternatives Considered**: 
  - *Manual multi-step execution*: Error-prone and lacks release verification consistency.
- **Source**: 
  - `scripts/build_rust.sh`
  - `scripts/package_cli_release.sh`
  - `scripts/run_local_ci_gate.sh`
