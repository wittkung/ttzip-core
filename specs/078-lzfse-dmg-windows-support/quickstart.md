# Quickstart & Verification Guide: 078-lzfse-dmg-windows-support

**Feature**: [078-lzfse-dmg-windows-support](file:///Users/kevintung/Documents/dev/TTZip/specs/078-lzfse-dmg-windows-support/spec.md)
**Status**: Completed (Phase 1)
**Date**: 2026-08-18

---

## 1. Scenario 1: 验证 C 静态 LZFSE 编解码与无 `dlopen` 回归测试

验证 `apple/lzfse` 源码已 100% 静态编译入 `CTTZipBridge`，零动态库依赖，单核吞吐达标。

### Command
```bash
swift test --filter AccelerationInfrastructureTests/testLZFSEStreamRoundTrip
```

### Expected Output
```text
Test Suite 'AccelerationInfrastructureTests' passed at ...
	 Executed 1 test, with 0 failures (0 unexpected) in 0.042 (0.042) seconds
```

### Failure Diagnostic
- **若编译期报找不到头文件或符号缺失**：检查 `Package.swift` 中 `CTTZipBridge` target 是否包含 `.headerSearchPath("lzfse")`，并核实 `Sources/CTTZipBridge/lzfse/` 下是否包含全部 14 个核心源码文件。
- **若测试返回 `ttzip_lzfse_is_available() == false`**：检查 `CTTZipBridge_LZFSE.c` 是否已彻底清除 `dlopen` 宏分发，直接返回 `true`。

---

## 2. Scenario 2: 验证 Apple DMG (UDIF LZFSE 0x80000006/0x80000007) 穿透解压

验证在非 macOS / Windows 模拟环境下，包含 LZFSE 压缩块的 DMG 磁盘映像能被 100% 穿透解压。

### Command
```bash
swift test --filter DMGLZFSEExtractionTests
```

### Expected Output
```text
Test Suite 'DMGLZFSEExtractionTests' passed at ...
	 Executed 3 tests, with 0 failures (0 unexpected) in 0.312 (0.312) seconds
```

### Failure Diagnostic
- **若遇到 `Unsupported compression method` 错误**：检查 `ArchiveExtractor+Dispatch.swift` 与 `SevenZipEngine` 的 DMG 解析路由，确认 `0x80000006`/`0x80000007` 块是否已正确分发至 `lzfse_decode_buffer`。
- **若解压输出文件哈希不一致**：检查 `koly` trailer 端序解析（Big-Endian 到 Host）以及 `SectorNumber * 512` 偏移量计算是否存在 32 位溢出。

---

## 3. Scenario 3: 50GB 虚拟流式大镜像内存常驻 (RSS <= 64MB) 门禁验证

验证在处理超大 DMG 分块时，微缓冲拉取管道与 Thread-Local Scratch 内存完全收敛，杜绝 OOM。

### Command
```bash
swift test --filter DMGLZFSEStreamingMemoryGateTests
```

### Expected Output
```text
[DMGLZFSEStreamingMemoryGateTests] Peak RSS measured: 14.82 MB (Floor limit: 64.00 MB)
Test Suite 'DMGLZFSEStreamingMemoryGateTests' passed at ...
```

### Failure Diagnostic
- **若 RSS 突破 64MB**：检查是否有任何地方执行了 `malloc(total_size)` 或 `Data(count:)` 预分配全镜像，确认每个 Chunk 解码完毕后立即释放或复用输入输出缓冲区。
