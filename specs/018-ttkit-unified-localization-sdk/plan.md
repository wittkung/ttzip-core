# Implementation Plan: 018 TTKit Unified Localization SDK & Ecosystem Migration

- **Feature Directory**: `specs/018-ttkit-unified-localization-sdk`
- **Classification**: `[Full SDD]`
- **Status**: `Planning`
- **Created**: 2026-08-25
- **Author**: Antigravity AI & TT Architectural Governance Team

---

## 1. Technical Context & Architectural Architecture

### 1.1 Scope of Components & Modules

```
ttkit/packages/localization/
├── tt-i18n-core/                     # [Rust Crate] 跨平台核心内核
│   ├── src/
│   │   ├── catalog.rs                # 零堆分配静态切片检索与动态加载器
│   │   ├── cldr.rs                   # 字节、速率、数字分隔符、复数规则引擎
│   │   ├── engine.rs                 # TTLocalizationEngine (UniFFI 导出对象)
│   │   └── lib.rs
│   └── Cargo.toml
│
├── TTLocalizationKit/                # [Swift Package] Apple 平台 SDK
│   ├── Sources/
│   │   ├── TTLocalizationCore/       # UniFFI 生成代码与基础 Manager
│   │   ├── TTLocalizationUI/         # SwiftUI 响应式原语 (L10nText, @Observable 状态机)
│   │   ├── TTLocalizationAppKit/     # AppKitMenuSynchronizer (3级拓扑菜单同步器)
│   │   └── TTLocalizationIPC/        # Darwin 通知中心与 AppGroup 桥接
│   └── Package.swift
│
├── tt-i18n-web/                      # [TypeScript Package] 前端与 Webview 适配库
│   ├── src/
│   │   ├── useTranslation.ts         # React Hook (响应 Native Bridge 事件)
│   │   └── bridge.ts                 # 与 Native Webkit Bridge 双向同步
│   └── package.json
│
└── tt-l10n-tools/                    # [CLI & CI 工具链]
    ├── src/
    │   ├── codegen/                  # 单源契约 -> Rust / Swift / TS 自动代码生成
    │   ├── linter/                   # SwiftSyntax / AST 裸字符串静态检测
    │   └── validator/                # 1:1 键对齐、4阶防伪翻译检测、格式化参数 Fuzzing
    └── Cargo.toml
```

### 1.2 Constitution Check
- **100% Mozilla UniFFI Mandatory Standard**: Fully compliant. All cross-language interfaces between Rust core and Swift/Python/TS use UniFFI macros without manual C headers or pointers.
- **Swift 6 Presentation Boundary**: Fully compliant. Swift package handles `@Observable` state, AppKit menu topology, and SwiftUI primitives; data lookups and CLDR formatting delegate to Rust.
- **Strict Single-File LOC Threshold ($\le 800$ LOC)**: Fully compliant. All modularized source files remain under 400 LOC.
- **Zero In-Tree Path Invariant**: Fully compliant. Pure in-memory `.rodata` and explicit manifest packaging.
- **Zero-Subprocess Policy**: Fully compliant. Pure in-process dynamic linking and UniFFI FFI calls.

---

## 2. Execution Phases

### Phase 0: Research & Architecture Foundations (`research.md`)
- Deep audit of existing TTZip i18n implementation (Rust `.rodata` slice, UniFFI C-ABI, Swift `AppLocalizationState`, `AppKitMenuSynchronizer`, FinderSync, QuickLook, Frontend React).
- Research on Mozilla Application Services (Rust + Fluent + UniFFI), Unicode ICU4X zero-copy architecture, Swift 6 `@Observable` fine-grained diffing, and Darwin IPC. *(Completed)*

### Phase 1: Design Artifacts (`data-model.md`, `contracts/`, `quickstart.md`)
- Define entity structures, JSON schema contracts for translation catalogs, menu topology models, and IPC payloads.
- Verify all contracts against `.specify/scripts/bash/lint-contracts.sh`. *(Completed)*

### Phase 2: Implementation & Package Scaffolding
- Implement `tt-i18n-core` Rust crate with UniFFI macro exports.
- Implement `TTLocalizationKit` Swift 6 package with `LocalizationState`, `L10nText`, `L10nLabel`, and `AppKitMenuSynchronizer`.
- Implement `tt-i18n-web` TypeScript module.
- Implement `tt-l10n-tools` (CodeGen and CI validator).

### Phase 3: TTZip Application Migration & Ecosystem Integration
- Point `ttzip-engine` and `TTZipCore` to `tt-i18n-core` and `TTLocalizationKit`.
- Fix `frontend/src/views/SettingsView.tsx` and connect webview localization to native bridge.
- Run automated unit tests, GUI localization tests, and CI security gates.
