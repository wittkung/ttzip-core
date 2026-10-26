# Implementation Plan: Unified AVSampleBuffer Media Pipeline & Hermetic Demuxing Sandbox (Feature 230)

**Feature**: `230-unified-avsamplebuffer-media-pipeline`

**Status**: Reopened for playback and isolation verification (2026-10-16)

**Architecture Classification**: Full SDD

---

## 1. Technical Context & Strategy

### 1.1 Root Problem
TTZip currently links directly against `/opt/homebrew/opt/mpv/lib/libmpv.2.dylib` and 20+ transitive Homebrew dynamic libraries. On any clean macOS machine without Homebrew, the application crashes immediately upon launch (Issue P2). In addition, running `libmpv` in-process incurs JIT/W^X memory allocations that violate Apple Hardened Runtime, maintains a redundant audio delay compensation engine, and risks patent exposure through included software decoders.

### 1.2 Target Architecture
1. **Hermetic Compilation**: Bazel 8 `rules_foreign_cc` builds minimal whitelist `libavformat` and `libdav1d` libraries. No host dynamic libraries are referenced.
2. **Orthogonal Demux/Decode Partitioning**:
   - `ttmedia-worker` (isolated App Sandbox process) exclusively performs pure container demuxing and Annex-B to AVCC translation.
   - Patented streams (H.264, HEVC, ProRes, AAC) are transferred over XPC as compressed NALU packets directly to the host for Apple Silicon hardware decoding via VideoToolbox.
   - Open streams (AV1 on non-M3+, VP9, Opus, FLAC) are decoded within the worker and delivered as `IOSurface` handles or linear PCM blocks.
3. **Unified Presentation Engine**:
   - Video: `AVSampleBufferDisplayLayer` (ASBDL).
   - Audio: `AVSampleBufferAudioRenderer`.
   - Master Clock: `AVSampleBufferRenderSynchronizer` (`CMTimebase` SSOT).
4. **Physical Decommission of MPV**:
   - Complete removal of `third_party/mpv` and `infra/ttmpv`.

---

## 2. Constitution & Invariants Evaluation

以下表格保留历史计划记录；当前实现与本轮证据以文末 2026-10-16 决定、tasks.md 验证结果和媒体孪生第 6 节为准，不将历史 PASS 视为本轮测量。

| Invariant | Status | Mechanism |
| :--- | :--- | :--- |
| **Zero External Host Dylibs** | **PASS** | Bazel `rules_foreign_cc` hermetic toolchains produce self-contained libraries with `@rpath`. `otool -L` validated. |
| **Zero Patent Exposure** | **PASS** | FFmpeg built with `--disable-decoders`; software decoders for patented codecs completely excised. |
| **Single Master Clock (ADR-017)** | **PASS** | `AVSampleBufferRenderSynchronizer` governs timebase. Dynamic audio delays forbidden. |
| **800 LOC Gate** | **PASS** | All new source files strictly conform to $\le 800$ LOC, functions $\le 80$ LOC. |
| **Zero Compile Warnings** | **PASS** | Verified under `cargo clippy -- -D warnings` and Swift `-warnings-as-errors`. |

---

