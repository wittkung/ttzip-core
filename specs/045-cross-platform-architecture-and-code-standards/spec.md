# Feature Specification: 045-cross-platform-architecture-and-code-standards

**Feature Name**: `cross-platform-architecture-and-code-standards`  
**Status**: `Draft`  
**Target Milestone**: TTZip v2.0 Dual-Platform Engine (macOS 14+ Sonoma & Windows 11 / Server 2022)  
**Created**: 2026-08-17  

---

## 一、 业务动机与第一性原理 (Context & First Principles)

TTZip 作为面向 Apple Silicon macOS 平台打造的极致性能归档压缩工具，已经验证了“100% 进程内纯原生 C 绑定 + 零拷贝内存流 + 硬件 SIMD 深度加速”的技术威力。

然而，为了使 TTZip 进化为世界级的全平台归档压缩引擎并为未来 Windows 11/x86_64 及 Windows on ARM 的发布奠定坚实基石，必须向拥有 20 余年跨平台工业实践的世界顶级开源项目 **libarchive** 深度对标：

1. **跨平台基础设施前置 (Cross-Platform Architecture by Design)**：
   - 学习 libarchive 的 `archive_platform.h`、`archive_windows.c/h` 与 `archive_read_disk_windows.c` 架构设计，构建系统级 **平台抽象层 (Platform Abstraction Layer, PAL / HAL)**。
   - 彻底解耦业务逻辑层与底层 OS 原语，使文件系统 I/O、内存映射、路径规范化、扩展属性及并发调度在 macOS 与 Windows 上均具备 1:1 对齐的接口，同时在两个平台均发挥出各自操作系统的极限硬件吞吐（macOS: APFS/mmap/NEON；Windows: Direct I/O/IOCP/AVX2/AES-NI）。
2. **代码工程规范与代码异味根除 (Code Standards & Refactoring)**：
   - 消除散落在 Swift 业务层中未经抽象的裸 POSIX 系统调用（`open`, `mmap`, `stat`, `unlink`, `lstat`）。
   - 统一 C 桥接层的符号命名、错误码传播、句柄生命周期管理与跨平台类型安全（`off_t` vs `int64_t` vs `__int64`，`wchar_t` vs `char*`）。
   - 建立全套跨平台编码规范指南与自动化静态检查契约。

---

## 二、 用户故事 (User Stories)

### User Story 1 (P1): 跨平台底层抽象层 (Platform Abstraction Layer, PAL) 架构重构
作为核心引擎开发者，我希望底层文件 I/O、虚拟内存映射、路径操作与硬件加速检测均通过抽象接口调用，使得在 macOS 和 Windows 下均可无缝编译运行，且保持 0 堆分配与 0 抽象损耗。

