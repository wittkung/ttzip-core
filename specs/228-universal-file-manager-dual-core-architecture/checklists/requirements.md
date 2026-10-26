# Specification Quality Checklist: Universal File Manager & Dual-Core Architecture

**Feature Directory**: `products/ttzip/core/specs/228-universal-file-manager-dual-core-architecture`  
**Purpose**: Validate specification completeness, physical testability, and architectural compliance before proceeding to planning.

---

## 1. Content Completeness & Testability

- [x] **Clear Persona & User Journeys**: Covers archiver workflows, media/document previewers, Android device management, and plugin developers.
- [x] **Measurable Success Criteria**: Quantifiable targets established for memory (32MB), FPS (120 FPS), main-thread I/O wait (0.0ms), and lines of code purged (45,000+ LOC).
- [x] **Edge Cases & Error States Documented**: Handles corrupted archives, missing entries, device disconnects, certificate expiration, and permission violations explicitly.
- [x] **No Implementation Leakage in Requirements**: Functional requirements state *what* must be accomplished without binding to low-level internal struct names.

---

## 2. Monorepo Industrial Governance Compliance

- [x] **Single Source of Truth (SSOT)**: Enforces unified `ArchiveFormat` and single PKI certificate hierarchy.
- [x] **Root Neutrality**: Respects root directory neutrality; components placed strictly within `products/ttzip/` and `infra/ttmedia/`.
- [x] **Single-File LOC Gate**: Mandates all new/refactored files $\le 800$ LOC (target mean $\le 350$ LOC).
- [x] **Zero-Warning Hard Gate**: Enforces `-warnings-as-errors` across Rust (`clippy`) and Swift compilers.
- [x] **Google Piper Licensing Model**: All files retain proprietary headers; all synthetic Apache-2.0 mock test code purged.

---

## 3. Dual-Core & Kit Topology Integrity

- [x] **Decoupled Archive Core (`ttzip/core`)**: Retains only compression, decompression, VFS, and WAL in-place mutations.
- [x] **Superset Infrastructure Kit (`infra/ttmedia`)**: Inherits `infra/ttmpv`, exposes decoupled sub-engines, and enables zero-friction repository extraction.
- [x] **Cross-Platform Application Core (`ttzip/app-core`)**: Centralizes device communication (MTP/ADB), transfer engine, and plugin PKI validation across macOS, Windows, and Linux.
- [x] **Thin UI Client (`ttzip/apple`)**: Strictly presentation-focused, isolated from disk I/O, conforming to Apple HIG.
