# Quickstart: TTZip v1.0.0 发布工程执行与验证指南 (Feature 009)

---

## 1. 快速执行全流程发布打包

```bash
# 1. 验证 Core 与 Apple 单元测试
swift test --package-path core
swift test --package-path apple

# 2. 执行许可证合规与代码行数防线门禁
python3 core/scripts/inject_spdx_headers.py
python3 core/scripts/audit_licenses.py
python3 core/scripts/lint_loc_gate.py core
python3 apple/scripts/lint_loc_gate.py

# 3. 构建 macOS 原生 App 与 Retina DMG
./apple/scripts/bundle_app.sh
./apple/scripts/create_dmg_installer.sh
./apple/scripts/generate_appcast.sh

# 4. 构建 CLI 发布包与 Checksums 清单
cargo build --release --manifest-path core/rust/Cargo.toml -p ttzip-tui
```
