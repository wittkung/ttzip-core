# Feature Specification: 全面国际化、CLI 标准化与测试体系专业化构建 (Comprehensive i18n, CLI & Test Suite Standardization)

**Feature Branch**: `068-i18n-cli-test-standardization`  
**Created**: 2026-08-17  
**Status**: Ready for Clarification / Planning  
**Input**: User description: "我们需要全面国际化 cli 需要全面标准化，专业化，测试体系需要全面标准化专业化"

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 多语言与无缝国际化体验 (Priority: P1)

作为全球各地区的用户（包括开发者与桌面端用户），我希望无论是运行桌面应用还是命令行工具，TTZip 都能自动以我的母语（或我指定的系统语言）提供清晰、准确、专业的术语和交互界面，并支持无缝动态切换语言，以便于无语言障碍地使用全部归档与压缩功能。

**Why this priority**: 国际化是产品全球化与商业级交付的基础门槛。当前全量硬编码文本严重限制了跨语种用户使用，统一的多语言体系是 CLI 与 GUI 标准化的前置依托。

**Independent Test**: 
- 设置系统或环境语言为不同语言（如英文、简体中文、繁体中文、日文、德文、法文、西班牙文），启动 App 或运行 CLI 命令，验证界面文字、提示信息、错误诊断与单位格式均呈现目标语言，且无任何未翻译的硬编码占位。
- 在 GUI 中切换应用语言设置，界面实时刷新且无需重启。
- 在 CLI 中通过参数或环境变量指定语言，输出立即呈现对应语言。

**Acceptance Scenarios**:
1. **Given** 用户的系统语言环境为 `en_US`，**When** 运行 `ttzip-cli --help` 或启动 GUI，**Then** 所有菜单、按钮、参数描述与状态事件均展示纯正专业的英文表达。
2. **Given** 用户在 CLI 中执行任何归档操作，**When** 传入 `--lang zh-Hans` 或环境变量 `LC_ALL=zh_CN.UTF-8`，**Then** 输出的实时进度、成功汇报与错误诊断信息均以简体中文呈现。
3. **Given** 发生归档损坏或安全威胁拦截事件，**When** 核心引擎向外抛出事件与错误，**Then** 错误代码与多语言模板正交绑定，不同端（GUI / CLI / 诊断日志）均可呈现本地化清晰诊断信息。

---

### User Story 2 - 企业级标准化的专业命令行交互 (Priority: P1)

作为开发者、DevOps 运维人员或自动化脚本编写者，我希望 `ttzip-cli` 具备符合 POSIX/GNU 标准的命令行规范、清晰的退出状态码、交互式彩色终端输出与非交互式机器可读输出（JSON/NDJSON 流）双模支持，以及标准管道（stdin/stdout）流式处理能力，以便于在本地开发、批处理和 CI/CD 自动化流水线中无缝集成。

**Why this priority**: 命令行是开发者与企业自动化场景的唯一入口。专业、规范、鲁棒的 CLI 设计是工具生态专业度的核心体现。

**Independent Test**:
- 运行包含标准 POSIX 选项的命令（如 `-v`, `-h`, `-q`, `-y`, `--json`, `--threads`），验证参数解析正确性。
- 在交互式 TTY 终端下执行大文件压缩/解压，观察平滑高频节流的进度条、速度与估算时间。
- 重定向输出到管道或文件（非 TTY），验证自动禁用 ANSI 颜色转义，并输出结构化 JSON 或干净数据流。
- 模拟各类成功与失败场景，验证进程返回确切的标准退出状态码（如 0 为成功，64 为参数错误，65 为数据格式损坏等）。

**Acceptance Scenarios**:
1. **Given** 用户在自动化流水线中调用 `ttzip-cli archive output.tar.zst src/ --json`，**When** 操作执行期间及结束时，**Then** CLI 向 stdout 输出结构化 NDJSON 进度事件与最终统计，向 stderr 输出纯文本告警，执行完毕后返回状态码 0。
2. **Given** 用户在终端交互环境执行解压操作，**When** 终端连接至标准 TTY，**Then** CLI 渲染具备 Unicode 样式的高性能进度条、即时吞吐速率（MB/s）与剩余时间（ETA），并在完成后展示美观的结构化摘要卡片。
3. **Given** 归档文件输入密码错误或校验和损坏，**When** 命令执行终止，**Then** CLI 输出本地化且附带明确排查指引的错误提示，并返回标准化的非零退出代码（如 65: Data format error 或 66: Cannot open input）。
4. **Given** 用户使用 UNIX 管道传输数据 `cat data.tar | ttzip-cli archive - -f zstd > data.tar.zst`，**When** 指定 `-` 作为输入/输出流，**Then** CLI 透明进行标准流式处理，不在 stdout 混杂任何进度日志。

---

