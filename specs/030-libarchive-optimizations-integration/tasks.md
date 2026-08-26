# Tasks: 030-libarchive-optimizations-integration

**Feature**: 将 libarchive 官方 PR (PR #3388) 优化成果与 7z AES-256 解密引擎集成至 TTZip  
**Spec**: `specs/030-libarchive-optimizations-integration/spec.md`  
**Plan**: `specs/030-libarchive-optimizations-integration/plan.md`  

---

## Phase 1: Setup & Vendor Infrastructure Upgrade

- [x] T001 [P] [US1] 复制最新静态库 `Vendor/libarchive-upstream/build/bin/libarchive.a` 覆盖 `Vendor/lib/libarchive.a`
- [x] T002 [P] [US1] 执行 `nm` 符号检查验证 `Vendor/lib/libarchive.a` 包含 `_decrypto_aes_cbc_*` 与 `_kdf_7z_sha256` 符号

---

## Phase 2: Native CTTZipBridge KDF Stack Optimization (User Story 3)

- [x] T003 [P] [US3] 重构 `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c`，使用 `kdf_buf[536]` 栈内存消除 `malloc`/`free` 堆分配并实现原地 64 位小端计数器递增
- [x] T004 [P] [US3] 移除 `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c` 中的 `s_kdf_cache_lock` 全局互斥锁，并在退出时增加敏感内存 `memset` 洗消

---

## Phase 3: Test Suite & End-to-End Encryption Verification (User Story 1 & 2)

- [x] T005 [P] [US1] 在 `Tests/TTZipTests/Libarchive7zEncryptionTests.swift` 中新增 7z AES 数据流解密、全头加密 (`kEncodedHeader`) 与混合条目双引擎解密测试
- [x] T006 [US2] 执行全量单元测试 `swift test` 验证全部测试通过且零回归

---

## Phase 4: Performance Floor & Zero-Regression Benchmark (User Story 3)

- [x] T007 [P] [US3] 执行 `swift test --filter XCTestPerformanceMeasureTests/testSevenZipKdf_HardwareAcceleration_DurationFloor` 验证 KDF 硬件加速耗时 $\le 17\text{ ms}$
- [x] T008 [US3] 执行 `TTZIP_RUN_BENCHMARKS=1 swift test -c release --filter AllFormatsPkSuiteTests` 与 `python3 scripts/audit_performance_regression.py` 验证全矩阵 46 项基准零性能倒退
