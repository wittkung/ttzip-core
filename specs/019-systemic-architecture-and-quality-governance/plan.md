# Implementation Plan: 019 Systemic Architecture & Quality Governance Hardening

- **Feature Directory**: `specs/019-systemic-architecture-and-quality-governance`
- **Classification**: `[Full SDD]`
- **Status**: `Planning`
- **Created**: 2026-08-26
- **Author**: Antigravity AI & TTZip Architectural Governance Team

---

## 1. Technical Context & Architectural Architecture

### 1.1 Scope of Components & Modules

```
apple/Sources/TTZipApp/
├── Services/
│   ├── AppIntent.swift                     # [NEW] 强类型应用意图与载荷数据结构
│   ├── AppIntentParser.swift               # [NEW] 统一 URL Scheme、文件、DragDrop 解析器
│   ├── AppIntentDispatcher.swift           # [NEW] @MainActor 全局单向意图调度中枢
│   ├── TabLifecycleModifier.swift          # [NEW] KeepAlive 视图激活/失活生命周期 ViewModifier
│   └── FinderFavoritesReader.swift         # [MOD] 消除 Carbon 废弃 API，收敛为现代文件系统查询
│
├── Components/
│   └── KeepAliveTabContainer.swift         # [MOD] 支持 isActive 传递与生命周期广播的持久化容器
│
├── ViewModels/
│   ├── AppViewState.swift                  # [MOD] 统一挂载 AppIntentDispatcher 与各 Tab 子状态机
│   ├── CompressFormSession.swift           # [MOD] 迁移为 @Observable 并支持动态 loadInputPaths
│   ├── PresetWorkspaceViewModel.swift      # [MOD] 迁移为 @Observable 并实现 StatefulTabViewModelProtocol
│   ├── BenchmarkViewModel.swift            # [MOD] 迁移为 @Observable 并实现 StatefulTabViewModelProtocol
│   └── PasswordVaultViewModel.swift        # [MOD] 迁移为 @Observable 并实现 StatefulTabViewModelProtocol
│
├── Views/
│   ├── TTZipMenuCommands.swift             # [MOD] 废弃死通知，直接调用 AppIntentDispatcher
│   ├── MainView.swift                      # [MOD] 单一入口路由分发，挂载 AppIntentDispatcher
│   ├── MainView+Toolbar.swift              # [MOD] 修复 Home 状态下工具栏快捷键失效缺陷
│   ├── CompressModalView.swift             # [MOD] 响应式生命周期绑定，支持热重载
│   └── Explorer/
│       ├── HomeDropZoneView.swift          # [MOD] 修复 NSItemProvider Data 强转丢失 Bug
│       ├── DiskDirectoryBrowserView.swift  # [MOD] 响应外部 rootDirectory 变更
│       └── FinderMillerColumnsView.swift   # [MOD] 限制全局按键监听仅在 Tab 激活时生效
│
├── scripts/
│   ├── bundle_app.sh                       # [MOD] Release-by-Default、符号 strip 与 Hardened Runtime 签名
│   └── lint_repo_hygiene.sh                # [NEW] 仓库卫生与死代码/废弃参数巡检门禁
│
└── Tests/TTZipAppTests/
    ├── AppNavigationStateFlowTests.swift   # [NEW] 状态机切换与 KeepAlive 缓存保留集成测试
    ├── FinderSyncIntentMappingTests.swift  # [NEW] 10 个 Action 识别器逆向解析与多进程同步测试
    └── Harnesses/
        ├── MockFileURLHarness.swift        # [NEW] 沙盒文件 RAII 清理桩
        ├── MockDarwinNotificationHarness.swift # [NEW] Darwin 多进程通知测试桩
        └── KeepAliveTabHarness.swift       # [NEW] 视图持久化容器状态测试桩
```

### 1.2 Constitution Check
- **Swift 6 Strict Concurrency**: Fully compliant. `AppIntentDispatcher` and all Tab ViewModels are `@MainActor` isolated; `AppIntent`, `CompressIntentOptions`, and `ExtractIntentOptions` conform to `Sendable`.
- **Single Source of Truth (SSOT)**: Fully compliant. All entrypoints converge onto `AppIntentParser` -> `AppIntentDispatcher` -> `AppViewState`.
- **Strict Single-File LOC Threshold ($\le 800$ LOC)**: Fully compliant. All newly created or modified components stay under 350 LOC.
- **Zero Broken Window / Zero-Warning Standard**: Fully compliant. All Swift packages build with `-warnings-as-errors` and 0 compiler warnings.
- **Release-by-Default**: Fully compliant. All scripts build `-c release` with LTO and Apple Silicon optimizations enabled.

---

## 2. Execution Phases

### Phase 0: Research & Architecture Foundations (`research.md`)
- Executed 4-way subagent audit across Multi-Entrypoint Routing, SwiftUI State Lifecycle, Test Matrix, and Build Governance. *(Completed)*

### Phase 1: Design & Contracts (`data-model.md`, `contracts/`, `quickstart.md`)
- Define `AppIntent`, `AppIntentEnvelope`, `TabActivationPayload`, `StatefulTabViewModelProtocol` data models.
- Formalize contract interfaces for Intent Router, Tab Lifecycle FSM, and Build Gates.
- Provide step-by-step verification quickstart guide.

### Phase 2: Implementation & Refactoring
- **Step 1**: Core Intent Routing Foundation (`AppIntent.swift`, `AppIntentParser.swift`, `AppIntentDispatcher.swift`).
- **Step 2**: SwiftUI State Lifecycle & KeepAlive Infrastructure (`KeepAliveTabContainer.swift`, `TabLifecycleModifier.swift`).
- **Step 3**: Fix High-Severity Entrypoint & UI Bugs (Drag-and-Drop `NSItemProvider`, Key monitor leak, Toolbar shortcuts, FinderSync action routing).
- **Step 4**: Migrate Tab ViewModels to `@Observable` & `StatefulTabViewModelProtocol`.
- **Step 5**: Build Automation & Repository Hygiene (`lint_repo_hygiene.sh`, `bundle_app.sh`, `core/Package.swift`).
- **Step 6**: Test Suite Delivery (`AppNavigationStateFlowTests`, `FinderSyncIntentMappingTests`, test harnesses).

### Phase 3: Verification & Quality Convergence
- Run `scripts/lint_repo_hygiene.sh`.
- Run `swift test` across `apple/` and `core/` (100% pass rate).
- Run `./apple/scripts/bundle_app.sh --release` and verify 0 warnings, hardened runtime, and release packaging.
