# Tasks: Unified AVSampleBuffer Media Pipeline & Hermetic Demuxing Sandbox (Feature 230)

**Feature Directory**: `products/ttzip/core/specs/230-unified-avsamplebuffer-media-pipeline`  
**Classification**: `[Full SDD]`  
**Status**: `Draft`  

---

## Phase 1: Setup & Hermetic Third-Party Infrastructure

- [ ] T001 Add `bazel_dep(name = "rules_foreign_cc", version = "0.13.0")` to [MODULE.bazel](file:///Users/kevintung/Documents/dev/metastudyline/source/MODULE.bazel) and register foreign toolchains.
- [ ] T002 [P] Create hermetic Bazel build definition for minimal whitelist FFmpeg (`libavformat`, `libavutil`, `libavcodec` headers) in [third_party/ffmpeg/BUILD.bazel](file:///Users/kevintung/Documents/dev/metastudyline/source/third_party/ffmpeg/BUILD.bazel).
- [ ] T003 [P] Create hermetic Bazel build definition for `libdav1d` in [third_party/dav1d/BUILD.bazel](file:///Users/kevintung/Documents/dev/metastudyline/source/third_party/dav1d/BUILD.bazel).
- [x] T004 [P] Implement C-ABI header and Swift modulemap for the whitelist demuxer bridge in [infra/ttmedia/include/ttmedia_demux.h](file:///Users/kevintung/Documents/dev/metastudyline/source/infra/ttmedia/include/ttmedia_demux.h).
- [ ] T005 Verify zero Homebrew dynamic library linkage via `otool -L` on compiled static/shared targets in `third_party/ffmpeg`.

---

## Phase 2: Foundational Data Plane & IPC Contracts

- [x] T006 Implement CoreMedia DTO structures (`MediaPacketDTO`, `VideoFormatParameters`, `AudioFormatParameters`, `MediaType`, `VideoCodec`, `AudioCodec`) in [infra/ttmedia/Sources/TTMediaKit/Domain/MediaPacketDTO.swift](file:///Users/kevintung/Documents/dev/metastudyline/source/infra/ttmedia/Sources/TTMediaKit/Domain/MediaPacketDTO.swift).
- [x] T007 Implement the XPC communication protocol interface conforming to [contracts/ttmedia-worker.schema.json](file:///Users/kevintung/Documents/dev/metastudyline/source/products/ttzip/core/specs/230-unified-avsamplebuffer-media-pipeline/contracts/ttmedia-worker.schema.json) in [infra/ttmedia/Sources/TTMediaKit/Services/TTMediaWorkerProtocol.swift](file:///Users/kevintung/Documents/dev/metastudyline/source/infra/ttmedia/Sources/TTMediaKit/Services/TTMediaWorkerProtocol.swift).
- [x] T008 [P] Implement in-place Annex-B to AVCC/HVCC NALU converter (4-byte length prefix substitution) in [infra/ttmedia/src/demux/annexb.rs](file:///Users/kevintung/Documents/dev/metastudyline/source/infra/ttmedia/src/demux/annexb.rs) with unit tests in `tests/test_annexb.rs`.

---

## Phase 3: User Story 1 - Instant Crash-Free Media Preview on Clean macOS (Priority: P1) 🎯 MVP

**Goal**: Deliver instant media preview for MKV and MP4 files on clean macOS machines with 0 Homebrew dependencies and 0 crashes.

- [x] T009 [US1] Scaffold standalone `ttmedia-worker` daemon binary target with App Sandbox entitlements in [infra/ttmedia/BUILD.bazel](file:///Users/kevintung/Documents/dev/metastudyline/source/infra/ttmedia/BUILD.bazel) and [infra/ttmedia/src/main.rs](file:///Users/kevintung/Documents/dev/metastudyline/source/infra/ttmedia/src/main.rs).
- [x] T010 [US1] Implement whitelist `libavformat` container demuxer loop for MKV, WebM, and MP4 in [infra/ttmedia/src/demux/format_demuxer.rs](file:///Users/kevintung/Documents/dev/metastudyline/source/infra/ttmedia/src/demux/format_demuxer.rs).
- [x] T011 [US1] Implement XPC service connection manager in host app [infra/ttmedia/Sources/TTMediaKit/Services/MediaWorkerXPCClient.swift](file:///Users/kevintung/Documents/dev/metastudyline/source/infra/ttmedia/Sources/TTMediaKit/Services/MediaWorkerXPCClient.swift).
- [x] T012 [US1] Integration test asserting `ttmedia-worker` parses local MKV files and delivers packets over XPC with zero Homebrew dependencies.

---

## Phase 4: User Story 2 - Hardware Video Decode & Single Master Clock (Priority: P1)

**Goal**: Implement host `AVSampleBufferDisplayLayer` and `AVSampleBufferAudioRenderer` synchronized by `AVSampleBufferRenderSynchronizer` (`CMTimebase` SSOT).

- [x] T013 [US2] Implement `CMSampleBuffer` reconstruction engine in [infra/ttmedia/Sources/TTMediaKit/UI/Video/AVSampleBuffer/SampleBufferReconstructor.swift](file:///Users/kevintung/Documents/dev/metastudyline/source/infra/ttmedia/Sources/TTMediaKit/UI/Video/AVSampleBuffer/SampleBufferReconstructor.swift) using `CMVideoFormatDescriptionCreateFromH264ParameterSets` and `CMBlockBuffer`.
- [x] T014 [US2] Wire `AVSampleBufferAudioRenderer` and CoreAudio hardware output into `AVSampleBufferRenderSynchronizer` in [infra/ttmedia/Sources/TTMediaKit/UI/Video/AVSampleBuffer/AVSampleBufferAudioPipeline.swift](file:///Users/kevintung/Documents/dev/metastudyline/source/infra/ttmedia/Sources/TTMediaKit/UI/Video/AVSampleBuffer/AVSampleBufferAudioPipeline.swift).
- [x] T015 [US2] Implement non-keyframe seek with preroll frame suppression (`kCMSampleAttachmentKey_DoNotDisplay = true`) in [infra/ttmedia/Sources/TTMediaKit/UI/Video/AVSampleBuffer/AVSampleBufferPlayerCoordinator.swift](file:///Users/kevintung/Documents/dev/metastudyline/source/infra/ttmedia/Sources/TTMediaKit/UI/Video/AVSampleBuffer/AVSampleBufferPlayerCoordinator.swift).
- [x] T016 [US2] Implement native macOS Picture-in-Picture (`AVPictureInPictureController`) attached to `AVSampleBufferDisplayLayer`.
- [x] T017 [US2] Update `UnifiedVideoPlayerView` in [infra/ttmedia/Sources/TTMediaKit/UI/Video/UnifiedVideoPlayerView.swift](file:///Users/kevintung/Documents/dev/metastudyline/source/infra/ttmedia/Sources/TTMediaKit/UI/Video/UnifiedVideoPlayerView.swift) to route all supported containers (MP4, MOV, MKV, WebM, AVI, TS) through the AVSampleBuffer pipeline.
- [x] T018 [US2] Unit and integration tests in [infra/ttmedia/Tests/TTMediaKitTests/AVSampleBufferPresentationTests.swift](file:///Users/kevintung/Documents/dev/metastudyline/source/infra/ttmedia/Tests/TTMediaKitTests/AVSampleBufferPresentationTests.swift) verifying lip-sync drift $\le 15\text{ ms}$ and preroll seeking behavior.

---

## Phase 5: User Story 3 - Sandboxed Isolation & Energy-Efficient Worker Lifecycle (Priority: P2)

**Goal**: Support open codecs and enforce Two-Tier Sleep state machine (30s standby -> 300s cold exit).

- [ ] T019 [US3] Implement `libdav1d` AV1 fallback and VP9 software decoding in [infra/ttmedia/src/codecs/open_video.rs](file:///Users/kevintung/Documents/dev/metastudyline/source/infra/ttmedia/src/codecs/open_video.rs), outputting `IOSurface`-backed pixel buffers.
- [ ] T020 [US3] Implement software audio decoding (Opus, FLAC, Vorbis) outputting linear PCM in [infra/ttmedia/src/codecs/open_audio.rs](file:///Users/kevintung/Documents/dev/metastudyline/source/infra/ttmedia/src/codecs/open_audio.rs).
- [x] T021 [US3] Implement Two-Tier Sleep timer in `ttmedia-worker`: 30s idle -> close demuxer/decoder contexts; 300s idle -> `exit(0)` in [infra/ttmedia/src/lifecycle.rs](file:///Users/kevintung/Documents/dev/metastudyline/source/infra/ttmedia/src/lifecycle.rs).
- [x] T022 [US3] Host XPC connection fault-recovery and reconnection handler in [infra/ttmedia/Sources/TTMediaKit/Services/MediaWorkerXPCClient.swift](file:///Users/kevintung/Documents/dev/metastudyline/source/infra/ttmedia/Sources/TTMediaKit/Services/MediaWorkerXPCClient.swift).
- [x] T023 [US3] Automated lifecycle test in `infra/ttmedia/tests/test_worker_lifecycle.rs` verifying 30s memory drop and 300s process clean termination.

---

## Phase 6: User Story 4 - Complete Physical Decommission of libmpv (Priority: P2)

**Goal**: Permanently delete `third_party/mpv`, `infra/ttmpv`, and all legacy MPV linkages.

- [x] T024 [US4] Remove `//infra/ttmpv:TTMPVKit` dependency from [products/ttzip/apple/BUILD.bazel](file:///Users/kevintung/Documents/dev/metastudyline/source/products/ttzip/apple/BUILD.bazel).
- [x] T025 [US4] Delete `third_party/mpv` directory and remove its target from Monorepo build trees.
- [x] T026 [US4] Delete `infra/ttmpv` directory entirely (`git rm -rf infra/ttmpv`).
- [x] T027 [US4] Remove legacy MPV strings and view references from [products/ttzip/apple/Sources/TTZipApp/Views/SettingsView+Tabs.swift](file:///Users/kevintung/Documents/dev/metastudyline/source/products/ttzip/apple/Sources/TTZipApp/Views/SettingsView+Tabs.swift) and preview handlers.
- [x] T028 [US4] Clean up test suites: remove obsolete MPV adversarial and headless tests from [products/ttzip/apple/Tests/TTZipAppTests/](file:///Users/kevintung/Documents/dev/metastudyline/source/products/ttzip/apple/Tests/TTZipAppTests/).
- [x] T029 [US4] Update `.ttaction.toml` and build configurations to remove `ttmpv` build stages.

---

## Phase 7: Polish, Benchmarking & CI Gates (Priority: P2)

- [x] T030 Add static anti-corruption check in [tools/ci/lint_code_quality.py](file:///Users/kevintung/Documents/dev/metastudyline/source/tools/ci/lint_code_quality.py) forbidding `libmpv`, `mpv_handle`, and `CTTMpvBridge` across all source trees.
- [ ] T031 Benchmark `bench_media_latency`: measure cold preview first-frame latency across MP4, MKV, and WebM (assert $\le 200\text{ ms}$, SC-002).
- [ ] T032 Benchmark `bench_lipsync`: measure audio/video timebase drift over 30 minutes continuous playback (assert $\le 45\text{ ms}$, SC-003).
- [x] T033 Execute `otool -L` audit in CI asserting 0 lines matching `/opt/homebrew` across `TTZip` app and `ttmedia-worker`.
- [x] T034 Run full [quickstart.md](file:///Users/kevintung/Documents/dev/metastudyline/source/products/ttzip/core/specs/230-unified-avsamplebuffer-media-pipeline/quickstart.md) checklist and verify 100% green.

---

## Dependencies & Completion Order

- Phase 1 (T001-T005) blocks Phase 2 and Phase 3.
- Phase 2 (T006-T008) blocks Phase 3 and Phase 4.
- Phase 3 (T009-T012) blocks Phase 4 and Phase 5.
- Phase 4 (T013-T018) delivers core MVP playback.
- Phase 5 (T019-T023) completes open codecs and lifecycle.
- Phase 4 + Phase 5 block Phase 6 (MPV Decommission).
- Phase 6 blocks Phase 7 (CI Gates & Final Polish).

---

## Parallel Opportunities

- **Phase 1**: T002 (FFmpeg Bazel) ∥ T003 (dav1d Bazel) ∥ T004 (C-ABI Header).
- **Phase 2**: T006 (Swift DTOs) ∥ T008 (Rust Annex-B Converter).
- **Phase 4**: T013 (Video Reconstructor) ∥ T014 (Audio Pipeline) ∥ T016 (PiP Controller).

## 2026-10-16 播放链路审查后的修复与重新验收

本轮保留既有控件隐藏/透明度修改。旧有勾选不代表最终包已经验证。

- [x] R001 原生播放器持续发布主时钟时间，只有呈现完成才通知结束；补回调和尾部测试。
- [x] R002 Worker 读取 EOF 后排空播放，seek 取消旧供包并恢复供包；补 EOF/seek 行为测试。
- [x] R003 打通原生/worker 加载与运行错误到可见错误、重试入口，统一 autoPlay 行为。
- [x] R004 Worker 连接保持时独立推进空闲计时；用真实进程、保持 stdin 打开的测试验收。
- [x] R005 核对并落实隔离协议、权限与打包；不得将进程内 DirectTransport 标为独立沙盒完成。
- [x] R006 同步媒体孪生的状态语义、实际路径、性能目标与验证边界，运行相关测试和门禁。

### 本轮验证结果（2026-10-16）

- `TTMediaKitTests` 62 项、`AppCoreGateTests` 90 项通过；真实 Rust worker 生命周期测试通过。新增测试覆盖持续时间回调、呈现结束、EOF 后 seek、未知时长 EOF、暂停加载、错误回传、裸路径/可写句柄拒绝。
- `PATH=/opt/homebrew/bin:$PATH GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=core.fsmonitor GIT_CONFIG_VALUE_0=false ttaction run --no-skip`：15/15 阶段通过。此前默认 Python 缺少 tomllib；使用已有 Python 3.14 解决运行环境问题，没有跳过门禁。
- `bazel build //infra/ttmedia:TTMediaServiceSmokeHost //products/ttzip/apple:TTZipApp //products/ttzip/apple:TTMediaWorkerService` 通过。
- 真实沙盒应用宿主输出 `BUNDLED_MEDIA_XPC_SMOKE_PASSED`；实际签名 entitlements 仅含 app-sandbox=true。TTZip zip 包内服务 ID 为 com.metastudyline.ttzip.media-worker，版本 1.0.0。
- `lint_doc_twin.py`、TTMedia 代码质量检查、bundle_app.sh 语法检查及 `git diff --check` 通过；未提交代码。
- 临时隐式书签方案曾被自动审批以可能扩大文件权限为由拒绝，未采用；改为只读 FileHandle，服务验证 O_RDONLY，不请求书签或用户目录权限。显式持久书签的失败尝试不计为可用能力。

### 仍待发布验收

- [ ] 最终商店签名/配置文件下的外部文件、移动介质、重新授权与关闭/重开；本轮烟测为本机构建，不等于商店签名验收。
- [ ] 真素材的长时间音画同步、尾帧与尾音、快速连续 seek、暂停后 seek、损坏文件、进程崩溃重试。
- [ ] 实际窗口的进度条移出/移入、拖出边界、透明度、全屏切换、错误面板与 Retry。
- [ ] Rust 独立管道 worker 的 standby 后重新打开/恢复与应用内 XPC 空闲回收分别验收；本轮空闲退出测试只证明保持连接的 cold exit，不冒充完整恢复验收。

## 2026-10-16 全格式样本与其他渲染器审查

- [x] R007 建立本地合成的格式样本目录，记录生成工具、校验和、预期预览方式和真实缺口；不以扩展名别名冒充编码覆盖。
- [x] R008 审查音频、图片、PDF、Office、电子书、文本/表格与 Web 渲染路径，核对格式声明和孪生，记录可复现问题；本轮不自动重构生产渲染器。
- [x] R009 对生成素材做结构/解码验证，运行样本审查与相关检查；界面观感保持待人工验收。


### 样本与渲染审查验证结果（2026-10-16）

- 166 个合成样本、153 个扩展名，另有 52 个注册表分类条目缺少真实样本；生成来源、SHA-256、覆盖缺口均已记录在 `infra/ttmedia/Tests/Fixtures/preview/`。
- `validate_preview_corpus.py --external-decoders`：166 个样本检查完成、0 个错误；包含容器结构、校验和与生成音视频的 FFmpeg 全流解码，不等同于 TTZip 播放验收。
- `PreviewCorpusIntegrityTests`、`normalize_headers_tests` 通过；只读 `PreviewCorpusProbe` 已运行，报告保存在 `observations.json`，没有播放声音或执行 Office 写回。
- 完整 `ttaction run --no-skip`：15/15 阶段通过（沿用上文 Python 与 fsmonitor 环境配置）；文档孪生检查通过。生产渲染器在本次样本/审查工作中未修改。
- 详细问题、证据和修复顺序见 `infra/ttmedia/docs/preview-audit-2026-10-16.md`；本次交付为样本与审查证据，不将缺陷标为已修复。

### 审查发现的后续修复（尚未实施）

- [ ] R010 以内容识别处理 TS/DAT 冲突路由，覆盖磁盘、同步与内存入口。
- [ ] R011 阻断自绘 Office 将原容器覆盖为 CSV；区分只读预览与另存导出。
- [ ] R012 修复 Unicode BOM/二进制嗅探顺序，以及损坏 PDF 的终止错误状态。
- [ ] R013 对齐音频首发支持承诺和实际引擎能力，替换合成波形，并用失败样本回归。
- [ ] R014 明确 SVGZ/MHTML、电子书正文、原图缩放与动画的首发边界；补实际应用窗口验收。

## 2026-10-16 用户实机反馈（文档优先）

- [x] R015 先更新 spec/plan：区分全窗口的会话设计缺口/取消竞态、字幕功能缺失和两套显隐状态机；不得用截图文件名替代编码诊断。
- [x] R016 修复旧 owner 取消新播放会话，保证同文件全窗口交接的状态连续性，并补竞态/交接回归。
- [x] R017 合并上下控件显隐状态，消除独立计时与整屏透明命中，覆盖移出与空闲/暂停场景。
- [x] R018 增加可用外挂字幕菜单、Rust SRT/WebVTT 解析与主时钟呈现，覆盖错误/授权/seek/呈现切换。
- [x] R019 验证真实全窗口播放、报告 HEVC/DTS-HD 的独立兼容边界，更新媒体孪生与运行相关门禁。


### 首轮呈现/字幕验证进展（已被后续实机反馈部分推翻）

- 先修订 spec/plan，再实现 owner generation 取消、非零播放状态快照交接、controller identity 对应的 NSView 重建和 loading/failed 状态。
- 顶部/导航/底栏共用 MediaControlsVisibility；移出/暂停/3s 空闲/文件选择交互锁已有状态机回归。原生菜单跟踪的时机与实际移出仍待前台窗口操作验收。
- Rust 新增有界 UTF-8 SRT/WebVTT 解析，通过现有 TTMediaDemux 桥接；Swift 文件选择、授权、后台读取、错误反馈、字幕菜单与主时钟叠字已接线。ASS、内封字幕、PiP 叠字未实施。
- `TTMediaKitTests` 68 项通过（含新增 6 项呈现/显隐/字幕回归），`AppCoreGateTests` 90 项通过，`ttmedia_test` 通过；完整 `ttaction run --no-skip` 15/15 通过，最新 TTZipApp 构建成功。
- 真实 MKV 只读静音探针：6281.561s、HEVC 3840×2160、131-byte extradata、选取 AC3 5.1，2s 后时钟推进/显示层 rendering/无错误。该结果不证明 DTS-HD 解码。
- 临时构建实际窗口的全窗口预览已出现影片画面，AX 可见外挂字幕菜单及非零总时长；窗口处于台前调度后台缩略状态，连续播放、非零位置往返、字幕选择与上下栏移出仍待前台完成。未覆盖 /Applications 中安装包；不记录用户影片私有路径，不复制影片。

- [x] R020 按 IINA 行为核对字幕目录：Off/内封轨/同目录自动外挂/手动外挂，默认父目录；记录权限与用户选择优先级，完成真实多轨验收。

- [x] R021 当前媒体/字幕/沉浸界面统一 TTI18n，补简繁中文、宿主语言同步、英文回切及门禁验证。

### 持久会话修订与真实窗口验收

快照交接被实机验证推翻，已改为持久 VideoPlaybackSession 和 viewport lease，共享同一 controller/时钟/字幕选择。真实 HEVC MKV 在暂停 00:12 后退出并重进全窗口，位置及暂停状态保持；33 条内封字幕菜单可见，简体中文轨选中保持。外挂 SRT 叠字在双向切换后保留；系统选择器默认视频父目录。Rust 有界 Matroska UTF-8 字幕扫描优化后中文轨 1536 cues 用时约 4.58s，后台执行并提供加载提示；不宣称支持 DTS-HD 解码、ASS/位图字幕、MP4 内封字幕或 PiP 叠字。原生 MP4 controller identity/非零位置/旧视图重排和显式 stop 的回归已补充，最新修改待本轮门禁。

### 统一 i18n 验证结果

媒体和沉浸界面共享 TTI18n 类型化词条及 observable 语言状态；TTZip 设置入口在启动/切换时同步。简繁中文词条完整性、英文后备及共享状态回切测试通过，媒体 Swift/Rust 回归和 AppCoreGateTests 通过，完整应用构建成功。`ttaction run --no-skip` 15/15 通过；孪生引用侧栏行号因删除旧停止回调缩短，已修正并通过检查。本轮中文界面尚未进行新版应用的完整实机视觉验收，当前运行的前一临时构建不会自动变更。

## 2026-10-17 字幕容错解复用、IINA 目录发现、后台有界预热与样式系统迭代（设计先行 & 复审对齐）

- [x] R022 先更新 spec/plan/contract：明确区分实机字幕报错（`Subtitle cue end must follow start` 源于全或无严格校验拒绝 DaVinci/WhisperX 零时长块）、同目录非同名工程字幕未列出、切换卡顿与样式缺失中的设计问题与实现问题，并按复审意见修正性能结论与安全边界。
- [x] R023 优化 Rust 字幕引擎（`subtitles.rs`、`subtitle_request_budget.rs`、`container_subtitles.rs`）：实现逐行容错流式解析、零时长块（`end == start`）自动修复与空块跳过、UTF-16LE/BE BOM 解码、TTML/DFXP（默认保留原始时间码，禁止自动猜测减去 1 小时，补“不应偏移”反例测试）与 ASS/SSA 解析；移除 `memmap2` `unsafe` 映射，改用安全 64 KiB `read_at` 窗口缓存并验证文件被外部截断时可安全恢复返回 `UnexpectedEof`；严格区分“用户主动完整加载（`track > 0`，无预热时间预算）”与“后台预热（`track = 0` 默认 8 轨/4000ms，或 `track <= -2` 编码剩余轨道名额、可选单轨目标与剩余毫秒预算）”，并在 `try_encode_prewarm_request` / `decode_container_request` 增加 `i64::MIN` 溢出拒绝（直接返回 `InvalidInput` 错误，**不**回退为批量预热）与 `> u32::MAX` 非法轨道 ID 拒绝（禁止静默截断为另一轨道）；在 `subtitle_request_budget.rs` 建立显式 `register -> cancel -> complete -> reclaim` 请求生命周期表 `SubtitleRequestTable`（已接收/已注册的请求在 `in_flight_scans == 0` 或 `> 0` 时取消均原位保留在 `active_entries` 并标记 `cancelled = true`，绝不降级移入墓碑表、绝不因超过 4096 次其他取消而丢失；活跃容量上限 `MAX_ACTIVE_REQUESTS = 4096` 满时显式拒绝新请求而非静默淘汰已有请求；分项暴露 `active_request_count`、`pre_start_tombstone_count`、`completed_record_count`、`pending_delay_count`），补齐超过 64 次与超过 4096 次后续取消后在途与已注册启动前请求仍被拒绝、4096 容量拒绝、分项长期运行记录回收、并发请求取消隔离及生产扫描中途停止的确定性测试。
- [x] R024 打通 Swift 会话、传输层、XPC Worker 到 Rust 的端到端 `request_id` 取消接线、生命周期回收与全入口竞态防护（`ExternalSubtitleTrack.swift`、`TTMediaWorkerProtocol.swift`、`MediaSubtitleTransport.swift`、`MediaWorkerDirectTransport.swift`、`MediaWorkerService.swift`、`MediaWorkerXPCClient.swift`、`VideoPlaybackSession.swift`、`VideoPlaybackSession+Subtitles.swift`）：支持传统字符编码回退转码、`maxEndPrefix` + `prevHigherEndIndex` 二分上界与跳表重叠检索、同目录 `companionSubtitles` + `directorySubtitles` 展示、优先激活默认字幕轨；移除单轨预热调用 `track > 0` 的预算绕过路径，并在 `SubtitlePrewarmRequestCodec` 拒绝 `Int64.min` 取负溢出与 `> Int(UInt32.max)` 的越界轨道 ID；在 `SubtitleSessionRequestRegistry` 实现 `allocateRequestID -> tryRegisterRequest/beginRequest -> cancelRequest/cancelSession -> finishRequest` 显式回收（移除 `unstartedOrder` 静默驱逐，已注册请求在 `runningDepth == 0` 时取消仍原位保留在 `liveRequests` 直至 `finishRequest` 回收；容量达到 `maxActiveRequests = 4096` 时显式拒绝新请求；所有生产与测试路径包含 `discoverEmbeddedSubtitlesCatalog()` 均配对 `defer { finishRequest(...) }`）；将会话内取消作用域拆分为独立 `request_id` 槽位（`activePrewarmRequestID`、`activePrimaryRequestID`、`activeSecondaryRequestID`、`activeDefaultEmbeddedRequestID`）并通过 `currentSubtitleSessionID()` 统一会话身份；在 `TTMediaWorkerProtocol`、`MediaWorkerXPCTransport`、`MediaWorkerXPCClient` 与 `MediaWorkerService` 打通 `subtitles(forSession:track:requestID:reply:)`、`cancelSubtitle(forSession:requestID:reply:)`、`subtitleRequestStatus(forRequestID:reply:)` 与 `configureSubtitleClusterDelay(forRequestID:delayMs:reply:)`，支持跨进程独立 Worker 进程（`workerPid != getpid()`）主动单轨完整加载（`track > 0`）中途取消、另一轨继续完成及 IPC 状态回传。
- [x] R025 实现字幕样式系统、解除字幕位置与控制栏显隐绑定、移除重复中央播放按钮（`SubtitleStyleConfiguration.swift`、`AVSampleBufferControlsOverlay.swift`、`AVSampleBufferVideoPlayerView.swift`、`MediaLocaleKey.swift`）：`SubtitleOverlayView.bottomPadding` 与 `controlsVisible` 完全解耦，移除 `< 380pt` 的 `compactCenterPlayButton`（底栏点击弹中央 Splash HUD 由播放器 agent 独立收尾），更新媒体孪生（`04_media_pipeline.md`）并通过自动测试与门禁。
- [ ] R026 实机与真实大文件交付证据验收（**当前状态：预算完整性、请求级 `request_id` 生命周期与 >64 / >4096 次取消驻留回收、4096 容量拒绝、分项统计口径、全套无过滤测试、独立 PID Worker 跨进程主动单轨取消及编码边界已全部完成并通过测试；真实大文件性能及前台 GUI 待验收**；全窗口黑屏、底栏触发中央 HUD 继续由原播放器负责人处理）：同一真实 MKV 的冷打开到字幕出现、首次切轨、重复切轨耗时及峰值内存/主线程阻塞测量；暂停在同一句字幕上反复移入/移出鼠标位置不变、实际窗口挂载字幕与全窗口不黑屏的前台实机验收。
- [x] R027 区分用户主动完整加载与后台预热并收口单轨/批量预热剩余时间预算（独立于 `R026`）：移除 `prewarmSingleEmbeddedTrack` 直接调用 `track > 0` 的预算绕过分支；外挂队列预热后统一在 `VideoPlaybackSession.prewarmEmbeddedBatch`（`infra/ttmedia/Sources/TTMediaKit/UI/Video/VideoPlaybackSession+Subtitles.swift`）计算剩余预算 `remainingBudgetMs = (deadline - CFAbsoluteTimeGetCurrent()) * 1000` 并通过 `encodePrewarmContainerRequest`（`track <= -2`）下推至 Rust `decode_container_request` / `resolve_target_tracks` / `read_container_subtitles_for_request`（`infra/ttmedia/src/container_subtitles.rs`），保留 `track > 0` 专供用户主动完整加载与初始默认轨激活。对应测试：`mid_scan_budget_exhaustion_marks_prewarm_incomplete_and_single_track_loads_late_cues`、`prewarm_encoding_rejects_i64_min_overflow_and_unrepresentable_track_ids`（`infra/ttmedia/src/container_subtitles_tests.rs`）、`testMidScanIncompletePrewarmRefusesPartialCacheAndSubsequentSelectionLoadsLateCues`、`testPrewarmEncodingBoundaryRejectsInt64MinOverflowAndUnrepresentableTrackIDs`（`infra/ttmedia/Tests/TTMediaKitTests/MediaPresentationStateTests.swift`）。
- [x] R028 端到端 `request_id` 请求级取消生命周期、>64 次与 >4096 次取消抗覆盖、4096 容量显式拒绝、分项长期回收与独立 PID Worker 跨进程单轨扫描中止验证（独立于 `R026`）：在 Rust `infra/ttmedia/src/subtitle_request_budget.rs` 与 `infra/ttmedia/src/container_subtitles.rs` 及 Swift `SubtitleSessionRequestRegistry` 建立 `register -> cancel -> complete -> reclaim` 生命周期，并严格区分三类证据：
  - Rust（`infra/ttmedia/src/container_subtitles_tests.rs`）：`request_scoped_cancellation_isolates_concurrent_and_prestart_requests`、`production_rust_scan_aborts_mid_flight_when_cancelled`、`cancellation_record_survives_over_64_subsequent_cancellations_and_reclaims_on_completion`（覆盖 `注册 R -> 取消 R -> 4500 (>4096) 次其他取消 -> 迟到启动 R 仍被拒绝 -> 回收`、`4096` 满容量显式拒绝新请求及各分类计数器验证）；
  - Swift `DirectTransport` 与同进程匿名 XPC 接口往返（`infra/ttmedia/Tests/TTMediaKitTests/SubtitleRequestCancellationTests.swift`）：`testRequestScopedCancellationIsolatesPrewarmPrimarySecondaryAndPreStart`、`testCancellationSurvivesOver64SubsequentCancellationsAndReclaimsLongRunningRecords`（覆盖在途 `>64` 次取消、已注册启动前 `>4096` 次取消不丢失、`4096` 容量显式拒绝及分项计数回收验证）、`testSessionStopAndXPCInterfaceRoundTripAndOutOfProcessWorkerCancellationStopNativeScan`（Evidence A：`DirectTransport` 会话关闭隔离；Evidence B：`NSXPCListener.anonymous()` 同进程 XPC 接口往返，`workerPid == getpid()`）；
  - 独立 PID Worker 跨进程验证（`workerPid != getpid()`）：`SubtitleRequestCancellationTests.testSessionStopAndXPCInterfaceRoundTripAndOutOfProcessWorkerCancellationStopNativeScan`（Evidence C：独立子进程 Worker 管道 IPC 往返）与 `TTMediaServiceSmokeHost`（真实打包 `TTMediaWorker.xpc` 跨进程 `NSXPCConnection` 往返，验证 `workerPid != getpid()` 下主动单轨 `track = 3` 停在 `clustersScanned == 1`、并发单轨 `track = 4` 完成 `clustersScanned == 2` 并通过 IPC 返回状态）。
- [x] R029 播放生命周期、呈现权移交与底栏免中央 HUD 主体修复（`VideoPlaybackSession.swift`、`VideoPlaybackSessions.swift`、`AVSampleBufferPlayerView.swift`、`AVSampleBufferVideoPlayerView.swift`、`AVSampleBufferWorkerPlaybackController.swift`、`AVAssetReaderPlaybackController.swift`、`AVSampleBufferPlayerCoordinator.swift`、`MediaPlaybackCoordinator.swift`、`AppViewState.swift`）：双键租约 `(sessionID, viewportID)`、唯一呈现视口 `activePresenterViewportID`、底栏 `togglePlayPauseFromControlBar()` 免中央 Splash HUD、暂停首帧 PTS 锚定、Worker `hasReachedPresentationEnd` 防暂停续播归零及 `[PlaybackLifecycle]` 诊断日志。
- [x] R030 收尾重复注册幂等性、全窗口呈现权优先级、双视口恢复自动重绑、恢复时主/副字幕与手动外挂内容还原、显式 seek 到 0 归零 `lastKnownTime` 及可配置真实 MKV 测试（`VideoPlaybackSession.swift`、`VideoPlaybackSessions.swift`、`AVSampleBufferVideoPlayerView.swift`、`PlaybackLifecycleRegressionTests.swift`）：修复“Inspector `onAppear` → 全窗口注册 → Inspector `task`”延迟注册夺回呈现权与 `displayLayer`；`VideoPlaybackSessions` `@Observable` + `bindViewport` 驱动双视口并存恢复时所有存活视图自动重绑至新会话，确保退出全窗口回退至 Inspector 正常接管画面且最终关闭释放新会话租约；恢复时完整迁移 `subtitleCache`、主/副字幕轨实例及任意目录手动外挂字幕，并在测试中断言实际 cue 文本（`activeSubtitleText` / `activeSecondarySubtitleText`）；修复显式 `seek(to: 0)` 后 `lastKnownTime` 保留旧正数的问题；真实 MKV 测试改为读取 `TTMEDIA_REAL_MKV_FIXTURE` 并在缺素材时显式 `XCTSkip`（明确仅证明 `DirectTransport` 会话状态，不冒充 GUI 窗口与 XPC 验收）。
- [ ] R031 独立临时包（`/tmp/ttzip-verify-230/TTZip.app`）前台 GUI 实机统一验收（**当前状态：本轮实现与专项回归完成，统一集成及 GUI 待验收**）：区分已证实修复的代码缺陷与待实机确认的故障原因，等待前台窗口验证全窗口往返、XPC 跨进程播放、字幕显隐与键鼠交互后关闭（`DirectTransport` 往返测试不能替代实际窗口播放验收）。
- [x] R032 补齐恢复后音量/静音/倍速向新 Controller 真正下发及过期视口绑定按会话/代际清理（`VideoPlaybackSession.swift`、`VideoPlaybackController.swift`、`VideoPlaybackSessions.swift`、`AVSampleBufferVideoPlayerView.swift`、`PlaybackLifecycleRegressionTests.swift`）：修复 `finalizeLoadedController` 用新 controller 默认值反向覆盖 `volume`/`playbackRate` 的问题，通过 `applyPlaybackSettings(to:)` 将恢复的非默认音量、静音、`1.5×` 倍速真正应用到底层 controller 并验证 controller 与 UI 状态一致；为 `viewportBindings` 增加 `(sessionID, generation)` 校验，在旧 `sessionID` 的 `release` 分支清理归属旧会话的过期绑定（覆盖 A→B 视频切换后旧视图退场），同时保证迟到的旧 `release` 不会删除同一 `viewportID` 在新会话上的新绑定。



### 27 个真实外挂字幕样本生产解析器（`ttmedia_subtitles_parse`）匿名可复现验证报告

- **验证边界与样本集合声明**：
  - 本报告统一样本集合为本机 `~/Documents/video_dev/**/*.*` 下的全部真实外挂字幕文件，共 **27 个文件、26 份唯一内容**（按 `(sha256_prefix, path)` 稳定排序编号 `Sample #01` ～ `Sample #27`；其中 `Sample #08` 与 `Sample #09` 为不同子目录下的同一份 `.srt` 副本，SHA-256 前 16 位均为 `297a97e481ecf10c`、字节数均为 `40,383` B）。此前聊天回复附件中出现的另一套未匹配哈希表来自早期临时合成/非本机批次，已作废并以本仓库唯一实测表为准。
  - **同一稳定源码快照与产物标识**：
    - Git 基线提交：`HEAD = 71dfbd3ab2fdf480718e0578eca0793109695c54`
    - 字幕源码与测试文件组合快照摘要（17 个字幕核心与测试文件 SHA-256 汇总）：`b64c1906b078b527149e2b3c84fdb191265ffa1d3df67fa52ee6092d9df5a8bb`
    - 静态库与测试二进制产物标识：
      - `bazel-bin/infra/ttmedia/libttmedia_cabi_static.a`（SHA-256: `1f58db5a534338a9b3f723e719aac778a522ccd607d19fd7a364b9dd9e18d56f`）
      - `bazel-bin/infra/ttmedia/ttmedia_test`（SHA-256: `c3d196b702a49de419e992d8dfe1e81b8b85e518516b63c0ce1ae03d2f37d544`）
      - `bazel-bin/infra/ttmedia/TTMediaKitTests.xctest/Contents/MacOS/TTMediaKitTests`（SHA-256: `56c231e0bee9688fc550df73d054f988895621e261679dda3d0350530fa8075d`）
  - 下表验证的是 **Rust 生产 C-ABI 双参数解析入口 `ttmedia_subtitles_parse(data, len)` 对全部 27 个文件（26 份唯一内容）100% 解析成功（共提取 22,660 条有效 cue，0 报错）**；因未单独保留旧版编译产物的二进制对比环境，已删除此前基于简化规则脚本推算的对比列，仅如实记录当前生产 C-ABI 解析器的实测输出。该结果属于**生产解析器静态与 C-ABI 层验证通过**，与**实际 GUI 窗口内挂载渲染验收**分开记录（GUI 与大文件性能归属未完成的 `R026`）。
- **可复现执行命令**（构建 `//infra/ttmedia:ttmedia_cabi_static`，链接生成临时校验动态库并通过 `ctypes` 调用真实导出的双参数 `ttmedia_subtitles_parse(data, len)` 与 `ttmedia_subtitles_free(ptr)`）：
  ```bash
  bazel build //infra/ttmedia:ttmedia_cabi_static
  shasum -a 256 bazel-bin/infra/ttmedia/libttmedia_cabi_static.a
  clang -dynamiclib -Wl,-force_load,bazel-bin/infra/ttmedia/libttmedia_cabi_static.a \
    -framework CoreFoundation -framework Security -liconv -o /tmp/libttmedia_cabi_verify.dylib
  python3 - <<'PY'
  import ctypes, glob, hashlib, json, os
  lib = ctypes.CDLL("/tmp/libttmedia_cabi_verify.dylib")
  lib.ttmedia_subtitles_parse.argtypes = [ctypes.POINTER(ctypes.c_uint8), ctypes.c_size_t]
  lib.ttmedia_subtitles_parse.restype = ctypes.c_void_p
  lib.ttmedia_subtitles_free.argtypes = [ctypes.c_void_p]
  lib.ttmedia_subtitles_free.restype = None
  exts = {".srt", ".vtt", ".ttml", ".dfxp", ".ass", ".ssa"}
  files = [
      p for p in glob.glob(os.path.expanduser("~/Documents/video_dev/**/*.*"), recursive=True)
      if os.path.splitext(p)[1].lower() in exts and os.path.isfile(p)
  ]
  records = []
  for path in files:
      raw = open(path, "rb").read()
      sha16 = hashlib.sha256(raw).hexdigest()[:16]
      records.append((sha16, path, raw))
  records.sort(key=lambda item: (item[0], item[1]))
  for idx, (sha16, path, raw) in enumerate(records, 1):
      ext = os.path.splitext(path)[1].lower()
      buf = (ctypes.c_uint8 * len(raw)).from_buffer_copy(raw)
      ptr = lib.ttmedia_subtitles_parse(buf, len(raw))
      payload = json.loads(ctypes.cast(ptr, ctypes.c_char_p).value.decode("utf-8"))
      lib.ttmedia_subtitles_free(ptr)
      cues = payload.get("cues", [])
      print(f"Sample #{idx:02d} {ext} sha256={sha16} bytes={len(raw)} cues={len(cues)} start={cues[0]['start']:.3f} end={cues[-1]['end']:.3f} err={payload.get('error')}")
  PY
  rm -f /tmp/libttmedia_cabi_verify.dylib
  ```

| 匿名样本编号 | 格式 | 字节数 | SHA-256 校验和 (前 16 位) | 内容去重说明 | 生产解析器 (`ttmedia_subtitles_parse`) 结果 | 提取有效 Cue 数 | 首个开始时间 (s) | 末个结束时间 (s) |
| :--- | :--- | ---: | :--- | :--- | :--- | ---: | ---: | ---: |
| `Sample #01` | `.srt` | 25127 | `00acac8faa6186fe` | 唯一内容 #01 | `OK (error: null)` | 501 | 0.000 | 1142.840 |
| `Sample #02` | `.ttml` | 10976 | `046447f0e0798f90` | 唯一内容 #02 | `OK (error: null)` | 124 | 3600.000 | 3817.066 |
| `Sample #03` | `.srt` | 158771 | `12c8bd449de294f0` | 唯一内容 #03 | `OK (error: null)` | 2715 | 0.000 | 4449.720 |
| `Sample #04` | `.srt` | 29504 | `15d932809070aa8e` | 唯一内容 #04 | `OK (error: null)` | 385 | 3605.601 | 4471.984 |
| `Sample #05` | `.srt` | 45462 | `171847aa0e8f52f7` | 唯一内容 #05 | `OK (error: null)` | 649 | 0.031 | 1660.483 |
| `Sample #06` | `.srt` | 10770 | `17d127eccad7d531` | 唯一内容 #06 | `OK (error: null)` | 172 | 0.000 | 649.640 |
| `Sample #07` | `.srt` | 107586 | `276c33260652dfbc` | 唯一内容 #07 | `OK (error: null)` | 1637 | 0.000 | 3945.840 |
| `Sample #08` | `.srt` | 40383 | `297a97e481ecf10c` | 唯一内容 #08（与 `#09` 同内容） | `OK (error: null)` | 523 | 3599.996 | 4885.630 |
| `Sample #09` | `.srt` | 40383 | `297a97e481ecf10c` | 重复副本（同 `#08`） | `OK (error: null)` | 523 | 3599.996 | 4885.630 |
| `Sample #10` | `.srt` | 176317 | `2fa2f8a79236434e` | 唯一内容 #09 | `OK (error: null)` | 2686 | 3600.000 | 7975.083 |
| `Sample #11` | `.srt` | 176252 | `304ee39dd39fda76` | 唯一内容 #10 | `OK (error: null)` | 2684 | 3600.000 | 7939.125 |
| `Sample #12` | `.srt` | 10650 | `40eeb362f1491898` | 唯一内容 #11 | `OK (error: null)` | 206 | 0.000 | 194.600 |
| `Sample #13` | `.srt` | 38324 | `4d150ad2bba5425b` | 唯一内容 #12 | `OK (error: null)` | 551 | 0.210 | 1336.045 |
| `Sample #14` | `.ttml` | 9035 | `4d585ad047f5ccc3` | 唯一内容 #13 | `OK (error: null)` | 100 | 3600.000 | 3773.050 |
| `Sample #15` | `.srt` | 161068 | `4f710d63d28b38a4` | 唯一内容 #14 | `OK (error: null)` | 2489 | 0.160 | 3946.760 |
| `Sample #16` | `.ttml` | 9041 | `6260bab5b67cf670` | 唯一内容 #15 | `OK (error: null)` | 100 | 3603.849 | 3773.050 |
| `Sample #17` | `.srt` | 176252 | `6342fc954b33f083` | 唯一内容 #16 | `OK (error: null)` | 2684 | 3600.000 | 7939.125 |
| `Sample #18` | `.srt` | 3095 | `7f18c8211506f3aa` | 唯一内容 #17 | `OK (error: null)` | 63 | 0.000 | 508.040 |
| `Sample #19` | `.srt` | 41858 | `7f82848c57470a0f` | 唯一内容 #18 | `OK (error: null)` | 834 | 0.000 | 3946.760 |
| `Sample #20` | `.srt` | 43196 | `82f2142ffdaf7bb8` | 唯一内容 #19 | `OK (error: null)` | 590 | 0.291 | 1557.446 |
| `Sample #21` | `.srt` | 18782 | `872b2f4986884a4c` | 唯一内容 #20 | `OK (error: null)` | 332 | 0.000 | 505.920 |
| `Sample #22` | `.srt` | 29861 | `9c5105adca85efce` | 唯一内容 #21 | `OK (error: null)` | 573 | 0.000 | 1142.280 |
| `Sample #23` | `.ttml` | 55330 | `d27f3c178aeeabbe` | 唯一内容 #22 | `OK (error: null)` | 601 | 3601.347 | 5137.298 |
| `Sample #24` | `.srt` | 11850 | `d37698f9ce9408c2` | 唯一内容 #23 | `OK (error: null)` | 201 | 3603.599 | 3791.938 |
| `Sample #25` | `.ttml` | 10981 | `d5e7621724d7bf4e` | 唯一内容 #24 | `OK (error: null)` | 124 | 3600.000 | 3817.066 |
| `Sample #26` | `.srt` | 27086 | `eb66b2234ecb0184` | 唯一内容 #25 | `OK (error: null)` | 391 | 0.231 | 873.809 |
| `Sample #27` | `.srt` | 11852 | `ef0c938d23d72a42` | 唯一内容 #26 | `OK (error: null)` | 222 | 0.000 | 267.000 |

### 同一稳定快照下的无过滤全套单元测试、DirectTransport、同进程 XPC 接口往返与独立 PID Worker 跨进程证据记录（2026-10-17）

1. **Rust 单元测试（无过滤：`bazel test //infra/ttmedia:ttmedia_test --test_output=all`）**：
   - 实际执行并全部通过：**35 项，0 失败**（`35 passed; 0 failed; 0 ignored; 0 filtered out`）。
   - 字幕专项覆盖（16 项）：
     - `container_subtitles_tests::prewarm_encoding_rejects_i64_min_overflow_and_unrepresentable_track_ids`：验证 `decode_container_request(i64::MIN)` 直接返回 `Err(InvalidInput("invalid container subtitle request encoding: i64::MIN"))`（拒绝有符号整数取负溢出，**不**回退为批量预热），验证 `try_encode_prewarm_request` 拒绝 `<= 0` 及 `(1 << 32) + 3` 等超过 `u32::MAX` 的轨道号并返回 `None`。
     - `container_subtitles_tests::cancellation_record_survives_over_64_subsequent_cancellations_and_reclaims_on_completion`：验证（a）在途请求 `req_a` 被取消后，即使在 `req_a` 到达下一个 Cluster 检查点前连续发生 128 次（`> 64`）其他请求的取消与完成，`req_a` 的取消状态仍保留在 `active_entries` 中并使 `req_a` 停在第 1 个 Cluster（`clusters_scanned == 1`），并发 `req_b` 完成全部 2 个 Cluster（`clusters_scanned == 2`）；（b）**`注册 req_pre -> 取消 req_pre -> 连续 4,500 次（> 4,096 墓碑容量）其他未注册取消 -> 迟到启动 req_pre`**，验证 `req_pre` 在 `in_flight_scans == 0` 时取消仍原位保留在 `active_entries`（绝不降级移入墓碑表被淘汰），迟到启动仍被拒绝（`clusters_scanned == 0`），且在回收后从 `active_entries` 移除；（c）当活跃请求达到 `MAX_ACTIVE_REQUESTS = 4096` 时，第 4097 个请求被显式拒绝（`register_container_subtitle_request == false`、`read_container_subtitles_for_request` 返回 `"capacity exceeded"`），不覆盖已有 4096 个活跃请求；（d）600 次连续请求后分项验证 `active_request_count == 0`、`pre_start_tombstone_count == 0`、`pending_delay_count == 0`、`completed_record_count <= 256`、`retained_record_count <= 256`。
     - `container_subtitles_tests::request_scoped_cancellation_isolates_concurrent_and_prestart_requests`、`production_rust_scan_aborts_mid_flight_when_cancelled`、`mid_scan_budget_exhaustion_marks_prewarm_incomplete_and_single_track_loads_late_cues`、`batch_prewarm_caps_scanned_tracks_and_respects_time_budget`、`discovers_tracks_and_extracts_timed_text_without_changing_video_cursor`、`recovers_cleanly_when_underlying_file_is_truncated_after_open`、`rejects_corrupt_bounds_and_invalid_vint_without_panicking` 及 `subtitles::tests::*`（9 项）。
2. **Swift 全套无过滤测试（`bazel test //infra/ttmedia:TTMediaKitTests --test_output=all`）**：
   - **全套无过滤实际执行并全部通过：196 项执行，1 项跳过（`testRealMKVDirectTransportTenRoundTripsPausedAndPlayingWhenAvailable` 需外部环境变量），0 失败**（`Executed 196 tests, with 1 test skipped and 0 failures (0 unexpected)`）。其中字幕专项 `MediaPresentationStateTests` 16 项 + `SubtitleRequestCancellationTests` 3 项共 19 项全部通过。
   - **修复全套运行下的 `retained_record_count == 259 > 256` 根因**：此前 `PlaybackLifecycleRegressionTests.testWorkerMKVTenRoundTripsPlayingAndPausedMidLoadAndRecovery` 中的 3 个恢复测试会话使用未实现 `MediaSubtitleTransport` 的模拟传输，在 `discoverEmbeddedSubtitlesCatalog()` 分配并注册 `requestID` 后直接抛出 `MediaWorkerError.serviceUnavailable`，未执行 `finishRequest` 回收，导致遗留 3 条记录（`256 + 3 = 259`）；现已在 `discoverEmbeddedSubtitlesCatalog()` 补齐 `defer { SubtitleSessionRequestRegistry.shared.finishRequest(...) }` 并通过 `currentSubtitleSessionID()` 统一会话标识，同时暴露分项统计接口，全套 196 项测试无过滤执行下 `SubtitleRequestCancellationTests` 实测 `activeRequestCount == 0`、`preStartTombstoneCount == 0`、`pending_delay_count == 0`、`completed_record_count <= 256`、`retained_record_count <= 256`。
   - **严格按进程边界区分三类证据**：
     - **证据分类一：`DirectTransport` 会话与请求级生命周期证据**：
       - `MediaPresentationStateTests.testPrewarmEncodingBoundaryRejectsInt64MinOverflowAndUnrepresentableTrackIDs`：验证 Swift `SubtitlePrewarmRequestCodec` 拒绝 `Int64.min` 取负溢出及 `(1 << 32) + 9` 等不可编码轨道 ID（返回 `nil`），同时验证 C-ABI `ttmedia_subtitles_container_request(path, Int64.min, reqID)` 返回 `InvalidInput` 错误。
       - `SubtitleRequestCancellationTests.testRequestScopedCancellationIsolatesPrewarmPrimarySecondaryAndPreStart`：验证取消预热不影响主/副轨、主轨加载中切副轨主轨仍成功、取消主轨不影响副轨、旧请求延迟取消不影响新请求、启动前取消 `clusters_scanned == 0`。
       - `SubtitleRequestCancellationTests.testCancellationSurvivesOver64SubsequentCancellationsAndReclaimsLongRunningRecords`：验证（1）在途请求 A 取消后再执行 128 次其他请求取消，A 仍在第 1 个 Cluster 停止（`clusters_scanned == 1`）且并发请求 B 完成（`clusters_scanned == 2`）；（2）**`注册 R -> 取消 R -> 4,500 次（> 4,096 墓碑容量）其他未注册取消 -> 迟到启动 R`**，验证 `R` 在 Swift `liveRequests` 与 Rust `active_entries` 中保持 `isCancelled == true`，迟到启动仍被拒绝（`clusters_scanned == 0`），并在 `finishRequest` 后完全回收（`activeRequestCount == 0`、`ttmedia_subtitles_active_request_count() == 0`）；（3）注册满 4,096 个活跃请求时显式拒绝第 4,097 个新请求（`tryRegisterRequest == false`、`ttmedia_subtitles_register_request == false`、`beginRequest` 抛出错误），已有请求不被淘汰；（4）连续 300 轮会话与请求后分项验证 Swift `activeRequestCount == 0`、`trackedSessionCount == 0`、`preStartTombstoneCount == 0`、`cancelledSessionCount <= 128`、`retainedCompletedRecordCount <= 256` 及 Rust `ttmedia_subtitles_active_request_count() == 0`、`ttmedia_subtitles_pre_start_tombstone_count() == 0`、`ttmedia_subtitles_pending_delay_count() == 0`、`ttmedia_subtitles_completed_record_count() <= 256`。
       - `SubtitleRequestCancellationTests.testSessionStopAndXPCInterfaceRoundTripAndOutOfProcessWorkerCancellationStopNativeScan`（Evidence A）：双会话并发扫描时，`sessionOld.stop()` 仅取消旧会话请求（`clusters_scanned == 1`），新会话完成全部 2 个 Cluster（`clusters_scanned == 2`）。
     - **证据分类二：`NSXPCListener.anonymous()` + `MediaWorkerService` 同进程 XPC 接口往返证据（`workerPid == getpid()`）**：
       - `SubtitleRequestCancellationTests.testSessionStopAndXPCInterfaceRoundTripAndOutOfProcessWorkerCancellationStopNativeScan`（Evidence B）：明确归类为**同进程 XPC 协议编解码与接口往返验证**（客户端与 `MediaWorkerService` 共享测试进程 PID），不作为独立 Worker 进程证据。
     - **证据分类三：独立 PID Worker 进程（`workerPid != getpid()`）跨进程主动单轨扫描停止、另一轨继续完成及 IPC 状态回传证据**：
       - **沙盒内跨进程 Worker 回归**（`SubtitleRequestCancellationTests` Evidence C）：通过 `Process()` 启动独立 OS 子进程（断言 `workerPid != getpid()`），在独立进程的地址空间内并发运行 `track = 3`（请求 A）与 `track = 4`（请求 B）主动单轨扫描；父进程通过 IPC 管道发送 `CANCEL <reqA>`，Worker 子进程通过 IPC 管道返回 JSON 状态，断言 `workerPid != getpid()`、`reqA.isCancelled == true`、`reqA.clustersScanned == 1`、`reqB.isCancelled == false`、`reqB.clustersScanned == 2`、`reqB.cueCount == 2`。
       - **真实打包 `TTMediaWorker.xpc` 跨进程 `NSXPCConnection` 验证**（`//infra/ttmedia:TTMediaServiceSmokeHost`）：宿主进程 `TTMediaServiceSmokeHost.app` 通过 `MediaWorkerXPCTransport` 连接独立 `TTMediaWorker.xpc` 进程，并发发起 `track = 3`（`reqA`）与 `track = 4`（`reqB`）单轨请求，中途发送 `cancelSubtitleRequest(sessionId:requestID: reqA)` 并通过 `subtitleRequestStatus(requestID:)` 查询跨进程 Worker 内部状态，实测输出：`BUNDLED_MEDIA_XPC_SUBTITLE_IPC_VERIFIED host_pid=69223 worker_pid=69239 reqA_clusters=1 reqA_cancelled=true reqB_clusters=2 reqB_cues=2` 与 `BUNDLED_MEDIA_XPC_SMOKE_PASSED`（`host_pid != worker_pid`）。
3. **当前工作树跨组文件归属核对清单**（基于 `git status --short` 实际工作树生成）：
   - **本字幕组负责并维护的文件**：
     - Rust / C-ABI：`infra/ttmedia/include/ttmedia_demux.h`、`infra/ttmedia/src/subtitles.rs`、`infra/ttmedia/src/subtitle_request_budget.rs`、`infra/ttmedia/src/container_subtitles.rs`、`infra/ttmedia/src/container_subtitles_tests.rs`
     - Swift 字幕、生命周期注册表与 Worker XPC 接线：`infra/ttmedia/Sources/TTMediaKit/Services/MediaSubtitleTransport.swift`、`infra/ttmedia/Sources/TTMediaKit/Services/TTMediaWorkerProtocol.swift`、`infra/ttmedia/Sources/TTMediaKit/Services/MediaWorkerDirectTransport.swift`、`infra/ttmedia/Sources/TTMediaKit/Services/MediaWorkerService.swift`、`infra/ttmedia/Sources/TTMediaKit/Services/MediaWorkerXPCClient.swift`、`infra/ttmedia/Sources/TTMediaKit/UI/Video/ExternalSubtitleTrack.swift`、`infra/ttmedia/Sources/TTMediaKit/UI/Video/SubtitleStyleConfiguration.swift`、`infra/ttmedia/Sources/TTMediaKit/UI/Video/VideoPlaybackSession.swift`、`infra/ttmedia/Sources/TTMediaKit/UI/Video/VideoPlaybackSession+Subtitles.swift`、`infra/ttmedia/Tests/TTMediaKitTests/MediaPresentationStateTests.swift`、`infra/ttmedia/Tests/TTMediaKitTests/SubtitleRequestCancellationTests.swift`、`infra/ttmedia/Tests/TTMediaServiceSmokeHost/main.swift`
     - 文档与契约：`products/ttzip/core/specs/230-unified-avsamplebuffer-media-pipeline/contracts/external-subtitles.md`、`products/ttzip/core/specs/230-unified-avsamplebuffer-media-pipeline/spec.md`、`products/ttzip/core/specs/230-unified-avsamplebuffer-media-pipeline/plan.md`、`products/ttzip/core/specs/230-unified-avsamplebuffer-media-pipeline/tasks.md`、`products/ttzip/docs/twin/04_media_pipeline.md`
   - **其他并行专项组在当前工作树中的文件（本字幕组不触碰、不回滚）**：
     - 播放生命周期与视口组：`infra/ttmedia/Sources/TTMediaKit/UI/Video/VideoPlaybackSessions.swift`、`infra/ttmedia/Sources/TTMediaKit/UI/Video/AVSampleBuffer/AVSampleBufferPlayerView.swift`、`infra/ttmedia/Sources/TTMediaKit/UI/Video/AVSampleBuffer/AVSampleBufferVideoPlayerView.swift`、`infra/ttmedia/Sources/TTMediaKit/UI/Video/AVSampleBuffer/AVSampleBufferWorkerPlaybackController.swift`、`infra/ttmedia/Sources/TTMediaKit/UI/Video/AVSampleBuffer/AVAssetReaderPlaybackController.swift`、`infra/ttmedia/Tests/TTMediaKitTests/PlaybackLifecycleRegressionTests.swift`
     - 非视频预览/音频/表格/代码/资源生命周期组：`infra/ttmedia/Sources/TTMediaKit/UI/Code/CodeSyntaxPreviewView.swift`、`infra/ttmedia/Sources/TTMediaKit/UI/Spreadsheet/SpreadsheetTablePreviewView.swift`、`infra/ttmedia/Sources/TTMediaKit/UI/Audio/UnifiedAudioPlayerView.swift`、`infra/ttmedia/Tests/TTMediaKitTests/AudioAuthenticityAndLoadingTests.swift`、`infra/ttmedia/Tests/TTMediaKitTests/PreviewResourceLifecycleAuditTests.swift`、`infra/ttmedia/Tests/TTMediaKitTests/TextAndSpreadsheetPerformanceTests.swift` 等。

## MHTML preview follow-up (2026-10-24)

- [x] R014-MHTML Restore default rendered saved-page preview under [the bounded offline contract](contracts/mhtml-preview.md); validate MIME decoding, original-source retention and actual WebKit rendering.