## 3. Implementation Waves

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│                         Feature 230 Implementation Waves                                │
├──────────────────────────────────────────────────────────────────────────────────────────┤
│ Wave 1: Hermetic C Dependencies (T001-T005)                                              │
│ - Bazel rules_foreign_cc configuration for FFmpeg (libavformat) and libdav1d             │
│ - Minimal whitelist demuxer flags, hidden visibility, zero Homebrew linkages             │
├──────────────────────────────────────────────────────────────────────────────────────────┤
│ Wave 2: Sandboxed ttmedia-worker & Demuxing Microkernel (T006-T012)                      │
│ - ttmedia-worker XPC service target, App Sandbox entitlements                            │
│ - libavformat C-ABI binding, container parsing, Annex-B -> AVCC in-place translation     │
├──────────────────────────────────────────────────────────────────────────────────────────┤
│ Wave 3: Host AVSampleBuffer Presentation Stack (T013-T018)                               │
│ - Video: AVSampleBufferDisplayLayer direct rendering                                     │
│ - Audio: AVSampleBufferAudioRenderer integration                                         │
│ - Clock: AVSampleBufferRenderSynchronizer master timebase and seek preroll               │
├──────────────────────────────────────────────────────────────────────────────────────────┤
│ Wave 4: Open Codec Pipeline (T019-T024)                                                  │
│ - AV1 hardware decode on M3+; fallback software decode via libdav1d                      │
│ - Zero-copy IOSurface frame transfer across XPC for software-rendered frames             │
├──────────────────────────────────────────────────────────────────────────────────────────┤
│ Wave 5: Two-Tier Sleep & Fault Recovery (T025-T028)                                      │
│ - 30s warm standby (demuxer release) & 300s cold termination (0 MB memory)               │
│ - XPC invalidation handler and seamless UI error recovery                                │
├──────────────────────────────────────────────────────────────────────────────────────────┤
│ Wave 6: Atomic MPV Decommission (T029-T034)                                              │
│ - Physical deletion of third_party/mpv and infra/ttmpv                                   │
│ - Removal of CTTMpvBridge, cleanup of apple/BUILD.bazel and settings references          │
├──────────────────────────────────────────────────────────────────────────────────────────┤
│ Wave 7: Polish & Performance Benchmarking (T035-T040)                                    │
│ - Automated otool -L verification in CI (SC-001)                                         │
│ - Lip-sync precision and first-frame latency benchmarks (SC-002, SC-003)                 │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Risk Mitigation & Failsafes

1. **Bazel foreign_cc Caching**: Heavy C builds (FFmpeg) will leverage the existing NativeLink CAS and local disk CAS (`~/.cache/bazel-disk-cache`) to ensure compilation runs once and is cached immutably.
2. **CoreMedia Compatibility**: VideoToolbox hardware decode failure on corrupted streams triggers graceful inline player error handling rather than crashing the host process.
3. **Sandboxed Worker Death**: If the auxiliary worker is killed by memory pressure or SIGKILL, the host regenerates the XPC connection on next user play action.

## 2026-10-16 修复决定与验证边界

- 本轮不重新引入 FFmpeg/dav1d，不将历史 PASS 表作为当前签名包或性能测量证据。
- 应用中的非原生媒体使用 `TTMediaWorker.xpc`（Swift NSXPC 服务外壳 + 现有 Rust 解复用静态库），由 `macos_xpc_service` 构建并经 `TTZipApp.xpc_services` 嵌入；旧 Rust framed worker 保留独立工具与生命周期测试用途。
- XPC `openFileMedia` 消息携带只读 FileHandle 能力；服务必须验证句柄为 O_RDONLY 并持有文件能力，关闭会话/连接时释放；拒绝使用裸路径绕过宿主授权。该消息不是旧 JSON schema 的字段扩展。
- 正式 `.app` 默认使用 bundled XPC transport；CLI/测试进程没有应用服务容器，因此使用 DirectTransport。正式应用不在 XPC 失败时静默回退主进程。
- 播放主时钟与输入状态分离：EOF 不等于呈现结束；seek 必须隔绝旧供包并重建新供包；错误必须到达 UI；取消和过期回调不能覆盖新选择。
- 验收依据见同目录 tasks.md 的 R001–R006 以及媒体孪生第 6 节。服务构建成功不等于正式签名包中的用户选取外部文件、崩溃恢复、空闲回收已通过。

- 真实沙盒烟测确认：宿主与服务通过只读 FileHandle 跨进程打开/读包/关闭；服务无需用户目录或书签 entitlement。显式持久安全书签的跨进程尝试已失败并弃用；隐式书签变更被自动审批拒绝，未采用。采用的 FD 方案权限更窄，依据 https://developer.apple.com/forums/thread/785735 。

## 2026-10-16 实机反馈的设计修订（先于实现）

