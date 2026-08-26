# Tasks: 032-libarchive-hardware-crc32-acceleration

## Phase 1: Setup & Groundwork
- [x] T001 [US1] 检查 `Vendor/libarchive-upstream/libarchive/archive_crc32.h` 当前实现并备份原始定义 in `Vendor/libarchive-upstream/libarchive/archive_crc32.h`

## Phase 2: User Story 1 (US1) - ARMv8 ACLE 硬件加速与主循环实现
- [x] T002 [US1] 在 `archive_crc32.h` 中引入 `LIBARCHIVE_HAS_ARM_CRC32` 探测宏与 `<arm_acle.h>` 包含 in `Vendor/libarchive-upstream/libarchive/archive_crc32.h`
- [x] T003 [US1] 实现 8 字节对齐前置处理、64 字节 8 路 `__crc32d` 超标量展开主循环与尾部处理 in `Vendor/libarchive-upstream/libarchive/archive_crc32.h`

## Phase 3: User Story 2 (US2) - 通用兜底与边界安全验证
- [x] T004 [US2] 验证并保留纯 C99 256 元素查表 fallback 路径及空指针/零长度边界检查 in `Vendor/libarchive-upstream/libarchive/archive_crc32.h`

## Phase 4: User Story 3 (US3) - 上游 CMake 构建、测试与性能门禁
- [x] T005 [US3] 在 `Vendor/libarchive-upstream` 下执行 CMake 构建并运行 `libarchive_test` 全量通过 in `Vendor/libarchive-upstream`
- [x] T006 [US3] 运行 TTZip `swift test --filter Libarchive7zEncryptionTests` 验证硬件加速吞吐与零性能倒退 in `Tests/TTZipTests/`
