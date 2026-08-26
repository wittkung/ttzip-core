# Tasks: 045-cross-platform-architecture-and-code-standards

**Feature Branch**: `045-cross-platform-architecture-and-code-standards`  
**Input Documents**: [spec.md](./spec.md), [plan.md](./plan.md), [data-model.md](./data-model.md), [research.md](./research.md), [quickstart.md](./quickstart.md), [contracts/](./contracts/)  

---

## Phase 1: Setup & Contracts Verification

- [x] T001 [P] 验证并核对 JSON Schema 契约在 `specs/045-cross-platform-architecture-and-code-standards/contracts/` 下的零通配与字段一致性

---

## Phase 2: C 桥接层跨平台规范与头文件重构 (Baseline)

- [x] T002 [P] 建立 C 桥接层跨平台标准头文件 `Sources/CTTZipBridge/include/ttzip_platform.h` (导出宏 `TTZIP_API`、64-bit 偏移量 `ttzip_off_t`、页对齐宏与防优化 `ttzip_secure_zero`)
- [x] T003 [P] 建立 Windows 适配头文件 `Sources/CTTZipBridge/include/ttzip_windows.h` (Win32 宽字符、句柄与长路径宏对齐)

---

## Phase 3: User Story 1 - 平台抽象层 (PAL) 架构构建 (Priority: P1)

*Goal*: 建立 Swift 6 平台抽象层基础模块，统一 OS 标识、虚拟内存映射与文件系统基础操作。  
*Independent Test*: 编译通过且 `PlatformOperatingSystem.current` 输出当前平台，`PlatformMemory` 成功完成 16KB 页对齐分配与释放。

- [x] T004 [P] [US1] 实现 `PlatformOperatingSystem.swift` 跨平台操作系统标识中枢 in `Sources/TTZipCore/Platform/PlatformOperatingSystem.swift`
- [x] T005 [P] [US1] 实现 `PlatformMemory.swift` 跨平台页对齐分配、安全擦除与虚拟内存映射中枢 in `Sources/TTZipCore/Platform/PlatformMemory.swift`
- [x] T006 [P] [US1] 实现 `PlatformFileSystem.swift` 跨平台文件打开、元数据读取与磁盘预分配中枢 in `Sources/TTZipCore/Platform/PlatformFileSystem.swift`

---

## Phase 4: User Story 2 - 双平台极限硬件加速与指令集特化 (Priority: P1)

*Goal*: 运行时探测 CPUID / ARM NEON 指令集特性，消除业务层裸 POSIX 系统调用，统一由 PAL 调度。  
*Independent Test*: `PlatformHardware.capabilities` 正确识别当前 CPU 指令集；`MmapBufferHandle` 成功通过 `PlatformMemory` 映射文件且单测 100% 通过。

- [x] T007 [P] [US2] 实现 `PlatformHardware.swift` CPUID 探测与 ARM NEON / x86_64 AES-NI 特性掩码 in `Sources/TTZipCore/Platform/PlatformHardware.swift`
- [x] T008 [US2] 重构 `Sources/TTZipCore/Zip/MmapBufferHandle.swift` 与 `Sources/TTZipCore/Zip/ZipMemoryEngine.swift`，接入 `PlatformMemory` 与 `PlatformFileSystem` 消除裸 POSIX 系统调用

---

## Phase 5: User Story 3 - 跨平台路径清理、Unicode 与 Windows 长路径穿透 (Priority: P2)

*Goal*: 实现跨平台路径规范化器，支持 `/` 与 `\` 双向转换、Windows 保留设备名拦截、ADS 冒号剥离与 `\\?\` 长路径支持。  
*Independent Test*: 运行路径测试套件，验证 30+ 种 Windows 与 macOS 边界路径被合规清理。

- [x] T009 [P] [US3] 实现 `PlatformPathSanitizer.swift` 统一路径正规化、`\\?\` 超长路径、DOS 保留设备名与 ADS 冒号拦截 in `Sources/TTZipCore/Platform/PlatformPathSanitizer.swift`

---

## Phase 6: User Story 4 - 跨平台测试套件构建与全回归验证 (Priority: P2)

*Goal*: 编写全套跨平台兼容性单测与质量门禁测试，验证 0 回归。  
*Independent Test*: 运行 `swift test --filter PlatformPathSanitizerTests,PlatformHardwareTests,PlatformMemoryTests` 全部通过。

- [x] T010 [P] [US4] 编写 `PlatformPathSanitizerTests.swift` 覆盖 30+ 项跨平台路径安全用例 in `Tests/TTZipTests/PlatformPathSanitizerTests.swift`
- [x] T011 [P] [US4] 编写 `PlatformHardwareTests.swift` 与 `PlatformMemoryTests.swift` 单元测试 in `Tests/TTZipTests/PlatformHardwareTests.swift` 与 `Tests/TTZipTests/PlatformMemoryTests.swift`

---

## Phase 7: Polish & CI Verification

- [x] T012 运行本地自动化 CI 流水线 `./scripts/run_local_ci.sh --quick` 验证 0 警告、100% 测试通过与性能门禁