- 播放生命周期以 owner/token 为取消边界，先补旧视图取消新会话的竞态回归；呈现模式与会话资源不能继续隐式绑定。全窗口交接保留完整播放状态，并显式呈现加载/失败。HEVC/DTS-HD 素材兼容性另验，不把黑屏一概归因于 SwiftUI。
- 显隐逻辑收敛为 TTMedia 的可观察状态，由沉浸宿主通过环境注入；播放器与顶部/导航读取同一实例。保留非视频浏览器的行为边界，不用全局无类型通知同步透明度。
- 外挂字幕增加 Rust 时间轴模型与有界解析，复用现有桥接机制后由 Swift 文件选择器和主时钟驱动叠字；不以添加按钮替代实际加载/渲染。
- 修改实施前先更新本 spec、tasks 与媒体孪生。证据状态区分“设计已约定”“代码已实现”“自动测试通过”“实机验收通过”。

### 状态交接方案被实机反证，改为持久会话

废弃“离场保存/新视图恢复快照”作为全窗口连续性的实现方案：动画、异步加载和中间预览视图会让其消费/覆盖顺序不可靠。由 TTMedia 主线程会话仓管理单个活动文件的可观察会话，controller 与字幕状态由会话持有，视图只领取带 owner 的呈现租约。首次启动加载任务由会话所有，不能随旧 SwiftUI task 取消。将显示层挂到新 viewport 时，旧 viewport 的布局不能再改动该层。

字幕目录与用户选择同属此会话。补同目录有界发现与父目录默认定位；内封字幕需要 Rust 元数据/包/协议统一演进及真实多轨素材验证，不允许用信息列表假装轨道可用。

### i18n 接线修订

复用 infra/tti18n 的类型化 key、线程安全 manager 与 observable LocalizationState。共享媒体词条提供无宿主目录时的内建后备，宿主注册目录可覆盖词条；不注册第二个全局 lookup handler。TTZip AppLocalizationState 作为既有设置入口，向共享状态单向同步。语言依赖只进入 UI 文案，不能进入播放 session identity。

## 2026-10-17 字幕容错解复用、IINA 目录发现、后台分阶段预热与样式系统设计修订（先于代码对齐）