### User Story 3 - 全维度多层次标准化测试体系 (Priority: P1)

作为核心架构师与测试维护者，我希望拥有一套分层严谨、全矩阵覆盖、具备黄金预言机与性能门禁的标准化自动化测试体系，使代码库在每次重构、特性添加与性能优化时能够秒级执行微测试、分钟级完成全量集成与差分回归，并自动拦截任何吞吐倒退与格式兼容性破损。

**Why this priority**: 专业的归档工具必须保证 100% 数据一致性与极限性能。标准化的测试架构是杜绝数据损坏、内存越界与性能倒退的终极防线。

**Independent Test**:
- 运行针对特定分层的测试套件（如 Tier 0 微基准、Tier 1 功能集成、Tier 2 系统差分、Tier 3 性能硬门禁、Tier 4 崩溃模糊测试）。
- 验证国际化字典覆盖率测试，断言 100% 的文案键在所有语言包中均已提供对应翻译。
- 验证所有 16 种格式与系统原生工具（`/usr/bin/tar`、`/usr/bin/unzip` 等）的双向差分测试结果。
- 验证性能门禁测试对历史最优峰值的基线回归拦截能力。

**Acceptance Scenarios**:
1. **Given** 开发者新增或修改了本地化文案，**When** 运行国际化完整性测试套件，**Then** 测试自动扫描所有代码中的文案引用与全语种资源包，若存在缺失翻译或占位符格式不匹配，立即抛出明确断言失败。
2. **Given** 开发者修改了底层 C 桥接或 Swift 并行解压逻辑，**When** 运行 Tier 3 性能门禁测试，**Then** 测试自动对比历史最优基准矩阵（`604d44d`），若任一格式吞吐发生超过阈值的倒退即刻阻断。
3. **Given** 运行 Tier 2 差分预言机测试，**When** 自研引擎生成压缩包，**Then** 测试调用系统原生工具进行独立解压，并比对解压前后目录树哈希，断言 100% 比特级一致。
4. **Given** 执行 Tier 4 变异模糊测试，**When** 传入破坏性畸形数据包，**Then** 引擎安全返回确界错误而绝不发生未捕获的段错误或死循环，并在沙盒保留最小崩溃复现样本。

---

### Edge Cases

- **终端不支持 ANSI 颜色或 Unicode 字符**：在 `TERM=dumb` 或非 UTF-8 终端环境中，CLI 自动降级为纯 ASCII 字符显示，不输出乱码转义字符。
- **未知或部分缺失的区域语言设置**：若用户指定的 Locale 在语言包中未完全支持（如 `pt_BR`），系统应优雅降级回默认英语（`en`），且绝不发生空指针或崩溃。
- **本地化字符串格式化参数不一致**：不同语言中变量占位符顺序可能不同（如 `%1$@` 与 `%2$@`），本地化格式化系统必须保证类型与参数引用的绝对安全性。
- **管道断开（SIGPIPE）与中断信号（SIGINT）**：当 CLI 在输出到管道接收端提前关闭（如 `ttzip-cli ls ... | head -n 1`）或用户按下 `Ctrl+C` 时，CLI 能够优雅捕获信号、清理临时文件与资源句柄并以标准退出码退出。
- **超深路径与超长文件名本地化展示**：在狭窄终端窗口中，CLI 进度与列表展示能够自适应终端宽度（`COLUMNS`），智能中间截断（`...`）保持对齐。

---

## Requirements *(mandatory)*

### Functional Requirements

#### 1. 全面国际化 (Internationalization & Localization)
- **FR-001**: 系统必须提供统一的类型安全本地化管理器，支持多语言动态解析与格式化。
- **FR-002**: 系统初始必须完整支持 7 种主流语言包：英语 (`en`)、简体中文 (`zh-Hans`)、繁体中文 (`zh-Hant`)、日语 (`ja`)、德语 (`de`)、法语 (`fr`)、西班牙语 (`es`)。
- **FR-003**: 本地化管理器必须同时适配 GUI 桌面端（SwiftUI / AppKit 环境下的即时响应式切换）与 CLI / 核心引擎（基于 POSIX 环境变量 `LC_ALL`/`LC_MESSAGES`/`LANG` 或 `--lang` 覆盖）。
- **FR-004**: 核心引擎的所有事件、进度通知、告警与错误模型必须通过正交的本地化代码进行索引，严禁向外抛出未本地化的硬编码字符串。
- **FR-005**: 系统必须提供符合各语种习惯的数字、文件容量单位（自动适配 B/KB/MB/GB/TB 与 IEC 规范）和日期时间格式化工具。

