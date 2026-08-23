# Implementation Plan: 7Z Final Two 500MB Conquest

**Feature Branch**: `007-7z-final-two-500mb-conquest`
**Feature Spec**: `specs/007-7z-final-two-500mb-conquest/spec.md`

## Summary

通过在 `Sources/CTTZipBridge/ttzip_lzma2_enc_native.c` 与 `ttzip_lzma2_fast_encoder.c` 中针对 500MB 大流实施：
1. 24 块（`p_cores * 2`，20.8MB 对齐）多核并发流水线；
2. 极简 HC3 快速哈希与深度 1 的直接短路搜索；
3. 原地流式 AES-256 NEON 加密与统一内存零拷贝；
将 500MB Level 1 压缩吞吐推升至 **$\ge 5,600\text{ MB/s}$**，实现 7Z 竞品对决 **32 战 32 胜（100% 全胜统治）**。

## Phase Breakdown

- **Phase 0 (Research)**: 调研 7-Zip `CLzma2Enc` 多流切分机制与 NEON AES 原地加密（`research.md` 已完成）。
- **Phase 1 (Design)**: 确定多核块流契约与数据模型（`data-model.md`, `contracts/`, `quickstart.md` 已完成）。
- **Phase 2 (Implementation)**: 调优 `ttzip_lzma2_enc_native.c` 与 `ttzip_lzma2_fast_encoder.c`。
- **Phase 3 (Verification)**: 运行全量 46 项基准与 32 项 7Z 对决，执行零倒退审计。