1. **容错字幕流解析与安全有界 `read_at` 容器提取（Rust `subtitles.rs` + `container_subtitles.rs`）**：
   - 将 SRT/WebVTT 解析改为逐行流式状态机（兼容正文内空行、WebVTT `NOTE`/`STYLE` 块与坐标参数），新增 TTML/DFXP（`<p begin="..." end="...">` 与 `<br/>` 换行；**默认保留原始时间码，禁止自动猜测并减去 `3600s` 一小时偏移**）及 ASS/SSA（`[Events]` `Dialogue:`）解析，支持 UTF-8 / UTF-16LE / UTF-16BE BOM；
   - 容错时间轴收敛（参照 FFmpeg `ff_subtitles_queue_finalize`）：自动跳过空文本块，对 DaVinci/WhisperX 导出的零时长块（`end == start`，即触发 `Subtitle cue end must follow start` 的根因）自动利用下一条 cue 起点或有界默认时长（`0.8s`~`1.5s`）修复结束时间，对个别倒置块跳过，仅当整份文件无任何有效 cue 时报错；
    - **安全读取前提、淘汰 `mmap`、严格区分用户主动完整加载与后台预热预算、编码边界校验、完整性与请求级 `register -> cancel -> complete -> reclaim` 原生取消契约**：由于“本进程只读打开”无法阻止外部进程修改或截断影片，使用 `memmap2::Mmap` 违反 Rust 内存安全前提（外部截断会触发 `SIGBUS` 或 UB）。`container_subtitles.rs` 彻底移除 `memmap2` 与 `unsafe`，改用 64 KiB `FileExt::read_at` 窗口缓存（`HeaderCache`），令 `vint()`、栈上 `uint()` 及短负载统一走缓存窗口读取，大视频帧直接移动偏移跳过；文件在扫描期间若被截断，安全返回可恢复的 `UnexpectedEof` 错误。容器扫描通过 `ttmedia_subtitles_container_request(path, track, request_id)`、`ttmedia_subtitles_register_request(request_id)`、`ttmedia_subtitles_cancel_request(request_id)` 与 `ttmedia_subtitles_reclaim_request(request_id)` 严格区分三类请求：（a）`track = -1` 仅读 `Tracks` 目录；（b）`track > 0` 仅用于**用户主动完整加载或初始默认轨激活**，不设 4 秒预热时间截断（`deadline: None`），但全程在扫描启动前、每个 EBML 元素与每个 `Cluster` 块边界检查按 `request_id` 隔离的取消状态 `is_request_cancelled(request_id)`（被取消时返回 `ErrorKind::Interrupted`）；（c）`track = 0` 或编码请求 `track <= -2`（`SubtitlePrewarmRequestCodec` / `try_encode_prewarm_request` / `decode_container_request`）用于**后台有界预热**，强制携带外挂预热后的**剩余时间预算 `budget_ms`**（上限 4000ms，绝不重置获得新的 4 秒预算）、`max_tracks`（上限 8）以及仅剩单槽位/单轨时的 `target_track`，**严禁在只剩 1 条预热轨道时通过 `track > 0` 绕过预热预算**；编码与解码严格校验边界：通过 `track.checked_neg().ok_or_else(...)` 拒绝 `Int64.min`（`i64::MIN`）并返回 `InvalidInput` 非法输入错误（绝不回退为批量预热），拒绝保留高位非零，并对 `<= 0` 或 `> UInt32.max`（如 `(1 << 32) + 3`）的轨道 ID 返回 `nil` / `None`，严禁静默截断到另一条轨道。`SubtitleRequestTable` 将已注册（含尚未开始扫描 `in_flight_scans == 0`）或正在扫描的活跃请求保留在 `active_entries`（上限 `MAX_ACTIVE_REQUESTS = 4096`）中，取消已注册未启动请求时在原地置 `cancelled = true` 而绝不降级到有界墓碑表（因此即使期间发生超过 4096 次其他取消，迟到启动时仍必定识别取消并拒绝执行）；容量达到 4096 时显式拒绝新请求而不遗忘已有请求，并在 `ActiveRequestGuard::drop` / `reclaim_request` 时归档至 256 槽有界完成表、分别导出 `active` / `pre_start_tombstone` / `completed_record` / `pending_delay` 四类计数器。预热扫描仅当整段 `Segment` 完整扫描至末尾时返回 `{"tracks": [...], "complete": true, "trackCues": {...}}`，中途超时或该 `request_id` 被取消时立即返回 `{"tracks": [...], "complete": false, "trackCues": {}}`。扩展支持 `S_TEXT/UTF8`、`S_TEXT/ASCII`、`S_TEXT/WEBVTT`、`S_TEXT/ASS`、`S_TEXT/SSA`（含 `BlockGroup` 与 `SimpleBlock`），统一由 `clean_subtitle_text` 剥离 ASS 覆写标签（`{\...}`）、基础 HTML 标签与转义换行（`\N`）。
