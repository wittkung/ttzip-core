# Tasks: 075 Mac App Store Release Sprint

**Feature Branch**: `075-mac-app-store-release-sprint`  
**Created**: 2026-08-18  
**Status**: `ad-hoc 渠道包及本地产物一致性检查完成`（正式签名、独立签名包集成、公证和商店验收仍未完成）  

---

## Phase 1: Foundational Assets & Metadata

- [x] T001 Implement `scripts/generate_app_icon.sh` generating `Sources/TTZipApp/Resources/AppIcon.icns` from `logo/AppIcon.png`.
- [x] T002 Implement Apple Privacy Manifest `Sources/TTZipApp/PrivacyInfo.xcprivacy`.
- [x] T003 Update `Sources/TTZipApp/TTZip.entitlements` with required App Sandbox keys.
- [x] T004 Expand `Sources/TTZipApp/Info.plist` with full 16-format UTIs and localized bundle names.

---

## Phase 2: MAS Build & Packaging Pipeline

- [x] T005 Implement `scripts/package_mas_app.sh` building `-DMAS_BUILD -c release` and creating `dist/mas/TTZip.app`.
- [x] T006 Implement unit test suite `Tests/TTZipTests/AppStorePackageAuditTests.swift` validating sandbox, entitlements, PrivacyInfo, and UTI completeness.

---

## Phase 3: Verification & Polish

- [x] T007 Run `swift test --filter AppStorePackageAuditTests`.
- [x] T008 Run `scripts/package_mas_app.sh` to package and test the `.app` bundle.
- [x] T009 Run local 6-stage CI gate (`scripts/run_local_ci_gate.sh`).

## 2026-10-16 重新验收：10 月 23 日上架目标

旧版 Completed 仅表示当时的任务完成，不代表当前构建已完成商店提交或审核。
本轮使用工作账本方式记录实际证据；商店状态、签名账号和真机交互尚未核实。

- [x] 10/16 修复视频容器移出仍显示控件：取消隐藏任务并立即开始短弹簧淡出，播放/暂停均适用；悬停边界统一为整个 SwiftUI 容器，移动时刷新空闲计时。
- [x] 10/16 紧凑/宽屏控制条材质底板 opacity 调为 0.55，保留文字和滑块自身透明度；参照本地 IINA `MainWindowController.mouseExited` 的销毁计时器与隐藏流程，未复制第三方代码。
- [x] 10/16 `bazel test //infra/ttmedia:TTMediaKitTests //products/ttzip/apple:AppCoreGateTests --test_output=errors`：57 + 90 项测试通过，`git diff --check` 通过。现有测试不覆盖鼠标真实事件和透明度观感。
- [ ] 10/16–17 完成媒体控件人工验收：播放/暂停时移出、重新进入、在视频与控制条间移动、静止 3 秒、拖动至容器外、倍速菜单、紧凑/宽屏/全屏、明暗画面。当前未进行 GUI 验证。
- [ ] 10/17 明确首发支持范围并冻结新增功能；优先修复崩溃、文件损坏、权限失败、无响应及首发支持格式播放失败。
- [ ] 10/17–18 解决或明确收敛媒体隔离缺口：`MediaWorkerXPCClient.makeDefault()` 当前使用 `MediaWorkerDirectTransport`；`apple/BUILD.bazel` 的 `TTZipApp.additional_contents` 当前仅包含 `ttarchive-worker`，未见媒体 worker/XPC service 打包。与 ADR-017 要求存在差距，不能将已有协议测试当成发布包端到端隔离验证。
- [ ] 10/18–19 在实际 MAS Release 包回归创建/解压、加密/错误密码、损坏包、中文长路径、大文件、覆盖/取消、归档内预览、Finder/QuickLook、重启后书签授权；检查失败提示和数据完整性。
- [x] 10/16 完整 `ttaction run --no-skip` 15/15 阶段通过；首次失败定位为默认 Python 缺少 tomllib，已用本机 Python 3.14 重跑。具体播放/隔离验证见 feature 230 的本轮记录。
- [ ] 10/19–20 验证实际商店签名与 provisioning、主程序/扩展/helper 权限、包内动态依赖及无开发机路径依赖；使用干净环境运行最终归档包。
- [ ] 10/20–21 完成 App Store Connect 元数据、截图、隐私回答、审核说明，上传并提审；当前未检查账号或商店状态。
- [ ] 10/21–23 仅处理阻断问题与审核反馈，使用相同构建路径复测返修包；10/23 是目标日期，不将外部审核完成时间视为已确认。

10/16 后续进展：播放时间、EOF、seek、错误反馈及 Rust worker 空闲退出已修复；正式应用已接入只读 FileHandle 的包内 XPC 服务，真实沙盒烟测和应用归档构建通过。外部文件、真实素材与最终商店签名包仍按上面的发布验收项执行。

---

## 2026-10-17 H 组三渠道（MAS / Direct / Steam）发布自检与修复台账

