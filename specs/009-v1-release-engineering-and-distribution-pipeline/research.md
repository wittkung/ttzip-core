# Research: TTZip v1.0.0 发布工程架构与生态分发机制 (Feature 009)

- **Feature ID**: `009-v1-release-engineering-and-distribution-pipeline`
- **Created**: 2026-08-24
- **Status**: `COMPLETED`

---

## 1. SwiftPM 多包依赖拓扑与 Target 隔离研究

### 1.1 现状与问题根因
- 在 Swift 6.0 包图解析规范中，同一个依赖图谱（Package Dependency Graph）内的 Target 名称必须具有全局唯一性。
- 当 `core/Package.swift` 声明 `TTZipApp`、`TTZipFinderSync`、`TTZipQuickLook` 时，`apple/Package.swift`（同样声明了这些 Targets）在引入 `ttzip-core` 作为依赖时会触发 Target 命名冲突。

### 1.2 解决方案
- **Core 职责限定**：`core/Package.swift` 仅作为 Pure Engine SDK 包，仅保留并导出：
  - Products: `TTZipCore` (library), `CTTZipBridge` (library), `ttzip-bench` (executable).
  - Targets: `TTZipVendor` (binaryTarget), `CTTZipBridge` (target), `TTZipCore` (target), `ttzip-bench` (executableTarget), `TTZipTests` (testTarget).
- **Apple 职责限定**：`apple/Package.swift` 作为客户端 Consumer 包，包含 `TTZipApp`、`TTZipFinderSync`、`TTZipQuickLook` 与 `TTZipAppTests`，依赖本地 `../core`（或已解耦的 `ttzip-core` 仓库）。

---

## 2. macOS 生产级签名、公证与 Sparkle 2.0 机制

### 2.1 Inside-Out 代码签名与 Hardened Runtime
- 签名顺序必须严格遵循 Inside-Out：
  1. 先对内部 Helper 二进制、Frameworks（`Sparkle.framework`）与 PlugIns 签名。
  2. 再对主程序 `TTZip.app` 注入 Hardened Runtime 标志 (`--options runtime --timestamp`) 与直装 Entitlements (`TTZip-Direct.entitlements`)。
  3. 最后对封装完成的 `TTZip-1.0.0.dmg` 执行 DMG 签名。

### 2.2 Apple Notary Service 提交与票据装订
- 使用 `xcrun notarytool submit <DMG> --keychain-profile <PROFILE> --wait`。
- 接收 `Accepted` 状态后，使用 `xcrun stapler staple <DMG>` 装订公证票据。
- 最终使用 `spctl -a -vv -t install <DMG>` 进行 Gatekeeper 准入评估。

### 2.3 Sparkle 2.0 EdDSA (Ed25519) 自动更新规范
- Sparkle 2.x 要求在 `appcast.xml` 的 `<enclosure>` 节点包含 `sparkle:edSignature="..."`。
- 在 `Info.plist` 中必须配置 `SUPublicEDKey` 对应公钥字符串。
- `appcast.xml` 必须记录精确的 DMG 字节长度、版本号 (`10000`) 与语义化版本 (`1.0.0`)。

---

## 3. CLI 与 Homebrew 分发机制研究

### 3.1 CLI 发布包组装
- 独立 CLI 发布包命名为 `ttzip-cli-v1.0.0-darwin-universal.tar.gz`。
- 内部目录结构：
  - `bin/ttzip` (执行文件)
  - `bin/ttzip-cli` (指向 `ttzip` 的符号链接)
  - `share/man/man1/ttzip.1`
  - `share/zsh/site-functions/_ttzip`
  - `share/bash-completion/completions/ttzip`
  - `share/fish/vendor_completions.d/ttzip.fish`
  - `LICENSE`, `README.md`
- 打包参数：`COPYFILE_DISABLE=1 tar --no-mac-metadata --no-xattrs -czf ...`。

### 3.2 Homebrew Formula 对齐
- `homebrew/Formula/ttzip.rb` 需维护源码构建测试，并可无缝对接 GitHub Release Tarball 与 SHA256 散列校验。
