# Feature Specification: 011 Comprehensive Official Portal, Developer Manuals, and Multi-Language SDK Integration Guide

- **Feature Directory**: `specs/011-official-portal-and-developer-manuals`
- **Type**: `[Full SDD]`
- **Status**: `Draft / Ready for Planning`
- **Created**: 2026-08-25
- **Target Release**: TTZip Portal v1.0.0 (`ttzip.app`)

---

## 1. Context & Business Intent

TTZip has evolved from a macOS native application into a unified ecosystem encompassing:
1. **Native macOS Desktop Application** (Swift 6, Miller Columns, Quick Look, App Store & Direct DMG);
2. **High-Performance Core Engine** (C-ABI 2.0, ARM64 NEON/PMULL, Rust Microkernel, APFS Clonefile);
3. **Multi-Language SDK Tier-1 Matrix** (8 languages: C/C++, Rust, Python, Go, Java/Kotlin, C#/.NET, Dart/Flutter, Swift);
4. **Command-Line & TUI Power Tool** (`ttzip-cli`, Homebrew);
5. **Cryptographic Multi-Channel Distribution & Licensing** (Ed25519, Free Community, ¥28 MAS, ¥28 Steam, ¥29 Direct).

To establish an authoritative, transparent, and developer-friendly global brand, `ttzip.app` must serve as the single source of truth. It must offer comprehensive, executable documentation, language-specific SDK onboarding guides, performance benchmarks, format compatibility matrices, and commercial licensing specifications while strictly preserving the **Zen aesthetic, Kintsugi Gold line work, and mathematical fluid background (`TTZipFluidBackgroundView.swift`)**.

---

## 2. User Stories & Acceptance Criteria

### User Story 1: Developer SDK Fast-Track Integration (开发者多语言 SDK 极速接入)
**As a** backend/systems developer or mobile engineer,  
**I want to** navigate to `ttzip.app/sdk.html` (or Developer Center), select my programming language (C/C++, Rust, Python, Go, Java/Kotlin, C#, Dart, Swift), and copy the exact package manager command and a 10-line runnable snippet,  
**So that** I can integrate in-process zero-subprocess archive capabilities into my project within 60 seconds without searching through Git trees.

- **AC 1.1**: Provide instant tab switching between 8 official language ecosystems.
- **AC 1.2**: Display official package manager installation instructions (e.g. `pip install ttzip`, `go get`, `cargo add`, Gradle, CMake `find_package(ttzip)`).
- **AC 1.3**: Provide copyable, syntax-highlighted minimal working examples for each language with zero subprocess dependencies.

### User Story 2: Power-User & CLI Automation Manual (终端运维与命令行手册)
**As a** DevOps engineer or CLI power user,  
**I want to** access a comprehensive CLI & TUI documentation hub (`ttzip.app/cli.html`),  
**So that** I can understand streaming pipes (`stdin`/`stdout`), batch operations, regex filtering, exit codes, and hardware benchmark arguments.

- **AC 2.1**: Full flag, option, and subcommand dictionary (`compress`, `extract`, `list`, `inspect`, `bench`, `vault`).
- **AC 2.2**: Practical recipes (e.g., APFS instant clone, memory-safe AES-256 encryption, Windows CJK encoding conversion).

### User Story 3: Architecture & Performance Whitepaper (架构深度拆解与性能白皮书)
**As an** enterprise architect or security auditor,  
**I want to** inspect the verifiable technical whitepaper (`ttzip.app/performance.html`),  
**So that** I can review the ARM64 PMULL CRC vector throughput (48GB/s), 16KB physical page alignment, APFS zero-copy clonefile architecture, and zeroization memory security.

- **AC 3.1**: Include interactive benchmark tabs across small files, large streams, text logs, and multi-threaded scaling.
- **AC 3.2**: Clear explanation of C-ABI 2.0, memory zeroization, and sandbox compliance.

### User Story 4: Transparent Licensing & Format Matrix (全格式矩阵与多轨授权中心)
**As a** corporate customer or open-source contributor,  
**I want to** view the 16-format compatibility table (`ttzip.app/formats.html`) and licensing guidelines (`ttzip.app/licensing.html`),  
**So that** I understand format capabilities (compression/decompression/encryption/splitting) and transparent distribution options (Community vs Direct vs MAS vs Steam).

- **AC 4.1**: 16-format matrix detailing algorithm, RFC standard, max compression level, encryption support, and clonefile compatibility.
- **AC 4.2**: Detailed offline Ed25519 licensing verification explanation and EULA terms.

---

## 3. Non-Functional & Design Requirements

1. **Design System Adherence**:
   - 100% mathematical fidelity to `TTZipFluidBackgroundView.swift` (3-orb harmonic orbit canvas, phase speed 0.3, Bamboo Green `#10B981` / `#2E8B57`, Dark mode `#34C759`).
   - WSJ editorial typography, Kintsugi Gold accents (`#D4AF37` / `#E5C158`), and translucent glass cards.
2. **Performance & Zero Cost**:
   - Zero external heavy frameworks (pure vanilla HTML5, CSS3, ES6+).
   - Instant page loads ($< 100\text{ms}$ DOMContentLoaded).
   - Global CDN deployment on GitHub Pages with HSTS on `ttzip.app`.
3. **Responsive & Accessible**:
   - 100% mobile, tablet, and widescreen desktop responsive.
   - Dark/Light mode automatic adaptation via `prefers-color-scheme`.
