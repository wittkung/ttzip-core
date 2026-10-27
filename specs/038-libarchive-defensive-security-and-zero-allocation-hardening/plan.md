# Implementation Plan: libarchive 级防御性安全与零分配热路径加固

**Feature Directory**: `specs/038-libarchive-defensive-security-and-zero-allocation-hardening`  
**Created**: 2026-08-16  
**Status**: In Progress  
**Spec**: [spec.md](file:///Users/kevintung/Documents/dev/TTZip/specs/038-libarchive-defensive-security-and-zero-allocation-hardening/spec.md)

---

## Technical Context

- **项目基线**: Swift 6.0 + macOS 14.0+ (Apple Silicon NEON) + C11 / POSIX。
- **改动范围**:
  1. `Sources/CTTZipBridge/CTTZipBridge_Archive.c`：解压选项补齐 `ARCHIVE_EXTRACT_SECURE_SYMLINKS` 与 `ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS`。
  2. `Sources/TTZipCore/SecurityScanner.swift`：增加 `sanitizePath` 路径清洗与 Zip Slip 深度拦截。
  3. `Sources/TTZipCore/Adapters/LibdeflateCAdapter.swift`：消除 `Data(count:)`，采用未初始化裸指针与 `bytesNoCopy`。
  4. `Sources/CTTZipBridge/CTTZipBridge_Crypto.c`：密码释放前强制调用 `memset_s`。

---

## Constitution Check

- [x] **Zero-Cost Abstraction**: 消除热路径隐式堆清零，加固防御不引入中间锁或动态对象树。
- [x] **No Subprocess**: 核心引擎坚持 100% In-process C 绑定。
- [x] **Zero Bare Logging**: 日志遵循 TTLogger 规范。
- [x] **Frozen Subsystems**: 零侵入已冻结的 ZIP 核心引擎代码。

---

## Phase 0: Research Items

- R001: 《Swift 未初始化内存管理与 `Data(bytesNoCopy:...)` 零拷贝模式》：研究如何使用 `UnsafeMutablePointer<UInt8>.allocate` 配合 `.custom` deallocator，消除内核零填充开销。
- R002: 《`archive_write_disk` 安全标志位与路径规整算法》：研究 `ARCHIVE_EXTRACT_SECURE_SYMLINKS` 与 `ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS` 在 macOS 平台下的行为特性。

---

## Phase 1: Artifacts & Contracts

- `data-model.md`: 定义 `HardenedSecuritySpec` 与 `ZeroAllocationConfig` 数据模型。
- `contracts/hardening_spec.json`: 格式化加固验证契约。
- `quickstart.md`: 安全防御与性能自测指南。

---

## Phase 2: Implementation Checklist

### 1. 解压管道安全与 Zip Slip 纵深防御
- [ ] 在 `Sources/CTTZipBridge/CTTZipBridge_Archive.c` 中配置全量安全标志位
- [ ] 在 `Sources/TTZipCore/SecurityScanner.swift` 中实现 `sanitizePath` 与符号链接/路径穿越检测

### 2. 热路径零分配优化
- [ ] 重构 `Sources/TTZipCore/Adapters/LibdeflateCAdapter.swift` 中的 `compressData` 与 `decompressData`

### 3. 密码内存安全清零
- [ ] 在 `Sources/CTTZipBridge/CTTZipBridge_Crypto.c` 中添加 `memset_s` 敏感凭据清零

### 4. 验证与回归
- [ ] 运行全量单元测试与性能门禁
