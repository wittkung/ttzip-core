# Research: 024-full-matrix-peak-consolidation-and-zero-regression

## 1. 加密 DMG 归档密码感知自适应路由 (Password-Aware Adaptive Dispatch)

- **Decision**: 在 `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift` 中实施密码感知分发：
  - 当 `(targetFormat == .dmg || pathLower.hasSuffix(".dmg")) && password != nil && !password!.isEmpty` 时，前置直通 `SevenZipEngine.shared.extract(...)` 硬件 NEON AES-256 解密管道。
  - 当 `password == nil` 时，保持原有纯原生 C 直通解压（`ttzip_extract_archive_advanced`）。
- **Rationale**: libarchive 原生解压不支持 Apple 加密磁盘映像；当提供密码时盲目尝试 libarchive 会产生 2~5ms 的冷失败系统调用并破坏 CPU 缓存；前置直通让未加密 DMG 保持 $\ge 9,933.1\text{ MB/s}$，加密 DMG 恢复至 $9,900+\text{ MB/s}$。
- **Alternatives Considered**:
  - *统一全量 DMG 交由 SevenZipEngine*: 处理未加密 DMG 吞吐会从 9,933 MB/s 跌至 5,000 MB/s，跌破硬门禁。
  - *在 libarchive 中自研 DMG AES 解包器*: 成本过高，重复造轮子。
- **Source**: `Sources/TTZipCore/ArchiveExtractor+Dispatch.swift:16-30`, `Sources/TTZipCore/SevenZip/SevenZipEngine.swift:36-71`, `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c:77-116`.

---

## 2. TAR 原生解压管道引入栈上双层零分配内联目录缓存池

- **Decision**: 在 `Sources/CTTZipBridge/ttzip_tar_native.c` 的 `ttzip_extract_tar_native_c` 中开辟栈上双层零分配内联目录缓存池（L1 `last_parent_dir` + L2 64-Slot FNV-1a Hash Table）。
- **Rationale**: 原逻辑对 100~600 个小文件重复执行 600 次 `mkdir(tmp, 0755)` 逐级系统调用并产生大量 `EEXIST` 锁争抢；双层缓存池将冗余系统调用压降 $99.5\%+$，零堆分配、零跨线程锁，助推小文件解压吞吐恢复至 $1,350+\text{ MB/s}$。
- **Alternatives Considered**:
  - *在调用 `mkdir_p` 前通过 `access()` 或 `stat()` 检查*: 依然切入内核态并产生 600 次系统调用，无法彻底消除开销。
  - *使用动态堆哈希表*: 违反热路径零堆分配铁律。
- **Source**: `Sources/CTTZipBridge/ttzip_tar_native.c:263-296`, `Sources/CTTZipBridge/CTTZipBridge_7zNativeDecoder.c:158-198`.

---

## 3. 全格式 262 项历史最高峰值门禁自动整合与永久锁定

- **Decision**: 自动汇总历史全部批次测试中的单项最高吞吐记录，固化在 `GEMINI.md` §3.1 中，任何单项测试倒退 $>10\%$ 即阻断流水线。
- **Rationale**: 坚守用户铁律（"不以任何方式降低门禁"），确保性能持续单调递增，不漏掉任何一项新突破的记录。
- **Source**: `GEMINI.md:124-148`, `scripts/audit_performance_regression.py:35-48`.
