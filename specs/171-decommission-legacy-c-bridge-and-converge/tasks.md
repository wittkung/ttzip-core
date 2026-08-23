# Tasks: CTTZipBridge 遗留 C 代码库清理与架构收敛 (Feature 171)

**Feature ID**: `171-decommission-legacy-c-bridge-and-converge`  
**Created**: 2026-08-21  
**Status**: Ready for Implementation  
**Specification**: [`specs/171-decommission-legacy-c-bridge-and-converge/spec.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/171-decommission-legacy-c-bridge-and-converge/spec.md)  
**Plan**: [`specs/171-decommission-legacy-c-bridge-and-converge/plan.md`](file:///Users/kevintung/Documents/dev/TTZip/specs/171-decommission-legacy-c-bridge-and-converge/plan.md)

---

## Phase 1: 补充 Rust 兼容 FFI 符号 (US2 - Compat FFI in Rust)

- [x] T001 [P] [US2] 在 `rust/ttzip-glue/src/ffi/compat_ffi.rs` 实现内存对齐、字符串自然排序、魔数探测与快解兼容 C-ABI 符号 in `rust/ttzip-glue/src/ffi/compat_ffi.rs`
- [x] T002 [P] [US2] 注册 `compat_ffi` 至 `rust/ttzip-glue/src/ffi/mod.rs` 并运行 `cargo test` in `rust/ttzip-glue/src/ffi/mod.rs`

---

## Phase 2: 头文件与静态库同步 (US2, US3 - Sync Headers & Staticlib)

- [x] T003 [P] [US3] 运行 `./scripts/build_rust.sh --release` 生成包含新兼容符号的 Universal 静态库与头文件 in `scripts/build_rust.sh`
- [x] T004 [P] [US3] 更新 `Sources/CTTZipBridge/include/ttzip_platform.h` 与 `module.modulemap` in `Sources/CTTZipBridge/include/`

---

## Phase 3: 清理遗留 C 源文件与目录 (US1 - Decommission Legacy C Code)

- [x] T005 [P] [US1] 保留 `CTTZipBridge.c`，删除 `Sources/CTTZipBridge/` 下的其余 92 个 `.c` 文件 in `Sources/CTTZipBridge/`
- [x] T006 [P] [US1] 删除 `Sources/CTTZipBridge/` 下的嵌套子目录 `fast-lzma2/`, `lzfse/`, `native_inflate/`, `snappy/`, `zopfli/` in `Sources/CTTZipBridge/`
- [x] T007 [P] [US1] 简化 `Package.swift` 中 `CTTZipBridge` target 的配置 in `Package.swift`

---

## Phase 4: 全量测试验证与收敛 (US1, US3 - Converge & CI Verification)

- [x] T008 运行 `swift test` 验证 859 项 Swift 测试全绿
- [x] T009 运行 `./scripts/run_local_ci_gate.sh` 验证 7 阶段本地 CI 门禁全绿
