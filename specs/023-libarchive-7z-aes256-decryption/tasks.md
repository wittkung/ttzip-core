# Implementation Tasks: 023-libarchive-7z-aes256-decryption

## Phase 1: Fork 仓库准备与验证环境构建 (Setup)

- [x] T001 [US3] 克隆 libarchive 上游仓库或在本地工作区建立隔离验证环境 in `Vendor/libarchive-upstream`
- [x] T002 [US3] 配置 CMake 构建目标 `libarchive_test` 并确认基线测试通过 in `Vendor/libarchive-upstream/build`

## Phase 2: 密码学抽象层实现 (User Story 3 - Cryptographic Backend)

- [x] T003 [P] [US3] 在 `archive_cryptor_private.h` 中声明 AES-CBC 与 7z KDF 接口 in `libarchive/archive_cryptor_private.h`
- [x] T004 [P] [US3] 在 `archive_cryptor.c` 中实现 Apple CommonCrypto 后端 AES-256-CBC 分组解密 in `libarchive/archive_cryptor.c`
- [x] T005 [P] [US3] 在 `archive_cryptor.c` 中实现 OpenSSL 与 Windows CNG 后端 AES-256-CBC 分组解密 in `libarchive/archive_cryptor.c`
- [x] T006 [US3] 在 `archive_cryptor.c` 中实现基于 `archive_digest_private.h` 的 7z SHA-256 迭代 KDF 与 UTF-16LE 转码 in `libarchive/archive_cryptor.c`

## Phase 3: 7z 数据流解密集成 (User Story 1 - Stream Encryption)

- [x] T007 [US1] 在 `archive_read_support_format_7zip.c` 的 `read_Folder` 中解析 `_7Z_CRYPTO_AES_256_SHA_256` 属性并保存到 Folder 结构体 in `libarchive/archive_read_support_format_7zip.c`
- [x] T008 [US1] 在 `init_decompression` 中集成密码查询、KDF 密钥派生与 AES-CBC 解密管道初始化 in `libarchive/archive_read_support_format_7zip.c`
- [x] T009 [US1] 在 `decompress` 中将密文输入块经由 AES-CBC 就地解密后传递至 LZMA 解压缩器 in `libarchive/archive_read_support_format_7zip.c`

## Phase 4: 7z 全头加密支持与测试套件绿灯 (User Story 2 & Verification)

- [x] T010 [US2] 在 `read_Header` / `slurp_central_directory` 中处理 `kEncodedHeader` 的密码解密与递归解析 in `libarchive/archive_read_support_format_7zip.c`
- [x] T011 [US1] 更新并运行 `test_read_format_7zip_encryption_data.c` 验证数据流解密正确性 in `libarchive/test/test_read_format_7zip_encryption_data.c`
- [x] T012 [US2] 更新并运行 `test_read_format_7zip_encryption_header.c` 验证头解密正确性 in `libarchive/test/test_read_format_7zip_encryption_header.c`
- [x] T013 [US3] 运行全量 `libarchive_test` 确认零破坏、零回归、零内存泄漏 in `Vendor/libarchive-upstream/build`

## Phase 5: 规范硬化与严谨性修复 (Quality Hardening & Strict Compliance)

- [x] T014 [P] [US3] 修复 `archive_cryptor.c` 中 `utf8_to_utf16le` 的 C89 变量声明前置 in `libarchive/archive_cryptor.c`
- [x] T015 [P] [US1] 修复 `archive_read_support_format_7zip.c` 中 `extract_pack_stream` 的 C89 变量声明前置 in `libarchive/archive_read_support_format_7zip.c`
- [x] T016 [US1] 消除 `setup_decode_folder` 中 `coder_copy` 的重复局部声明以通过 `-Wshadow` 门禁 in `libarchive/archive_read_support_format_7zip.c`
- [x] T017 [US1] 在 `test_read_format_7zip_encryption_*.c` 中补充错误密码负面测试分支 in `libarchive/test/`
- [x] T018 [US3] 全量重新构建并运行 `libarchive_test` 确认 100% 绿灯且零告警通过 in `Vendor/libarchive-upstream/build`
