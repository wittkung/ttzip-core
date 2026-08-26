# Implementation Plan: 047-libarchive-elegance-and-decoupling

**Feature Name**: `047-libarchive-elegance-and-decoupling`  
**Milestone**: Industrial Code Standards, Architectural Decoupling & Comprehensive Documentation Alignment  
**Dependencies**: [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)  

---

## 一、 技术上下文 (Technical Context)

构建对标 `libarchive` 的世界级工业注释、正交解耦与状态机恢复体系：

```mermaid
graph TD
    subgraph "正交解耦体系 (Orthogonal Decoupling)"
        CF["ArchiveContainerFormat<br>(zip, 7z, tar, cpio, iso, wim, ar)"]
        SF["ArchiveStreamFilter<br>(none, gzip, bzip2, xz, zstd, lz4, brotli, lzip, lrzip)"]
        APC["ArchivePipelineCompositor<br>(正交笛卡尔映射 & Fast-Path 直通)"]
        CF --- APC
        SF --- APC
    end

    subgraph "自解释文档与契约体系 (Documentation & Contracts)"
        H["C 桥接头文件 (CTTZipBridge.h, ttzip_platform.h)"]
        PAL["Swift PAL 平台模块 (Platform*.swift)"]
        H -->|@brief / @note Ownership / @param [in,out] / @return| DocC["DocC & Doxygen 100% 覆盖"]
        PAL -->|时间空间复杂度 / Invariants / Sendable 契约| DocC
    end
```

---

## 二、 架构原则审查 (Constitution Check)

1. **热路径零成本抽象 (Zero-Cost Abstraction)**：
   - 保留 Zip 并行、Tar-Zstd 直通与 7z SIMD Fast-Path，杜绝在热路径插入动态堆对象。
2. **零性能倒退铁律**:
   - 保持 46 项基准吞吐硬门禁全达标。

---

## 三、 Phase 0: 深度技术调研 (Research)

- R001 [SUBAGENT:research] 《libarchive 工业级代码注释契约与容器-滤镜正交解耦架构研究》

---

## 四、 Phase 1: 数据模型与契约 (Data Model & Contracts)

- [x] `data-model.md`: 定义 `ArchiveContainerFormat`, `ArchiveStreamFilter`, `ArchivePipelineComposition`, `TTZipStatus`。
- [x] `contracts/pipeline_composition_schema.json`: 强类型 Schema。
- [x] `contracts/engine_status_schema.json`: 状态码 Schema。
- [x] `quickstart.md`: 3 大验证场景。

---

## 五、 改动清单与组件设计 (Component Breakdown)

### 1. 正交管道数据模型与解耦中枢
- `[NEW]` [`Sources/TTZipCore/Pipeline/ArchiveContainerFormat.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Pipeline/ArchiveContainerFormat.swift): 容器格式与流式滤镜正交枚举。
- `[NEW]` [`Sources/TTZipCore/Pipeline/ArchivePipelineCompositor.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Pipeline/ArchivePipelineCompositor.swift): 正交管道组合器与 Fast-Path 判定。
- `[NEW]` [`Sources/TTZipCore/Pipeline/TTZipStatus.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Pipeline/TTZipStatus.swift): 6 级错误码与状态恢复模型。

### 2. C 桥接层头文件自解释契约注释重构
- `[MODIFY]` [`Sources/CTTZipBridge/include/ttzip_platform.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/ttzip_platform.h): 补齐 libarchive 级四维契约注释。
- `[MODIFY]` [`Sources/CTTZipBridge/include/CTTZipBridge.h`](file:///Users/kevintung/Documents/dev/TTZip/Sources/CTTZipBridge/include/CTTZipBridge.h): 补齐四维契约注释。

### 3. Swift PAL 平台模块工业级 DocC 注释规范化
- `[MODIFY]` [`Sources/TTZipCore/Platform/PlatformMemory.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Platform/PlatformMemory.swift): 补齐复杂度、所有权与内存屏障文档。
- `[MODIFY]` [`Sources/TTZipCore/Platform/PlatformPathSanitizer.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Platform/PlatformPathSanitizer.swift): 补齐防御深度与不变式文档。
- `[MODIFY]` [`Sources/TTZipCore/Platform/PlatformFileSystem.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Platform/PlatformFileSystem.swift): 补齐 POSIX/Win32 映射与预分配文档。
- `[MODIFY]` [`Sources/TTZipCore/Platform/PlatformHardware.swift`](file:///Users/kevintung/Documents/dev/TTZip/Sources/TTZipCore/Platform/PlatformHardware.swift): 补齐 CPU 拓扑与指令集探测文档。

### 4. 单元测试套件
- `[NEW]` [`Tests/TTZipTests/ArchiveOrthogonalPipelineTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/ArchiveOrthogonalPipelineTests.swift): 正交组合与 Fast-Path 验证测试。
- `[NEW]` [`Tests/TTZipTests/TTZipStatusAndRecoveryTests.swift`](file:///Users/kevintung/Documents/dev/TTZip/Tests/TTZipTests/TTZipStatusAndRecoveryTests.swift): 状态码映射与恢复测试。
