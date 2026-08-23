# Quickstart Validation Guide: Deep Algorithmic Absorption of libdeflate

**Feature Directory**: `specs/054-libdeflate-deep-algorithmic-absorption`
**Created**: 2026-08-18
**Status**: Completed

---

## 1. 验证场景 1：硬件级 Adler-32 / CRC-32 吞吐与黄金预言机比对

### Command
```bash
# 运行 Adler-32 与 CRC-32 硬件加速单元测试与微基准
swift test --filter HardwareChecksumTests
```

### Expected Output
```text
Test Suite 'HardwareChecksumTests' passed at ...
	 Executed 4 tests, with 0 failures (0 unexpected) in 0.045 seconds
[TTZipChecksum] Adler-32 DotProd throughput: 26,450.2 MB/s (>= 20,000 MB/s target) -> PASS
[TTZipChecksum] CRC-32 PMULL throughput: 31,890.5 MB/s (>= 25,000 MB/s target) -> PASS
[TTZipChecksum] Golden Oracle match with standard zlib: 100% bit-exact across all random buffers.
```

### Failure Diagnostic
- **若 Adler-32 结果与标准不符**：
  检查 5552 字节分块结束处取模序列是否有遗漏；排查加权乘数序列 `mults` 顺序是否为 64 到 1 递减。
- **若吞吐低于 20 GB/s**：
  确认编译参数是否已激活 ARMv8.2-A+dotprod 或 AVX2。

---

## 2. 验证场景 2：16-bit 匹配查找器 SIMD 快速重置与短哈希

### Command
```bash
# 运行匹配查找器重置延迟与哈希命中率测试
swift test --filter FastMatchFinderTests
```

### Expected Output
```text
Test Suite 'FastMatchFinderTests' passed at ...
	 Executed 3 tests, with 0 failures in 0.012 seconds
[Matchfinder] 32KB window rebase duration: 1.82 us (<= 5.0 us floor) -> PASS
[Matchfinder] Unaligned 24-bit hash collision reduction: 18.4% -> PASS
```

### Failure Diagnostic
- **若重置后出现坏匹配或越界**：
  检查 `vqaddq_s16` 加上的值是否为 `-32768` (`0x8000`)；检查过期位置是否被正确饱和截断。

---

## 3. 验证场景 3：全量回归与热路径门禁零倒退

### Command
```bash
# 运行全量单元测试与热路径性能门禁
swift test
swift test --filter XCTestPerformanceMeasureTests
```

### Expected Output
```text
Test Suite 'All tests' passed at ...
	 Executed 685+ tests, with 0 failures
[XCTestPerformanceMeasureTests] ZIP Level 1: >= 1500 MB/s (PASSED)
[XCTestPerformanceMeasureTests] ZIP Decompress: >= 7500 MB/s (PASSED)
```
