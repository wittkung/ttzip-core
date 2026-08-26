# Tasks: 011 Comprehensive Official Portal, Developer Manuals, and Multi-Language SDK Integration Guide

- **Feature Directory**: `specs/011-official-portal-and-developer-manuals`
- **Specification**: `specs/011-official-portal-and-developer-manuals/spec.md`
- **Plan**: `specs/011-official-portal-and-developer-manuals/plan.md`

---

## Phase 1: Foundations & Architecture Alignment

- [ ] T001 Define portal navigation matrix and metadata across all subpages in `specs/011-official-portal-and-developer-manuals/data-model.md`
- [ ] T002 Verify contracts compliance using `bash .specify/scripts/bash/lint-contracts.sh specs/011-official-portal-and-developer-manuals/contracts`

---

## Phase 2: Core Portal & Developer Manuals Implementation

- [ ] T003 [P] Build dedicated 8-language SDK Developer Center in `apple/site/sdk.html` with interactive tab switching and zero-subprocess code examples
- [ ] T004 [P] Build comprehensive Multi-Channel Distribution & Cryptographic Licensing Guide in `apple/site/licensing.html`
- [ ] T005 [P] Upgrade CLI & TUI Manual with streaming pipe workflows and format recipes in `apple/site/cli.html`
- [ ] T006 [P] Upgrade Performance Whitepaper with ARM64 PMULL CRC throughput and APFS clonefile benchmarks in `apple/site/performance.html`
- [ ] T007 [P] Upgrade 16-Format Ecosystem and capability matrix in `apple/site/formats.html`
- [ ] T008 [P] Update Homepage navigation bar, hero actions, and footer links in `apple/site/index.html`

---

## Phase 3: Synchronization & Multi-Directory Verification

- [ ] T009 Synchronize all portal web artifacts from `apple/site/` to `core/`, `apple/docs/`, and root `docs/`
- [ ] T010 Validate local static server rendering and interactive components via Python HTTP server
- [ ] T011 Commit and push to GitHub remote repositories and verify GitHub Pages deployment on `https://ttzip.app`
