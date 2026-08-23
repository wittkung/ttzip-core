# Tasks: 7Z 全方位深度升级执行清单

- [x] **TASK-001 (Phase 1)**: Thread-Local 解码器池化与零堆分配重构 (`ttzip_lzma2_dec_native.c`) -> 测速验证 -> Commit & Push
- [x] **TASK-002 (Phase 2)**: ARM64 NEON 向量化 BCJ 指令跳转表过滤器与无分支加速 (`ttzip_bcj_arm64_neon.c`) -> 测速验证 -> Commit & Push
- [x] **TASK-003 (Phase 3)**: 7z 固实归档 $O(1)$ 随机访问索引表实现 (`SevenZipSeekTable.swift`) -> 测速验证 -> Commit & Push
- [x] **TASK-004 (Phase 4)**: 7z-Zstd 现代混合容器 Codec 扩展支持 (`CTTZipBridge_7zNativeDecoder.c`) -> 测速验证 -> Commit & Push
