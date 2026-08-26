# Feature Specification: 046-codebase-standards-and-pal-integration

**Feature Name**: `codebase-standards-and-pal-integration`  
**Status**: `Draft`  
**Target Milestone**: TTZip Core Engine Modernization & Universal PAL Integration  
**Created**: 2026-08-17  

---

## 一、 业务动机与第一性原理 (Context & First Principles)

在完成 `Feature 045` 平台抽象层 (PAL) 的基础架构搭建之后，我们需要将整个核心业务层与安全层全面接入 PAL，彻底根除历史遗留代码中的系统调用异味、非安全路径拼接与非对称内存管理：

1. **安全中枢与路径清洗全面收敛 (Universal Path & Security Standardization)**：
   - 将 `SecurityScanner.swift` 与各格式解包器（`ArchiveExtractor`, `TarArchiveEngineTemplate`, `ZipParallelExtractor`）全面接入 `PlatformPathSanitizer`。
   - 彻底防范跨平台 Zip Slip 目录穿越、Windows 保留设备名（`CON`, `PRN`, `AUX`）系统死锁、NTFS 冒号流（ADS）木马注入以及 Unicode NFD/NFC 编码分歧。
2. **密码库与敏感内存物理擦除加固 (Cryptographic Memory Hardening)**：
   - 将 `PasswordVaultManager.swift`、`ArchivePasswordStore.swift` 与 `ZipCryptoEngine.swift` 中的密钥清零逻辑统一迁移至 `PlatformMemory.secureZero`，物理免疫 Clang/LLVM 编译器死存储消除 (Dead Store Elimination, DSE)。
3. **异构硬件感知调度器跨平台统一 (Cross-Platform Hardware Tuner)**：
   - 升级 `AppleSiliconTuner.swift` 与 `HardwareCalibrator.swift`，基于 `PlatformHardware` 提供平滑的跨平台 CPU 拓扑识别与线程池推演，在 macOS 保持 P-Core/E-Core QoS 提权的同时，在非 Darwin 平台提供对称的线程调度。
4. **C 桥接层内存分配与符号规范化 (C Bridge Harmonization)**：
   - 确保 `CTTZipBridge` 下所有物理分配遵循 `ttzip_platform_aligned_alloc` / `ttzip_platform_aligned_free` 对称契约。

---

## 二、 用户故事 (User Stories)

### User Story 1 (P1): 安全扫描器与解包管道全面接入 PAL 路径规范化
作为安全审计员，我希望所有归档解包、浏览与路径校验统一由 `PlatformPathSanitizer` 过滤，确保无论解包何种恶意构造包，均能自动中和 Zip Slip、DOS 保留名与 ADS 冒号流。

- **Acceptance Criteria**:
  - `SecurityScanner.sanitizePath` 委托给 `PlatformPathSanitizer`。
  - `ArchiveExtractor` 与 `TarNativeEngine` 解包落盘前无条件经过 `PlatformPathSanitizer` 清洗。
  - 30+ 种恶意路径测试 100% 拦截。

### User Story 2 (P1): 密码存储与加密引擎全面接入 PAL 防 DSE 安全物理擦除
作为密码学安全工程师，我希望密码与密钥在释放前调用 `PlatformMemory.secureZero`，杜绝编译器在 Release 优化阶段将清零指令作为 Dead Store 消除。

- **Acceptance Criteria**:
  - `PasswordVaultManager` 与 `ArchivePasswordStore` 密钥缓冲区销毁时统一调用 `PlatformMemory.secureZero`。
  - 密码库 v4 单元测试全量绿灯。

### User Story 3 (P2): 硬件调度器与核心编解码管道跨平台平滑演进
作为架构师，我希望 `AppleSiliconTuner` 与硬件推演引擎基于 `PlatformHardware` 与 `PlatformOperatingSystem` 进行策略分发，杜绝在非 macOS 环境下产生 `sysctlbyname` 崩溃。

- **Acceptance Criteria**:
  - `AppleSiliconTuner` 提供跨平台安全回退，非 macOS 平台自动返回标准 CPU 核心拓扑。
  - 保持全格式 46 项基准吞吐硬门禁，零性能倒退。

---

## 三、 功能需求 (Functional Requirements)

1. **FR001**: 必须重构 `SecurityScanner.swift`，使用 `PlatformPathSanitizer` 替代历史手写字符串正则匹配。
2. **FR002**: 必须将 `PasswordVaultManager.swift` 与 `ArchivePasswordStore.swift` 接入 `PlatformMemory.secureZero`。
3. **FR003**: 必须重构 `AppleSiliconTuner.swift`，接入 `PlatformHardware` 与 `PlatformOperatingSystem`。
4. **FR004**: 必须在 C 桥接层统一引入 `ttzip_platform.h`，确保页对齐内存严格对称分配与释放。
5. **FR005**: 必须保持本地 CI 流水线 100% 绿灯且硬性能门禁零倒退。

---

## 四、 成功指标 (Success Criteria)

- **SC001**: 全代码库 100% 消除未经抽象的裸 POSIX 路径操作与 `memset_s` 直接调用。
- **SC002**: 全量 584+ 单元测试 100% 通过。
- **SC003**: 性能硬门禁全线超额达标，11s 本地 CI 流水线满分通过。
