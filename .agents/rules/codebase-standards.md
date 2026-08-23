# TTZip 源码规范、SPDX 版权与英文文档铁律 (Codebase Standards & Documentation Mandate)

> 本规则为全工程最高优先级代码规范，适用于 `Sources/`、`Tests/`、`scripts/` 下的所有 `.swift`、`.c`、`.h`、`.py`、`.sh` 文件。

---

## 一、 统一 SPDX 版权头部声明 (Mandatory SPDX Header)

所有新建与修改的源文件顶部第 1 行起必须包含标准 SPDX 版权声明：

### Swift / C / C 头文件 (`.swift`, `.c`, `.h`)
```c
// SPDX-License-Identifier: LicenseRef-TTZip-Source-Available-1.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine for macOS.
```

### Shell 脚本 (`.sh`)
```bash
#!/usr/bin/env bash
# SPDX-License-Identifier: LicenseRef-TTZip-Source-Available-1.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
```

### Python 脚本 (`.py`)
```python
#!/usr/bin/env python3
# SPDX-License-Identifier: LicenseRef-TTZip-Source-Available-1.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
```

- **作者姓名**：`Witt Kung`
- **联系邮箱**：`witt.w.kung@gmail.com`

---

## 二、 libarchive 工业级英文文档与注释标准 (Industrial English Documentation)

1. **零中文原则 (Zero Non-English Characters in Source Code)**：
   - 源码文件中的所有函数 Docstrings、代码注释、错误提示、常量说明和内部日志必须 100% 使用专业英文。
   - 严禁在源码中出现任何中文字符（UI 本地化字符串文件 `Localizable.xcstrings` 或独立 i18n 字典除外）。

2. **Doxygen / Docstrings 结构化规范**：
   - 公共函数、类和结构体必须提供完整的说明文档。
   - 明确标注入参边界条件（Parameters）、返回值含义（Returns）、错误码（Errors）、线程安全性（Thread-Safety）以及内存所有权语义（Memory Ownership）。

3. **宏与条件编译分支注释**：
   - `#if` / `#elif` / `#else` 条件编译分支必须在宏内部紧随其后配备自解释注释，阐明适用平台、编译器特性及选型理由。

---

## 三、 绝对零编译告警铁律 (Zero-Warning Hard Gate)

1. **全链路 0 Warnings 门禁**：
   - 任何改动后必须执行 `./scripts/lint_codebase_standards.sh` 与 `swift build --build-tests -Xswiftc -warnings-as-errors`。
   - 无论是 Debug、Release 还是 Test Target，只要出现 1 个 warning 即阻断合并。
2. **即时清理**：
   - 严禁遗留未使用的变量（`speedStr` 等）、只读 `var` 变量（应收敛为 `let`）或跨类型比较告警。

---

## 四、 测试分层与竞品 CLI 隔离铁律 (Test Tiering & In-Process Mandate)

1. **单测自包含与高吞吐**：
   - 常规 `swift test` 必须在 40 秒内完成，严禁拉起 `pigz`、`7zz`、`advzip`、`ouch` 等外部竞品进程。
   - 跨软件跑分测试（`*PkTests.swift`）与大型语料综合跑分必须通过 `guard ProcessInfo.processInfo.environment["TTZIP_RUN_BENCHMARKS"] != nil else { throw XCTSkip(...) }` 隔离。
2. **系统预言机唯一例外**：
   - 功能测试仅允许调用系统原生底座 `/usr/bin/unzip -t` 或 `/usr/bin/tar -tzf` 做 RFC 格式合规性断言（耗时 < 0.002s）。

---

## 五、 ZIP 压缩档位单一真理源 (Strict 8-Tier ZIP Profile)

1. **8 大黄金标准预设**：
   - ZIP 压缩档位必须且仅绑定 `ZipCompressionProfile.allProfiles`（Tier 0..7：`.store`, `.fast`, `.fastPlus`, `.normal`, `.maximum`, `.graphFast`, `.ultraZopfli`, `.extremePeak`）。
2. **严禁冗余等级循环**：
   - 严禁在测试或业务代码中对 ZIP 使用 `for lvl in 1...12` 遗留迭代，杜绝重复触发 15 轮次 Zopfli 极端重平衡导致的性能爆炸。

---

## 六、 零倒退物理验证门禁 (Zero-Regression Verification Gate)

代码合并前必须依次执行并通过：
1. `./scripts/lint_codebase_standards.sh`: SPDX 头部、C 桥接纯英文与 0 Warning 门禁。
2. `swift test`: 1000+ 单元测试 100% 全部通过（耗时 < 40 秒）。
3. `swift test --filter XCTestPerformanceMeasureTests`: 核心吞吐门禁 100% 达标。
4. `./scripts/run_all_tests.sh`: 6 阶段自动化回归全部 PASS。
5. `swift build -c release` 与 `swift build -c release -Xswiftc -DMAS_BUILD`: 双渠道编译 0 errors, 0 warnings。
