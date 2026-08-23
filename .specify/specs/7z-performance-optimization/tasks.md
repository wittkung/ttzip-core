# Tasks: 7Z 引擎性能优化执行清单

- [x] **TASK-001**: 重构 `CTTZipBridge_7zNativeDecoder.c` 元数据分配为动态容量，消除 1024 文件硬上限，并清理裸 `fprintf`。
- [x] **TASK-002**: 在 `CTTZipBridge_7zNativeDecoder.c` 中实现多 Block `dispatch_apply` 多核并发 LZMA2 解码。
- [x] **TASK-003**: 重构 `CTTZipBridge_7zSolid.c` 中的输入读取循环，采用并发 `pread` + 并行 NEON CRC32 预载数据。
- [x] **TASK-004**: 针对 7Z 解压与压缩路径实施 64 字节内存对齐（`posix_memalign`）。
- [x] **TASK-005**: 运行 `swift test --filter XCTestPerformanceMeasureTests` 与全量 `swift test` 验证性能与功能。
- [x] **TASK-006**: 运行 `swift run ttzip-cli bench -f 7z` 采集 7Z 优化后全矩阵基准并生成对比报告。
