# Implementation Plan: 030-libarchive-optimizations-integration

**Feature Name**: 将 libarchive 官方 PR (PR #3388) 优化成果与 7z AES-256 解密引擎集成至 TTZip (030-libarchive-optimizations-integration)  
**Status**: PLANNED  
**Branch**: `feat/030-libarchive-optimizations-integration`  
**Dependencies**: `Vendor/libarchive-upstream/build/bin/libarchive.a`, `Sources/CTTZipBridge/`, `Tests/TTZipTests/`  

---

## 1. Technical Context & Baseline

- **当前现状**:
  1. TTZip 的 `Vendor/lib/libarchive.a` 为旧版，不支持 7z AES-256 加密归档解密与 `kEncodedHeader` 全头加密。
  2. 原生 CTTZipBridge 内部的 `ttzip_7z_kdf_arm64.c` 虽然具备 NEON/CommonCrypto 加速，但单次派生包含 2 次 `malloc`/`free` 堆分配，且依赖全局互斥锁 `s_kdf_cache_lock`。
  3. 我们向上游贡献的 PR #3388（`7zip: add AES-256-SHA-256 decryption support`）已在 `Vendor/libarchive-upstream` 验证通过并产出最新静态库。
- **改动范围**:
  1. `Vendor/lib/libarchive.a`: 替换为包含完整 7z AES 解密的最新静态库。
  2. `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c`: 重构为栈上 `kdf_buf[536]` 零堆分配与无锁重入设计。
  3. `Tests/TTZipTests/Libarchive7zEncryptionTests.swift`: 新增端到端双引擎 7z AES 解密测试套件。

---

## 2. Constitution Check

- [x] **In-Process 纯 C 绑定准则**: 100% 静态库链接与 C 桥接，零外部 CLI 进程调用。
- [x] **热路径零成本抽象**: 原生 KDF 消除 `malloc`/`free`，采用 $O(1)$ 栈内存，严禁在热路径加锁。
- [x] **冻结文件保护**: 严禁修改 `ZipParallelExtractor.swift` 等 9 个冻结文件。
- [x] **零裸打印日志纪律**: C 桥接层与测试层零裸 `printf`/`print`，统一使用 `TTLogger`。
- [x] **性能门禁与零倒退**: 满足 `testSevenZipKdf_HardwareAcceleration_DurationFloor`（$\le 17\text{ ms}$ Debug / $\le 15\text{ ms}$ Release）与全格式 46 场景基准零倒退。

---

## 3. Phase 0: Research Findings

- R001 [SUBAGENT:research] 《Vendor 静态库替换与符号 ABI 兼容性》：详见 `research.md`，替换 `Vendor/lib/libarchive.a`，保留统一 `Vendor/include/` 头文件，验证 `_decrypto_aes_cbc_*` 与 `_kdf_7z_sha256` 符号完备。
- R002 [SUBAGENT:research] 《TTZip 原生 KDF 栈内存与无锁优化》：详见 `research.md`，采用栈上固定缓冲区 `kdf_buf[536]`，消除堆分配与全局互斥锁，原地 64 位计数器递增，退出时 `memset_s` 洗消。
- R003 [SUBAGENT:research] 《7z 加密测试用例与双引擎验证方案》：详见 `research.md`，复用 `Tests/TTZipTests/Fixtures/Encrypted/` 中的 3 个标准 7z 语料，建立双引擎端到端解密与容错断言。

---

## 4. Phase 1: Design & Contract Index

- **Data Model**: `data-model.md`（定义 `ttzip_7z_crypto_session_t`、`ArchiveEntryEncryptedMetadata`、`ArchiveExtractionResult`）
- **Contracts**:
  - `contracts/7z-kdf-session.json`
  - `contracts/archive-inspection-response.json`
  - `contracts/archive-extraction-response.json`
- **Validation Guide**: `quickstart.md`

---

## 5. Component Breakdown & Execution Strategy

### Component 1: Vendor Infrastructure Upgrade
- **[MODIFY]** `Vendor/lib/libarchive.a`: 替换为 `Vendor/libarchive-upstream/build/bin/libarchive.a`。
- **Verification**: `nm Vendor/lib/libarchive.a | grep decrypto_aes_cbc`。

### Component 2: Native CTTZipBridge KDF Stack Optimization
- **[MODIFY]** `Sources/CTTZipBridge/ttzip_7z_kdf_arm64.c`: 重构为 `kdf_buf[536]` 栈模式，移除 `malloc`/`free` 与全局互斥锁。
- **Verification**: `swift test --filter XCTestPerformanceMeasureTests/testSevenZipKdf_HardwareAcceleration_DurationFloor`。

### Component 3: Test Verification Suite & Benchmark Protection
- **[NEW]** `Tests/TTZipTests/Libarchive7zEncryptionTests.swift`: 双引擎 7z 加密（数据加密、头加密、混合条目）解密测试。
- **Verification**: `swift test` & `python3 scripts/audit_performance_regression.py`。
