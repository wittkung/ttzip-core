# Implementation Plan: 011 Comprehensive Official Portal, Developer Manuals, and Multi-Language SDK Integration Guide

- **Feature Directory**: `specs/011-official-portal-and-developer-manuals`
- **Type**: `[Full SDD]`
- **Status**: `Draft`
- **Created**: 2026-08-25

---

## 1. Technical Context

- **Platform**: Web Static Architecture (HTML5 / CSS3 / ES6 Canvas / HSTS `.app` Domain)
- **Deployment**: GitHub Pages (`main` branch, `/docs` directory) linked to `ttzip.app` via Namecheap Anycast DNS
- **Theme**: Zen Minimalist, Kintsugi Gold Micro-Rules, WSJ Typography, 3-Orb Fluid Dynamic Canvas
- **Subsystems**:
  - `index.html`: Portal Landing Page & Interactive 3-Column Miller Column Simulator
  - `sdk.html` *(New)*: Multi-Language SDK Developer Center (8 Ecosystems: C/C++, Rust, Python, Go, Java/Kotlin, C#, Dart, Swift)
  - `cli.html`: Command-Line Interface & TUI Power User Manual
  - `performance.html`: Silesia Benchmark & Microarchitecture Whitepaper
  - `formats.html`: 16-Format Ecosystem, RFC Standards, and Capability Matrix
  - `licensing.html` *(New)*: Multi-Channel Distribution, Ed25519 Cryptographic Verification & EULA
  - `privacy.html`: Zero Data Collection Legal Policy
  - `terms.html`: Service & License Agreement
  - `support.html`: Customer Support & FAQ Center

---

## 2. Constitution Checks & Guardrails

| Constitution Principle | Status | Compliance Verification Method |
| :--- | :--- | :--- |
| **Zero-Subprocess Policy** | ✅ PASS | All SDK code snippets in `sdk.html` demonstrate direct native C-ABI binding and in-process execution. |
| **Single-File LOC ($\le 800$)** | ✅ PASS | Codebase files strictly bounded; HTML documentation files modularized across clear subpages. |
| **Zero In-Tree Path Invariant** | ✅ PASS | All SDK install commands use standard package managers (`pip`, `cargo`, `go get`, CMake `find_package`). |
| **Living & Executable Examples** | ✅ PASS | Code snippets in `sdk.html` align with existing tests in `core/tests/` and `examples/`. |
| **Transparent Packaging** | ✅ PASS | Artifacts synchronized across `apple/site/`, `apple/docs/`, `core/`, and root `docs/`. |

---

## 3. Work Breakdown Structure (Phases)

### Phase 1: Specifications & Contracts
- **T001**: Generate `data-model.md` defining portal sitemap, routing entities, SDK language descriptors, and interactive states.
- **T002**: Generate `contracts/portal_routes_contract.json` & `contracts/sdk_matrix_contract.json`.
- **T003**: Generate `quickstart.md` for local web server validation and live browser checks.

### Phase 2: Portal Development & Interactive Refinement
- **T004**: Create `apple/site/sdk.html` (Dedicated 8-language SDK integration hub with tab switching and code copy).
- **T005**: Create `apple/site/licensing.html` (Comprehensive multi-channel pricing, Ed25519 offline licensing guide, and corporate procurement).
- **T006**: Update `apple/site/index.html` with direct navigation to the new SDK Center and Licensing Guide.
- **T007**: Refine `apple/site/cli.html`, `apple/site/performance.html`, `apple/site/formats.html`, `apple/site/privacy.html`, `apple/site/terms.html`, and `apple/site/support.html`.

### Phase 3: Multi-Directory Synchronization & Deployment
- **T008**: Synchronize updated web assets across `core/`, `apple/site/`, `apple/docs/`, and root `docs/`.
- **T009**: Commit and push changes to GitHub, verifying CI gate and GitHub Pages automated deployment on `https://ttzip.app`.