2. **仿 IINA 同目录发现、分阶段后台预热、完整性门禁、请求级生命周期回收与跨 PID 独立 Worker 取消验证（Swift `VideoPlaybackSession` + `MediaSubtitleTransport` + `MediaWorkerXPCClient` + `MediaWorkerService`）**：
   - `ExternalSubtitleTrack` 支持 `.srt`/`.vtt`/`.ttml`/`.dfxp`/`.ass`/`.ssa` 与传统编码（`GB18030`、`Big5`、`UTF-16`、`WindowsCP1252`）转码 UTF-8 后备，构建时在后台线程排序 `cues` 并预计算 `maxEndPrefix` 与跳跃前驱索引 `prevHigherEndIndex`；`text(at: seconds, delay:)` 先以 $O(\log N)$ 二分定位 `start <= target` 上界，再跳表收集活动重叠块（常规 $O(\log N + k)$，极端全片重叠块最坏 $O(N)$）；
   - 对标 IINA `AutoFileMatcher` 与 `QuickSettingViewController`，将同目录字幕分为自动匹配伴随轨（`companionSubtitles`）与同目录可用轨（`directorySubtitles`），统一呈现在字幕菜单与快设面板中；
   - **默认字幕优先、按剩余预算有界预热、完整性校验、请求级 `register -> cancel -> complete -> reclaim` 全链路回收与三级 XPC 证据分类（零主线程解码阻塞）**：发现阶段先快速获取内封轨道目录（`track = -1`）与同目录文件列表，**优先在结构化 `nonisolated` 后台函数中解码并构建首选默认字幕轨（外挂伴随首轨或内封首选轨）使其立即可用**；随后启动可取消的后台预热任务（`subtitlePrewarmTask` + `subtitlePrewarmGeneration`），外挂与内封共享 4.0s 协作式软预算与 8 轨上限。外挂队列完成后，`prewarmEmbeddedBatch` 按 `deadline - CFAbsoluteTimeGetCurrent()` 计算**剩余预算 `remainingBudgetMs`**（不重新获得 4 秒），即便只剩 1 个预热名额或 1 条未缓存内封轨也统一通过 `encodePrewarmContainerRequest`（`track <= -2`）走预热通道，并由 `decodeBatchTracksOffMainActor` 强制校验 `response.complete == true` 才写入 `subtitleCache`。取消机制采用请求级身份（`activePrewarmRequestID`、`activePrimaryRequestID`、`activeSecondaryRequestID`、`activeDefaultEmbeddedRequestID`）并配对 `SubtitleSessionRequestRegistry` 的 `allocateRequestID -> beginRequest -> cancelRequest -> finishRequest` 回收生命周期：已注册请求（含尚未启动请求）与运行中请求常驻 `liveRequests`（容量上限 `4096`，移除 `unstartedOrder` 静默淘汰，取消时在原地置 `isCancelled = true` 而非移入墓碑表；超过 4096 次其他取消后迟到启动仍必定拒绝，容量满时显式拒绝新请求），完成时通过 `finishRequest` 确定性清理 `liveRequests`、`sessionRequests` 与 Rust 状态；关闭旧会话（`stop()` / `closeMedia`）仅取消并清理该会话请求而不影响新会话；`TTMediaWorkerProtocol` 提供 `subtitles(forSession:track:requestID:reply:)`、`cancelSubtitle(forSession:requestID:reply:)` 与 `subtitleRequestStatus(forSession:requestID:reply:)`，并严格区分进程内匿名 `NSXPCListener.anonymous()` 接口往返验证（`workerPid == getpid()`）与**不同 PID 的独立 Worker 进程验证（`workerPid != getpid()`）**，确保独立 Worker 进程中主动单轨扫描被取消时停在第 1 个 Cluster、并发另一单轨继续完成且状态经 IPC 返回；
   - **全选择入口统一取消与缓存命中切轨竞态修复**：`loadExternalSubtitle`、`selectSubtitle` 与 `selectSecondarySubtitle` 在检查 `"off"` 或 `subtitleCache[id]` 命中之前，统一调用 `beginPrimarySubtitleSelection()` / `beginSecondarySubtitleSelection()` 递增 generation 并取消对应的上一主/副请求及后台预热请求，防止“未缓存轨 A 正在加载时切换至已缓存轨 B，A 完成后覆盖 B”；移除 `AVSampleBufferVideoPlayerView` 的 `isSubtitleLoading` 进度条遮罩，改为右上角 OSD 胶囊提示，并打通播放器视口拖拽加载外挂字幕（`onDropFiles`）；
   - `VideoPlaybackSession` 在时钟推进、seek、切轨或调整延迟时去重更新 `activeSubtitleText` / `activeSecondarySubtitleText`，避免 30Hz 无变化帧触发 SwiftUI 文本重排。