**当前发布状态**：`ad-hoc 渠道包及本地产物一致性检查完成`（正式签名、独立签名包集成、公证和商店验收仍未完成）。

### 已接收项
- [x] H1：修复 `build_bazel_channels.sh` 可选 entitlements 参数处理（空参数省略 `--entitlements`，非空无效路径报错），完整执行三渠道脚本打包。
- [x] H1：各渠道独立保存签名后应用归档（`dist/bazel/<channel>/TTZip-0.1.0-<channel>.zip`）与 Bazel 原始归档，记录完整源码快照（`git_head_commit`、`git_diff_head_sha256`、`dirty_files_sha256`）、签名信息及包内 15 个可执行文件 SHA-256。
- [x] H1：`bazel_notarize_dmg.sh` 上传前强制核验渠道、DMG 路径与 `pre_notarize_sha256`，支持 `--verify-only` 本地验证与 `post_staple_sha256` 回写。
- [x] H2：`QuarantineWorkerClient` 父进程 `F_DUPFD_CLOEXEC >= 10` 重映射，消除子进程 FD 4 / FD 5 `dup2` 碰撞。
- [x] H3：按四组件（`TTZipApp`、`TTZipFinderSync`、`TTZipQuickLook`、`TTMediaWorkerService`）真实调用拆分并打包 `PrivacyInfo.xcprivacy`（含 `1C8F.1` 与 `CA92.1`）。

### 本轮收尾项
- [x] H2-PublicBookmark：移除生产链路 `QuarantineWorkerClient.swift` 与 `ttarchive-worker` 中的私有 `sandbox_extension_issue_file` / `sandbox_extension_consume` / `sandbox_extension_release` API；改为通过公开 Foundation / CoreFoundation 隐式安全作用域 URL Bookmark（`minimalBookmark` + `CFURLCreateByResolvingBookmarkData` / `CFURLStartAccessingSecurityScopedResource` / `CFURLStopAccessingSecurityScopedResource`）跨进程传递访问权，保留 `--archive-fd 4` / `--target-dir-fd 5` 与 `DirJail` 隔离设计。
- [x] H2-VerificationTools：将原合成令牌脚本重命名并入仓为 `products/ttzip/apple/scripts/verify_synthetic_sandbox_extension.py`（合成 sandbox extension 授权测试）；另补入仓的真实 App Sandbox 宿主验证（`products/ttzip/apple/Tests/SandboxPowerboxHost/` 与 `products/ttzip/apple/scripts/verify_nsopenpanel_sandbox_host.py`），通过 `NSOpenPanel` 选择外部源归档与外部目标目录，经生产 `QuarantineWorkerClient` 调用链完成解压并验证未授权外部目录拒绝访问。
- [x] H3-LicenseClosure：重构 `products/ttzip/core/scripts/generate_acknowledgements.py`，移除固定组件列表、多 crate 合并项及 `STANDARD_LICENSE_TEXTS` 缩略文本；按三渠道实际构建依赖闭包收集全部第三方组件（187 项）、精确版本及原始上游 `LICENSE`/`COPYING`/`NOTICE` 全文，缺失版本或许可证文件时立即报错退出。
- [ ] Gate-UIPreviewKitTests：保留 `//products/ttzip/apple:UIPreviewKitTests` 在最终发布门禁中（不因非 H 组职责排除）；已记录当前 4 个失败测试方法及日志路径，交 UI / 媒体预览对应组修复：
  - 日志路径：`/private/var/tmp/_bazel_kevintung/bc7b818d367659311eafb4e72b48b678/execroot/_main/bazel-out/darwin_arm64-opt/testlogs/products/ttzip/apple/UIPreviewKitTests/test.log`
  - 失败用例 1：`-[UIPreviewKitTests.FinderMillerColumnsNavigationTests testIsNavigableArchiveFolder]`（`FinderMillerColumnsNavigationTests.swift:18`：`XCTAssertFalse failed`）
  - 失败用例 2：`-[UIPreviewKitTests.MediaPreviewAuditTests testAll69MediaFormatsClassificationAndZeroHexFallthrough]`（`MediaPreviewAuditTests.swift:610,613,622,627,636,638,658,663,671`：模拟文本载荷 `"mock video payload for ..."` / `"mock audio payload for ..."` 被嗅探器归类为 `.text` 而非 `.video` / `.audio`，且缺少 `ts` 格式枚举）
  - 失败用例 3：`-[UIPreviewKitTests.MediaPreviewAuditTests testAudioFormatMatrixAndNonNativeAudioClassification]`（`MediaPreviewAuditTests.swift:320,329,337,345,353`：`"mock ... audio stream"` 文本载荷被归类为 `.text` 而非 `.audio`）
  - 失败用例 4：`-[UIPreviewKitTests.MediaPreviewAuditTests testMKVAndNonNativeVideoContainerClassificationAndFallback]`（`MediaPreviewAuditTests.swift:258,267,277`：`"mock mkv video container stream"` 文本载荷被归类为 `.text` 而非 `.video`）

