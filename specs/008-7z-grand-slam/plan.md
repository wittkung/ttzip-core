# Implementation Plan: 7Z Grand Slam Supremacy (32/32 All Conquest)

## 1. Plan Overview
This plan defines the end-to-end engineering tasks to optimize 500MB stream LZMA2 compression, eliminate medium-stream fragmentation, and achieve 32/32 (100% win rate) across all 7Z benchmark scenarios.

---

## 2. Phase 0: Research & Grounding
- [x] Survey USENIX FAST/ASPLOS 2024-2026 papers on asymmetric multicore compression.
- [x] Reverse-engineer 7-Zip LZMA SDK 24.x Level 1 execution engine (`CLzma2Enc.c` / `LzmaEnc.c`).
- [x] Document microarchitecture findings in `research.md`.

---

## 3. Phase 1: Architectural Design & Contracts
- [x] Define data entities and state machines in `data-model.md`.
- [x] Define C-Bridge pipeline interface in `contracts/7z_grand_slam_contract.md`.
- [x] Provide testing and validation workflow in `quickstart.md`.

---

## 4. Phase 2: User Story 1 - 500MB L1 Unencrypted Compression (Priority: P1)
- Optimize 500MB chunk size to dynamic 24-block alignment (20.8MB per block) matching Apple Silicon L2 cache and core count.
- In `ttzip_lzma2_fast_encoder.c`, tune `LZMA_MF_HC3` with `dict_size = 4096`, `nice_len = 273`, `depth = 1`.
- Verify throughput reaches $\ge 5,800\text{ MB/s}$.

---

## 5. Phase 3: User Story 2 - 500MB L1 AES-256 Compression (Priority: P2)
- Preserve in-place single-pass ARMv8 NEON AES-256 encryption pipeline.
- Verify throughput reaches $\ge 5,600\text{ MB/s}$.

---

## 6. Phase 4: User Story 3 - Grand Slam Conquest & Zero-Regression (Priority: P3)
- Clamp medium-stream (1MB~32MB) block sizes to $\ge 1\text{MB}$ minimum window.
- Execute full 1v1 PK benchmark suite (`AllFormatsPkSuiteTests`).
- Validate 32/32 (100% win rate) in 7Z.
- Execute regression audit script (`audit_performance_regression.py`).
- Run 11 performance gates and 560+ regression unit tests.
