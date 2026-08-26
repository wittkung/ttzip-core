# Quickstart: TTZip 交互式 TUI 与 CLI 引擎 (Feature 170)

**Feature ID**: `170-rust-interactive-tui-engine`  
**Created**: 2026-08-21  
**Status**: Completed  
**Artifact**: Phase 1 Validation & Quickstart

---

## 1. 验证场景 1: 构建独立 Standalone Universal CLI 二进制

### Command
```bash
./scripts/build_tui.sh --release
```

### Expected Output
```text
[INFO] Building ttzip binary for aarch64-apple-darwin...
[INFO] Building ttzip binary for x86_64-apple-darwin...
[INFO] Combining into Universal Mach-O binary: bin/ttzip
[SUCCESS] Standalone universal binary ready at bin/ttzip
```

### Failure Diagnostic
- **若找不到 `bin/` 目录**: 检查脚本是否具有写入权限。

---

## 2. 验证场景 2: 命令行无头模式列表与解压验证 (Headless Mode)

### Command
```bash
./bin/ttzip list Fixtures/test_archive.zip
```

### Expected Output
```text
Archive: Fixtures/test_archive.zip (Format: ZIP, Entries: 1)
--------------------------------------------------------------------------------
Path                                 Uncompressed      Compressed   Ratio  CRC32
--------------------------------------------------------------------------------
sample.txt                                   18 B            18 B  100.0%  0x12345678
--------------------------------------------------------------------------------
Total: 1 files, 18 B (0 directories)
```

---

## 3. 验证场景 3: 运行 TUI 单元与快照测试

### Command
```bash
cargo test --manifest-path rust/ttzip-tui/Cargo.toml
```

### Expected Output
```text
running 8 tests
test app::tests::test_app_state_navigation ... ok
test vfs::tests::test_vfs_tree_building_and_nesting ... ok
test vfs::tests::test_fuzzy_search_filtering ... ok
test preview::tests::test_text_and_hex_preview_truncation ... ok

test result: ok. 8 passed; 0 failed; 0 ignored
```