#### 2. CLI 全面标准化与专业化 (CLI Standardization & Professionalization)
- **FR-006**: CLI 必须具备标准化的一级命令（如 `create`/`archive`、`extract`、`list`、`test`、`bench`、`info`、`diff`）与全局通用选项（`-h/--help`, `-v/--version`, `-q/--quiet`, `--verbose`, `--no-color`, `--json`, `--threads`, `--lang`）。
- **FR-007**: CLI 必须提供完善的多级帮助信息系统（主帮助 `--help` 与各子命令专属帮助 `ttzip-cli <command> --help`）与详尽的参数说明、用例示范。
- **FR-008**: CLI 必须支持双重输出模式：交互式 TTY 模式（富文本彩色排版、Unicode 进度条、吞吐测速表）与非交互式管道模式（静默模式、纯文本流、结构化 JSON/NDJSON 事件流）。
- **FR-009**: CLI 必须严格遵循标准 POSIX Sysexits 进程退出状态码规范，不同类型的成功、参数错误、输入/输出故障、损坏与中断必须映射为确定的退出码。
- **FR-010**: CLI 必须支持标准输入/输出流管道（`-` 符号），允许直接通过 stdin 读取原始流并压缩，或将解压数据流输出至 stdout。
- **FR-011**: CLI 必须支持自动生成标准 Shell 补全脚本（Bash, Zsh, Fish）以及 UNIX man page 手册页文件。

#### 3. 测试体系全面标准化与专业化 (Test Suite Professionalization)
- **FR-012**: 测试套件必须按照分层职责划分为 6 大标准梯队：
  - Tier 0: 核心算法与微基准测试 (Micro & Unit Tests)
  - Tier 1: 模块间功能与契约集成测试 (Integration & Contract Tests)
  - Tier 2: 跨生态双向差分预言机测试 (System & Differential Oracle Tests)
  - Tier 3: 全格式性能硬门禁测试 (Performance Regression Gate Tests)
  - Tier 4: 崩溃现场优先变异模糊测试 (Crash-First Mutation Fuzz Tests)
  - Tier 5: 极限容量与高并发压力测试 (Extreme Stress & Concurrency Tests)
- **FR-013**: 系统必须包含自动化国际化完整性测试，断言 100% 本地化键在所有支持语种中无遗漏、无多余且格式化占位符类型一致。
- **FR-014**: 测试框架必须支持灵活的分类过滤调度器（支持按 Tier、按格式、按子系统执行筛选），并生成结构化 JUnit XML / JSON 测试报告。
- **FR-015**: 性能门禁测试必须与历史最优基准矩阵（`604d44d` 记录之 16 种格式、262 项细分维度）绑定，吞吐倒退 $> 10\%$ 时强行断言失败。

---

### Key Entities *(include if feature involves data)*

- **LocalizationBundle / LocaleKey**: 本地化资源包与类型安全文案键，包含语种标识、文案命名空间、多语言键值映射与参数化插值模板。
- **CLICommandContext**: CLI 运行上下文，包含执行命令、解析后选项、输入/输出流类型、TTY 探测状态、格式化输出模式（Human/JSON）与进程退出码调度器。
- **CLIProgressRenderer**: 终端进度渲染器，支持 TTY 自适应宽度计算、Unicode/ASCII 进度条、节流派发与多任务状态卡片。
- **TestTierMatrix**: 测试分层执行配置模型，定义各测试 Tier 的执行超时约束、依赖资源、并发级别与报告输出策略。

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001 (i18n 覆盖率)**: 全工程（GUI / CLI / Core）所有面向用户的静态文本、错误消息与诊断输出实现 100% 本地化键替换，7 种目标语言包翻译覆盖率达 100%。
- **SC-002 (CLI POSIX 与自动化合规)**: CLI 所有子命令均具备标准参数解析、结构化 JSON 输出与 POSIX Sysexits 规范退出码，非 TTY 管道模式下零 ANSI 乱码泄漏。
- **SC-003 (测试体系执行确定性)**: 全量单测与分层测试集实现 100% 通过（Pass Rate = 100%），国际化完整性自动化检查通过率 100%。
- **SC-004 (性能硬门禁全量守门)**: Tier 3 性能门禁对全格式历史峰值（262 项维度）执行 100% 自动核验，确保零性能倒退。
- **SC-005 (体验与诊断无感交互)**: 在 TTY 终端下，CLI 进度刷新平滑且派发频率严格控制在 $\le 60\text{Hz}$，CPU 额外开销 $< 0.5\%$。

---

## Assumptions

- 初始支持的 7 种语言覆盖绝大部分主流用户群体，未来扩展新语种只需增加对应语言包资源，无需改动核心逻辑。
- CLI 在 macOS 环境优先利用终端标准能力（`TIOCGWINSZ` 获取终端行列大小），非 macOS 平台或非 TTY 环境自动安全降级为标准列宽。
- 测试体系在本地和 CI (GitHub Actions macOS-14) 均可使用相同的分层命令快速执行。
