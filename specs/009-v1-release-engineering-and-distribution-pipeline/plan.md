# Implementation Plan: TTZip v1.0.0 生产级发布工程与全生态自动化分发流水线 (Feature 009)

- **Feature ID**: `009-v1-release-engineering-and-distribution-pipeline`
- **Created**: 2026-08-24
- **Status**: `READY`

---

## 1. 阶段架构与分步执行策略

### Phase 1: SPM 目标拓扑彻底解耦与双端测试闭环 (P0)
1. 重构 `core/Package.swift`：
   - 移除 `products` 中的 `TTZipApp`、`TTZipQuickLook`、`TTZipFinderSync`。
   - 移除 `targets` 中的 `TTZipApp`、`TTZipQuickLook`、`TTZipFinderSync`、`TTZipAppTests`。
   - 保持 `TTZipCore`、`CTTZipBridge`、`TTZipVendor`、`ttzip-bench`、`TTZipTests` 纯库拓扑。
2. 更新 `apple/Package.swift`：
   - 依赖本地 Core：`.package(path: "../core")`。
   - Target 依赖指定为 `.product(name: "TTZipCore", package: "core")`（或按 package 声明）。
3. 验证执行：
   - `swift test --package-path core`（166 个用例全通）。
   - `swift test --package-path apple`（17 组 XCTest 用例全通）。

### Phase 2: 开源许可证合规、SPDX 头部注入与 LOC 门禁 (P0)
1. 运行 `core/scripts/inject_spdx_headers.py` 注入 UniFFI 生成文件的缺失头。
2. 运行 `python3 core/scripts/audit_licenses.py` 确保 100% 通过（0 失败）。
3. 运行 `python3 core/scripts/lint_loc_gate.py` 校验 `core` 与 `apple` 源码文件均 $\le 800$ LOC。

### Phase 3: macOS 客户端 Universal App 组装、DMG 制作与公证就绪 (P0)
1. 运行 `apple/scripts/bundle_app.sh` 编译并组装 Release `TTZip.app`。
2. 运行 `apple/scripts/create_dmg_installer.sh` 生成高压缩 `dist/TTZip-1.0.0.dmg`。
3. 运行 `apple/scripts/notarize_dmg.sh --diagnose` 验证本地签名与 Gatekeeper 评估。
4. 运行 `apple/scripts/generate_appcast.sh` 生成 `apple/appcast.xml`。

### Phase 4: CLI 发布包组装、SHA256 Manifest 生成与 Homebrew 对齐 (P0)
1. 编译发布版 `ttzip` CLI 二进制（`cargo build --release -p ttzip-tui`）。
2. 打包 `dist/ttzip-cli-v1.0.0-darwin-universal.tar.gz`，内嵌二进制、man 手册与三套 completions。
3. 计算并输出 `dist/checksums.txt`。
4. 校验 `homebrew/Formula/ttzip.rb` 格式与测试语法。

### Phase 5: 全链路 CI/CD 质检回归与发布签收 (P0)
1. 运行 `make -C core test-all-sdk` 确保各语言 SDK 测试套件通过。
2. 运行 `make -C core test-out-of-tree-smoke` 验证无源码环境下的 SDK 引用。
3. 提交并同步所有变更。