3. **仿 IINA 字幕样式配置、固定字幕位置与移除重复中央按钮（`SubtitleStyleConfiguration` + `SubtitleOverlayView` + `AVSampleBufferVideoPlayerView`）**：
   - 新增 `SubtitleStyleConfiguration`（`UserDefaults` 持久化），支持字号（14–48pt）、缩放倍率（0.5x–2.0x）、粗体/斜体、文本颜色预设、描边宽度与颜色、背景不透明度、垂直位置（`0...100`）与时间延迟（`-10s...+10s`），并在紧凑视口（`width < 380pt`）下自动按比例缩放字号；
   - **解除 `SubtitleOverlayView.bottomPadding` 与 `controlsVisible` 的绑定**，确保鼠标移入/移出引发控制栏显隐变化时字幕位置恒定不变；
   - **移除 `< 380pt` 视口的中央播放按钮 `compactCenterPlayButton`**，消除与底栏播放键的重复。
4. **播放生命周期、全窗口呈现权优先级、双视口异常恢复重绑与证据边界收尾（`VideoPlaybackSession` + `VideoPlaybackSessions` + `AVSampleBufferPlayerView` + `AVSampleBufferVideoPlayerView`）**：
   - **区分已修复的代码缺陷与待实机确认的故障原因**：明确记录 `/Applications/TTZip.app` 旧产物不一致证据，以及代码层修复的多视口租约、呈现权优先级、暂停首帧锚定、Worker `inputExhausted` 暂停续播防归零、异常恢复字幕与双视口重绑；不把历史所有黑屏笼统归结为单一竞态，进程内 `DirectTransport` 往返测试仅证明会话状态连续性，不冒充应用窗口与 XPC 跨进程验收；
   - **重复注册幂等性与全窗口呈现优先级**：`VideoPlaybackSession.acquireViewport` 对已在 `activeViewports` 内的 `viewportID` 执行幂等处理（不重排 `viewportOrder`、不抢回 `activePresenterViewportID`），并强制执行全窗口优先级——只要存在活跃的 `isFullScreen == true` 视口，任何 `isFullScreen == false`（Inspector）的延迟 `.task` 或重注册均不得夺走 `activePresenterViewportID`；仅 `isPresenting == true` 的唯一呈现视口允许挂载与布局 `displayLayer`；
   - **双视口存活时的会话重建自动重绑与绑定代际清理**：`VideoPlaybackSessions` 标记为 `@Observable` 并维护带 `(sessionID, generation)` 身份的 `ViewportBinding`；当 `claim(..., restart: true)` 重建新会话时，保留所有存活视口及其 `isFullScreen` 标志，同步推进绑定 `sessionID`/`generation` 并触发所有存活视口将 `@State playback` 重绑至新会话；当 `release(sessionID:viewportID:)` 命中旧 `sessionID` 分支时，通过 `unbindViewportIfMatching` 清理仍归属该旧会话的过期绑定（防止 A→B 切换视频后旧视图消失泄漏闭包），同时保证旧 `release` 不会删除同一 `viewportID` 已升级到新会话的新绑定；
   - **异常恢复完整迁移主/副字幕内容、向新 Controller 应用音量/静音/倍速，以及显式 seek 到 0 的 `lastKnownTime` 同步**：`restorePreservedContext(from:)` 完整迁移 `volume`、`isMuted`、`previousVolume`、`playbackRate`、`subtitleCache`、`companionSubtitles`、`directorySubtitles`、`preservedExternalSubtitles`（含任意目录手动加载的外挂字幕）、`embeddedSubtitles`、已激活的 `subtitleTrack` / `secondarySubtitleTrack` 及自定义 loader；`setupPlayback` 与 `finalizeLoadedController` 调用 `applyPlaybackSettings(to:)` 将保存的音量、静音与倍速真正写入新 `VideoPlaybackController`（不再被新 controller 默认值覆盖）；发现完成后自动激活已选中主/副轨道与当前时间点文本；同时通过 `isTearingDown` / `isSettingUpPlayback` 守卫使显式 `seek(to: 0)` 将 `lastKnownTime` 归零，避免恢复时跳回旧正数进度。