- **Acceptance Criteria**:
  - `PlatformFileSystem` 提供统一的 `openFile`, `statFile`, `preallocateDiskSpace`, `readDirect`, `writeDirect` 接口。
  - `PlatformMemory` 提供统一的 `mapFileReadOnly`, `allocateAlignedPages`, `secureZeroMemory` 接口，macOS 映射为 `mmap/munmap`，Windows 条件编译预留 `VirtualAlloc/CreateFileMappingW/MapViewOfFile`。
  - `PlatformPath` 提供跨平台路径规范化（统一正斜杠 `/` 内部表示，Windows 边界自动处理 `\`、驱动器盘符 `C:`、UNC 路径与 `\\?\` 长路径前缀）。

### User Story 2 (P1): 双平台极限硬件加速与指令集特化 (Dual-Platform Hardware Peak Engine)
作为性能工程师，我希望 TTZip 在 macOS 上打满 Apple Silicon NEON/mmap 极限性能的同时，在 Windows 上具备对 AVX2/AVX-512/AES-NI 及 Overlapped/Direct I/O 的原生硬件调度支持，且双方 Fast-Path 绝不互相退化。

- **Acceptance Criteria**:
  - `PlatformHardware` 提供运行时 CPU 指令集特性探测（ARM NEON, x86 AES-NI, AVX2, AVX-512, VAES）。
  - 加密解密引擎分发器通过 PAL 调度：ARM 走 Apple Silicon NEON SIMD，x86_64 走 AES-NI / AVX2，杜绝任何平台性能损耗。
  - 核心编解码与 I/O 门禁保持历史最优基准，零性能倒退。

### User Story 3 (P2): 字符集与长路径跨平台穿透 (Unicode, CodePage & Long Path Normalization)
作为跨平台用户，我希望解压包含 Windows GBK/Shift-JIS 乱码或 32,767 字符超长路径的归档时，系统能自动平滑处理并正确落盘。

- **Acceptance Criteria**:
  - 学习 libarchive 字符集探测与 UTF-8 <-> UTF-16 (`wchar_t`) 宽字符转换管道。
  - 路径校验器支持 Windows `\\?\` 扩展前缀与禁止文件名（`CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9`, 尾部空格/点号）安全防御与转义。
  - APFS NFD (分解形式) 与 Windows/Linux NFC (预组合形式) 自动规范化对齐。

### User Story 4 (P2): 代码规范全面审查与 C 桥接层优雅重构 (Code Elegance & Clean Architecture)
作为架构师，我希望 C 桥接层与 Swift 核心库遵循最严苛的跨平台 C11 / Swift 6 标准规范，消除历史代码异味。

- **Acceptance Criteria**:
  - 统一 C 头文件跨平台导出宏（`TTZIP_API`, `TTZIP_PUBLIC`, `__cdecl`, `__stdcall` 兼容）。
  - 统一结构体内存对齐（`#pragma pack` 与 `aligned` 跨编译器一致性）。
  - 彻底清理未加保护的指针强转与裸内存拷贝。

---

## 三、 功能需求 (Functional Requirements)

1. **FR001**: 必须实现 `PlatformAbstractionLayer` (PAL) 统一接口，包含 `PlatformFileSystem`、`PlatformMemory`、`PlatformPath`、`PlatformHardware` 四大子模块。
2. **FR002**: 必须在 C 桥接层 (`Sources/CTTZipBridge/include/`) 中引入 `ttzip_platform.h` 与 `ttzip_windows.h`，对标 libarchive 的 `archive_platform.h` 与 `archive_windows.h`。
3. **FR003**: 必须实现统一的跨平台路径格式化器 `PlatformPathSanitizer`，支持 `/` 与 `\` 双向安全转换、Windows 保留设备名拦截、UNC 与 `\\?\` 长路径解析。
4. **FR004**: 必须将 `ZipMemoryEngine` 与 `MmapBufferHandle` 迁移至 `PlatformMemory` 统一接口，确保 macOS 与 Windows 虚拟内存映射生命周期严格一致。
5. **FR005**: 必须在硬件探测模块 `PlatformHardware` 中增加 x86_64 AES-NI / AVX2 / AVX-512 CPUID 指令探测，保持与 Apple Silicon NEON 的对称抽象。
6. **FR006**: 必须建立跨平台安全擦除 `ttzip_secure_zero`（macOS: `memset_s`；Windows: `SecureZeroMemory`；Generic: volatile 函数指针）。
7. **FR007**: 必须编写全套跨平台兼容性单测与路径规范化单测（覆盖 Windows 盘符、长路径、非法字符、Unicode 转换）。

---

## 四、 成功指标 (Success Criteria)

- **SC001 (0 架构耦合)**: Swift 核心库 `Sources/TTZipCore/` 中 100% 消除裸 POSIX 系统调用，全部经由 PAL 层调度。
- **SC002 (0 性能倒退)**: 全格式 46 项基准测试在 macOS 平台保持 $\ge 100\%$ 历史峰值吞吐，$\Delta \ge 0.0\%$。
- **SC003 (0 代码警告)**: Swift 6 严格并发与 C11 编译在 Debug / Release 模式下保持 0 Warnings, 0 Errors。
- **SC004 (100% 路径覆盖)**: 路径规范化测试覆盖 Windows 盘符、UNC 共享、长路径前缀、DOS 保留名等 30+ 种复杂边缘用例。
