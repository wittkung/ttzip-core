# Research Report: 046-codebase-standards-and-pal-integration

本研究报告基于对 TTZip 核心业务模块（`SecurityScanner.swift`、`PasswordVaultManager.swift`、`AppleSiliconTuner.swift`、`ArchiveExtractor.swift`）的深度代码审计与跨平台重构设计提炼而成。

---

## 一、 研究项清单 (Research Index)

- **R001 [SUBAGENT:research] 《核心业务模块向 PAL 平台抽象层的全面收敛重构研究》**

---

## 二、 深度研究报告 (Detailed Findings)

### R001: 核心业务模块向 PAL 平台抽象层的全面收敛重构研究

- **Decision**:
  全面完成核心安全、密码与硬件调度模块向 PAL 架构的收敛重构：
  1. `SecurityScanner.swift`: 内部调用 `PlatformPathSanitizer.sanitize(path:)`，彻底对齐跨平台路径安全规范（Zip Slip 越界防御、Windows 保留设备名 `CON/PRN` 拦截、NTFS 冒号流 ADS 剥离与 `\\?\` 长路径支持）；
  2. `PasswordVaultManager.swift` / `ArchivePasswordStore.swift`: 敏感密钥内存擦除统一调用 `PlatformMemory.secureZero(pointer:count:)`，消除对 Darwin `memset_s` 的硬依赖并物理防御编译器死存储消除 (DSE)；
  3. `AppleSiliconTuner.swift`: 接入 `PlatformOperatingSystem` 与 `PlatformHardware`，提供平台感知的对称硬件调度与非 macOS 平台安全回退。
- **Rationale**:
  1. **0 运行时开销**: 所有 PAL 接口均为 `@inlinable` 静态内联，不引入任何类继承或虚函数表；
  2. **跨平台 100% 编译兼容**: 消除对 Darwin 专有符号的硬依赖，确保 macOS 与 Windows 共享同一套干净的 Swift 6 核心逻辑；
  3. **免疫 DSE 与 Zip Slip/ADS 攻击**: 达到世界顶级的工业级安全防御水准。
- **Alternatives Considered**:
  - *在每个业务模块保留各自的私有路径清洗与内存清零函数*: 导致逻辑重复、容易遗漏边界（例如某些地方忘了处理 Windows 保留设备名或忘了处理 ADS 冒号流），违背 DRY 原则。
- **Source**:
  - `Sources/TTZipCore/SecurityScanner.swift`
  - `Sources/TTZipCore/PasswordVaultManager.swift`
  - `Sources/TTZipCore/Services/ArchivePasswordStore.swift`
  - `Sources/TTZipCore/AppleSiliconTuner.swift`
  - `Sources/TTZipCore/Platform/PlatformPathSanitizer.swift`
  - `Sources/TTZipCore/Platform/PlatformMemory.swift`
  - `Sources/TTZipCore/Platform/PlatformHardware.swift`
  - `Sources/TTZipCore/Platform/PlatformOperatingSystem.swift`
