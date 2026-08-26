# Feature 010: All 16 Formats Benchmark Requirements Checklist

## Requirements Verification
- [ ] 全 16 种格式（ZIP, 7Z, TAR.GZ, TAR.ZST, TAR.BZ2, TAR.XZ, TAR, LZIP, LZ4, BROTLI, LRZIP, AAR, SNAPPY, WIM, DMG, ISO）全部纳入竞品压测矩阵。
- [ ] 扩展 `AllFormatsPkSuiteTests.swift` 并支持全格式并发/顺序执行。
- [ ] 针对已安装的竞品 CLI（Apple ditto/aa/hdiutil, 7zz, zstd, pigz, pbzip2, pixz, plzip, lz4, brotli, lrzip, wimlib）精准对接执行并记录耗时与吞吐。
- [ ] 自动落盘包含全格式 1v1 对比表格的 Markdown 与 JSON 报告。
- [ ] 11 大性能门禁与 560+ 单测 100% 绿灯。
