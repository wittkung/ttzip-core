# Feature Specification: Unified AVSampleBuffer Media Pipeline & Hermetic Demuxing Sandbox (ADR-017 Stages 3–5)

**Feature Directory**: `products/ttzip/core/specs/230-unified-avsamplebuffer-media-pipeline`

**Created**: 2026-10-04

**Status**: Draft

**Pipeline Tier**: [Full SDD] (introduces XPC IPC protocols between host and ttmedia-worker, C-ABI demuxer boundary, and hermetic C/C++ third-party library closures via Bazel)

**Input**: The desktop application currently links directly against Homebrew's `/opt/homebrew/opt/mpv/lib/libmpv.2.dylib` and 20+ transitive dylibs. As a result, the application crashes immediately upon launch on any macOS machine without a matching Homebrew installation (Issue P2). Furthermore, retaining `libmpv` introduces Hardened Runtime JIT/W^X violations, dual-clock drift, and patent exposure. This feature executes Stages 3, 4, and 5 of ADR-017: building hermetic whitelist `libavformat` and `libdav1d` libraries under Bazel, implementing the sandboxed `ttmedia-worker` XPC process for pure demuxing and open-codec decoding, feeding compressed `CMSampleBuffer` DTOs directly to the host's `AVSampleBufferDisplayLayer` and `AVSampleBufferAudioRenderer` synchronized by `AVSampleBufferRenderSynchronizer`, and completely eliminating `libmpv` and `infra/ttmpv`.

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Instant Crash-Free Media Preview on Clean macOS (Priority: P1) 🎯 MVP

A user downloads TTZip onto a factory-clean Mac running macOS 14.0+ without Homebrew or developer tools installed. When selecting or space-bar previewing any supported media file (MP4, MOV, MKV, WebM, AVI, FLV, TS, MP3, AAC, FLAC, Ogg, WAV), the media begins playing smoothly within 200 ms with zero crashes, zero external dylib search failures, and zero Gatekeeper / Hardened Runtime rejections.

**Why this priority**: Linking against `/opt/homebrew/...` makes the application completely undeployable to end users. It is a critical release blocker.

**Independent Test**: Build the application via Bazel; audit with `otool -L` and verify zero references to `/opt/homebrew` or `/usr/local`; launch on a clean VM or test environment lacking Homebrew and assert successful playback of MKV, MP4, and WebM files.

**Acceptance Scenarios**:

1. **Given** a macOS machine with no Homebrew installed, **When** the user launches TTZip and opens an archive containing `video.mkv` and `clip.mp4`, **Then** the application launches cleanly and renders both files without crashing or missing dynamic library errors.
2. **Given** `otool -L` executed against `TTZip`, `ttmedia-worker`, and all embedded app extensions, **Then** all dynamic library dependencies point exclusively to `@rpath`, `/System/Library/`, or `/usr/lib/`.
3. **Given** the app sandbox enabled with Hardened Runtime Library Validation, **When** media playback commences, **Then** no sandbox violations or JIT memory allocation crashes occur.

---

### User Story 2 - Unified Hardware Acceleration & Master Clock Presentation (Priority: P1)

A user plays a 4K 60fps HEVC or H.264 video with AAC or ALAC audio. Playback uses Apple Silicon hardware video decoding (`VTDecoderXPCService`) directly feeding `AVSampleBufferDisplayLayer` (ASBDL) and hardware CoreAudio output via `AVSampleBufferAudioRenderer`. Playback clock is strictly governed by `AVSampleBufferRenderSynchronizer` (`CMTimebase`). Audio and video remain lip-synced within ATSC IS-191 tolerances ($[-15\text{ms}, +45\text{ms}]$) with zero runtime dynamic audio delay distortion, full EDR/HDR tone mapping, and seamless macOS Picture-in-Picture (PiP).

**Why this priority**: Eliminates dual-clock jitter and CPU software decoding overhead while maintaining royalty immunity through Apple Silicon hardware decoder patent exhaustion.

**Independent Test**: Play a 4K 60fps HDR video sample; measure CPU usage (assert $\le 5\%$ CPU utilization on M-series Apple Silicon), inspect Metal rendering (0 GPU CPU blits), verify Picture-in-Picture button functions and continues playing smoothly in floating window.

**Acceptance Scenarios**:

1. **Given** an MP4 or MKV container containing H.264 or HEVC video, **When** playing in TTZip, **Then** video packets are delivered as compressed `CMSampleBuffer` instances directly into `AVSampleBufferDisplayLayer`, executing on the hardware decoder.
2. **Given** active media playback, **When** the user activates the macOS native Picture-in-Picture button, **Then** playback transitions into the system floating PiP window without interruption.
3. **Given** a seeking operation across the timeline, **When** the user jumps to a non-keyframe time $T$, **Then** preroll frames from the preceding keyframe are decoded with `kCMSampleAttachmentKey_DoNotDisplay = true` so that target frame $T$ renders cleanly without visual artifacting or corruption.

---

### User Story 3 - Sandboxed Isolation & Energy-Efficient Worker Lifecycle (Priority: P2)

Non-Apple containers (MKV, WebM, AVI, FLV, TS) and open codecs (AV1, VP9, Opus, Vorbis, FLAC) are parsed and processed in an isolated XPC auxiliary process (`ttmedia-worker`). On Apple Silicon M3/M4/M5, AV1 is dispatched to hardware decoding; on M1/M2 or hardware fallback, AV1 is decoded via hermetic `libdav1d`. The worker operates under a strict Two-Tier Sleep lifecycle: entering warm standby after 30 seconds of idle time and terminating cleanly (SIGTERM) after 5 minutes of inactivity, dropping idle memory consumption to exactly 0 MB.

**Why this priority**: Malformed or hostile media files cannot compromise the host application process, and background workers do not drain battery when media playback is not active.

**Independent Test**: Feed a fuzz-corrupted MKV file; assert the worker handles the malformed input gracefully or restarts without crashing the host app; verify with `ps` that after 5 minutes of closing the media preview, `ttmedia-worker` terminates and frees all memory.

**Acceptance Scenarios**:

1. **Given** an MKV or WebM file with open audio (Opus / FLAC), **When** opened, **Then** `ttmedia-worker` parses the container via `libavformat` and streams sample buffers to the host.
2. **Given** a playback session stops, **When** 30 seconds elapse without new playback, **Then** decoding resources are released while worker remains warm.
3. **Given** 5 minutes elapse without new playback, **When** inspecting system processes, **Then** `ttmedia-worker` has cleanly exited and host memory is reclaimed.

---

### User Story 4 - Complete Physical Decommission of libmpv & Clean Monorepo (Priority: P2)

All historical references, C-ABI bindings, submodules, and static/dynamic library artifacts of `libmpv` and `infra/ttmpv` are deleted from the codebase. Build definitions (`BUILD.bazel`, `Package.swift`) and CI lint checks enforce that no mpv headers, symbols, or bridge files exist in the repository.

**Why this priority**: Eliminates technical debt, closes orphan audit items, and prevents accidental re-introduction of legacy playback pathways.

**Independent Test**: Run `git grep -n "libmpv\|ttmpv"` across source trees and verify zero live references; execute full Bazel build and test suites to confirm clean compilation and zero link-time warnings.

**Acceptance Scenarios**:

1. **Given** the monorepo root, **When** searching for `libmpv` and `CTTMpvBridge`, **Then** no active source code, headers, or build target dependencies remain.
2. **Given** `tools/ci/lint_code_quality.py`, **When** executed, **Then** static anti-corruption gates pass with 0 forbidden legacy playback symbols.

---

## Requirements Matrix *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST universally demux containers via hermetic `libavformat` strictly adhering to a compile-time whitelist (`mov,matroska,webm,avi,flv,mpegts,mp3,aac,flac,ogg,wav`), with all other protocols and demuxers explicitly disabled.
- **FR-002**: For patented video codecs (H.264, HEVC, ProRes) and audio codecs (AAC), the worker MUST NOT include or invoke software decoders; it MUST package extracted compressed packets into CoreMedia format descriptions and timing DTOs for host VideoToolbox hardware decoding.
- **FR-003**: For MPEG-TS containers with H.264/HEVC, the worker MUST perform in-place Annex-B to AVCC/HVCC conversion (replacing `00 00 00 01` start codes with 4-byte big-endian length prefixes) prior to delivery to the host.
- **FR-004**: For open video codecs (AV1, VP9), the pipeline MUST attempt hardware acceleration on supported Apple Silicon (M3+) and fall back to hermetic `libdav1d` software decoding delivering `CVPixelBuffer` / `IOSurface` frames to the host.
- **FR-005**: All media presentation in the host UI MUST be driven by `AVSampleBufferDisplayLayer` and `AVSampleBufferAudioRenderer`, with master clock and synchronization governed exclusively by `AVSampleBufferRenderSynchronizer` (`CMTimebase` SSOT).
- **FR-006**: XPC communication between the host and `ttmedia-worker` MUST deliver media packets via optimized binary serialization or Mach ports with bounded ring buffer back-pressure.
- **FR-007**: Preroll and random-access seeking MUST start from the keyframe immediately preceding the target timestamp $T$, marking preroll frames with `kCMSampleAttachmentKey_DoNotDisplay = kCFBooleanTrue` to eliminate visual artifacts.
- **FR-008**: The `ttmedia-worker` MUST implement a two-tier idle sleep state machine: Level 1 Warm Standby (30s timeout, release active decode sessions) and Level 2 Cold Hibernation (300s timeout, clean process termination).
- **FR-009**: All legacy `libmpv` symbols, headers, and targets in `infra/ttmpv` and `third_party/mpv` MUST be completely deleted from the monorepo.

---

### Non-Functional & Systems Requirements

- **NFR-001**: Single-file LOC hard gate $\le 800$ LOC, function LOC $\le 80$ LOC, cyclomatic complexity $\le 15$.
- **NFR-002**: Zero external package manager runtime dependencies (zero Homebrew dylibs in release build closure).
- **NFR-003**: Strict Swift 6 concurrency compliance across all UI and coordinator code (`Sendable`, `@Observable`, no unchecked data races).
- **NFR-004**: Zero compiler warnings under `cargo clippy -- -D warnings` and Swift `-warnings-as-errors`.

---

## Success Criteria *(mandatory)*

- **SC-001 (Zero Homebrew Linkage)**: `otool -L` on `TTZip` binary and all helper bundles exhibits 0 lines containing `/opt/homebrew` or `/usr/local`.
- **SC-002 (First-Frame Latency)**: Cold media preview of MP4, MOV, and MKV files reaches first visible frame within $\le 200\text{ ms}$ on Apple Silicon M-series hardware.
- **SC-003 (Lip-Sync Precision)**: Audio and video synchronization drift remains strictly within $[-15\text{ ms}, +45\text{ ms}]$ over 30 minutes of continuous playback.
- **SC-004 (Zero Idle Memory Leak)**: 5 minutes after closing media preview, `ttmedia-worker` process exits and releases 100% of allocated memory (0 MB resident memory).
- **SC-005 (100% MPV Decommission)**: Full repository audit confirms 0 references to `libmpv.dylib`, `CTTMpvBridge`, or `infra/ttmpv` in active build configurations.

---

## Clarifications & Architectural Decisions *(clarify phase)*

### Clarification 1: Incremental Vertical Spike Priority (Vertical Spike First)
- **Decision**: The implementation strictly follows the *Vertical Spike Invariant*. Wave 1 proves the end-to-end pipeline: hermetic `rules_foreign_cc` build of whitelist `libavformat` -> `ttmedia-worker` demuxing MKV (H.264/HEVC) -> XPC DTO delivery -> host `AVSampleBufferDisplayLayer` hardware rendering. Wave 2 layers audio (`AVSampleBufferAudioRenderer` + `CMTimebase` synchronization), Wave 3 layers open codecs (`libdav1d` AV1 / VP9 / Opus), and Wave 4 performs the atomic deletion of `infra/ttmpv`.
- **Rationale**: Eliminates theoretical abstraction speculation and proves the hardest physical boundary (Bazel C-library hermetic build and XPC CoreMedia frame reconstruction) before expanding codec breadth.

### Clarification 2: XPC Inter-Process Frame Transfer Mechanism
- **Decision**:
  1. *Patented compressed streams (H.264, HEVC, ProRes, AAC)*: Transferred as raw NALU payload bytes (`NSData`) with `CMSampleTimingInfo` and extradata parameter sets (SPS/PPS/VPS). Host reconstructs `CMSampleBuffer` via `CMSampleBufferCreateReady` with zero IPC copy overhead for hardware decode.
  2. *Software-decoded open video frames (AV1 fallback, VP9)*: Worker renders into an `IOSurface` backing `CVPixelBuffer`. The `IOSurfaceID` handle is transferred across XPC with zero CPU blit, bound directly to host `CALayer.contents` or ASBDL.
  3. *Uncompressed audio (PCM, decoded Opus/FLAC)*: Transferred across XPC as aligned audio sample blocks and fed directly into `AVSampleBufferAudioRenderer`.
- **Rationale**: Strict alignment with Apple Silicon unified memory (UMA) optical model; avoids copying high-bitrate raw video frames across process boundaries.

### Clarification 3: Hermetic Bazel Build Strategy for FFmpeg & dav1d
- **Decision**: `libavformat` and `libdav1d` are hermetically compiled via Bazel 8 `rules_foreign_cc` under `//third_party/ffmpeg` and `//third_party/dav1d`. Release outputs are compiled with `-Dvisibility=hidden` and pure whitelist flags, producing static or private dynamic libraries with absolute path independence (`@rpath`). Zero system `/opt/homebrew` dependencies are permitted.
- **Rationale**: Directly resolves Issue P2 (crash on clean macOS machines) and guarantees reproducible, hermetic CI builds.

### Clarification 4: Atomic MPV Decommission (No Dual-Path Technical Debt)
- **Decision**: In Stage 5, `infra/ttmpv` and `third_party/mpv` will be completely deleted in a single atomic commit. No dynamic fallback toggle or parallel legacy engine will be retained in production.
- **Rationale**: Enforces the *Zero-Debt First Strike* invariant. Dual playback engines cause testing divergence and maintenance hazards.

## 2026-10-16 实机反馈：呈现切换、字幕与统一控件（重新打开验收）

### 现象与证据边界

用户实机反馈：从预览进入全窗口后视频黑屏、时间显示 00:00；外挂字幕按钮不可见；顶部文件信息栏比底部控制栏更晚隐藏。示例文件名提示 MKV/HEVC 10-bit/DTS-HD，但文件名不能证明实际编码，也不能证明黑屏只由窗口切换导致。须分开验证普通 H.264/AAC 与该类素材；不得把通用生命周期修复当成 HEVC/DTS-HD 兼容性完成。

### 分类与修订后的约束

1. **播放会话：设计约束不完整，并有实现竞态。** 当前全窗口入口创建新的播放器视图，播放状态归各视图持有；共享 generation 管理器的无条件 cancel 允许旧视图离场取消新视图。要求：会话有独立 owner/token；过期 owner 只能释放自己的资源；同一文件呈现切换保留时间、播放/暂停、音量、倍速和外挂字幕选择。不得因布局改变重置为 00:00 或静默黑屏。若交接需要重新加载，必须明确显示 loading/failed，并确保只有一个可听会话。
2. **外挂字幕：功能设计与接线缺口。** 当前 AVSampleBuffer 控件没有字幕入口，包类型含 subtitle 不代表完成字幕播放。要求：在紧凑/宽屏模式均提供可发现的字幕菜单，允许用户选择授权的外挂文件、关闭及替换；以播放主时钟更新字幕，暂停/seek/切换呈现仍一致。首批 SRT/WebVTT 的时间轴解析放在 Rust，Swift 仅负责系统文件授权和排版；ASS/SSA 样式、内封字幕和 PiP 字幕均须独立验收，不默认承诺。损坏、超限或不支持格式必须明确报错，不能只增加无效按钮。
3. **上下控件：设计重复造成实现不一致。** 应用沉浸浏览器使用独立 2.5s Timer，播放器使用 3s Task，且前者不处理 hover ended。要求：单一可观察显隐状态负责顶部信息、左右导航和底部控制栏；进入/移动时同时显示、播放空闲 3s 同时隐藏、移出容器立即同时隐藏。暂停时悬停可见，移出仍隐藏；操作菜单/字幕文件选择期间避免控件被计时器撤走。隐藏后禁止命中测试；不允许不可见整屏布局拦截视频点击。

### 必需验收

- H.264/AAC MP4、H.264 MKV 与 HEVC 10-bit MKV 分别执行预览→全窗口→返回，覆盖播放中/暂停中、连续切换和非零位置；检查画面、声音、时钟、错误状态及会话数量。
- 旧会话在新会话启动前后分别离场，均不得取消新 owner；快速换文件不串音、不泄漏旧回调。
- SRT/WebVTT 多行、中文、BOM、重叠 cue、seek/暂停、损坏/超限输入与选取取消；加载失败保留原字幕。
- 上下栏同帧开始显隐，覆盖移出窗口、移到顶部按钮、暂停、3s 空闲、菜单与文件选择；实际窗口验收与状态机自动测试分别记录。

### 2026-10-16 再次实机验收的纠正（取代快照交接方案）

实际往返已复现“退出后从头播放且外挂字幕丢失”。因此快照字段单测通过不能代表呈现切换通过，R016 重新打开。设计改为：同文件只有一个会话对象，拥有 controller、主时钟、播放状态、字幕目录/选择及加载任务；预览/全窗口只领取 viewport lease，不重建会话。旧 lease 释放不得停止新 lease。真正换文件或最后 viewport 退出后有界回收资源；窗口动画和视图重建不能触发业务重开文件。

字幕目录统一包含 Off、内封轨、同目录匹配外挂和手动外挂。内封轨须由 Rust 解复用提供稳定 track ID、语言、标题、格式与 cue；选择不可冒用音视频轨号。不支持的图形/样式字幕须明确标记。自动外挂仅在已有目录读取权限下匹配视频同名或带分隔符的语言后缀，不误挂另一视频、不覆盖用户手动选择；缺目录授权时降级到手动选择，不扩大沙盒权限。手动选择器默认视频父目录。关闭字幕属于显式用户选择，不因自动扫描再打开。

IINA 参考依据：本地 v1.3.5 的 QuickSettingViewController.loadExternalSubAction / MainMenuActions.menuLoadExternalSub 将 currentURL 的父目录传入文件选择器；AutoFileMatcher 使用带 ticket 取消的同目录/搜索路径匹配；PlayerCore.loadExternalSubFile 检查重复外挂并重载已有轨；内外字幕共用 track 选择。这里只借鉴行为与职责，不复制其 MPV 调用或代码。

### 媒体界面统一 i18n（2026-10-16）

当前界面混用主 Bundle 本地化、硬编码英文与宿主自有语言状态，属于接入设计缺口及漏接实现。播放器、字幕菜单、加载/失败提示、沉浸浏览顶部和导航统一接入现有 TTI18n；TTZip 在启动及语言切换时同步其语言。简繁中文完整覆盖本轮文案，其他语言按显式英文后备处理；文件名、轨道原始元数据和底层诊断不翻译。验收包括中文实际输出、英文回切和语言变化不重建播放会话。

## 2026-10-17 实机反馈：字幕挂载报错、同目录发现缺口、切换卡顿与样式系统（设计先行修订）

### 设计问题 vs 实现问题归因（对照 `references/iina` 与 `references/ffmpeg`）

1. **设计问题 A（导致实机截图报错 `无法加载字幕: Subtitle cue end must follow start` 的直接根因）：将字幕流解复用设计为“全或无（All-or-Nothing）严格校验”，而非容错流式解复用**
   - **实机证据**：真实剪辑样本 `Sample #22`（`.srt`，SHA-256 前缀 `9c5105adca85efce`，29,861 字节）第 337 条字幕为 `00:14:52,456 --> 00:14:52,456`，紧接第 338 条 `00:14:53,456 --> 00:14:54,077`。对本机 `~/Documents/video_dev` 全部 27 个真实外挂字幕文件（26 份唯一内容：22 个 `.srt`、5 个 `.ttml`，其中 `Sample #08` 与 `Sample #09` 为同一内容重复文件，SHA-256 前缀均为 `297a97e481ecf10c`）执行生产 C-ABI 解析器 `ttmedia_subtitles_parse` 实测：**27 个文件（26 份唯一内容）全部 `OK (0 errors)`**（27 个文件合计提取 20,037 条有效 cue，26 份唯一内容合计 19,514 条有效 cue），其中包括 5 个含 `start == end` 零时长块的 `.srt` 样本与 5 个 `.ttml` 样本。
   - **FFmpeg / IINA 参照与修正设计**：FFmpeg `libavformat/srtdec.c` 与 `subtitles.c`（`ff_subtitles_queue_finalize`）采用逐行容错状态机——跳过空文本块，对 `end == start` 的零时长字幕块使用下一条非重叠字幕的起点（上限 `start + 1.5s`，末尾默认 `+0.8s`）自动补齐展示时长，对个别时间戳倒置（`end < start`）的单条损坏块跳过，仅当整份文件产出的有效 cue 数为 0 时才返回错误。
2. **设计问题 B：同目录字幕自动匹配与菜单展示过度严苛，不符合剪辑工程目录习惯**
   - **现状缺陷**：`VideoPlaybackSession.companions(for:)` 仅当字幕文件名等于视频主干名或以 `.`/`_`/`-` 紧接视频主干名时才纳入菜单。而在实际视频制作目录中，视频常命名为 `视频号_4k_H264.mp4`、`成片.mp4`，同目录字幕则为 `Timeline 1.srt`、`Subtitle 1.srt`、`简体字幕.srt`、`Timeline 1_Subtitle 1.ttml`，导致菜单完全不展示同目录现成字幕，迫使用户每次手动打开系统文件选择器。
   - **IINA 参照与修正设计**：对标 IINA `AutoFileMatcher.swift` 与 `QuickSettingViewController.swift`（同时读取 `relatedSubs` 与同目录全部字幕 `currentSubsInfo`）：
     - 匹配伴随轨（`companionSubtitles`）：支持同名、`.`/`_`/`-` 分隔后缀匹配，自动预热并优先挂载；
     - 同目录可用轨（`directorySubtitles`）：将同文件夹下其余受支持字幕文件（如 `Timeline 1.srt`、`Subtitle 1.srt`、`简体字幕.srt`、`Timeline 1_Subtitle 1.ttml`）直接列入字幕菜单与快设面板，实现一键挂载，并对排名前列的通用时间轴字幕预先后台缓存。
3. **设计问题 C：把内封字幕切换设计为“每次切轨触发全文件扫描 + 阻塞式 Loading 遮罩”，且必须严格区分“用户主动完整加载”与“后台预热”预算及原生扫描取消边界**
   - **现状缺陷**：此前契约将 `track = -1`（仅读目录）与 `track = N`（按需扫描整部 MKV 提取单轨 cue）分离，且会话层未设计轨道缓存（`subtitleCache`），UI 层设计了 `isSubtitleLoading` 居中 `ProgressView("正在加载字幕…")` 遮罩。用户每次切换内封或外挂字幕都会重新发起跨进程/磁盘扫描，弹出加载遮罩。
   - **IINA 参照与修正设计**：IINA（`QuickSettingViewController.swift`、`PlayerCore.swift`、`OSDMessage.swift`）通过 `setTrack` 向 `mpv` 设置 `sid`/`secondary-sid` 并以非阻塞 `OSDMessage.subTrack` 提示当前轨道，不在画面中央弹出加载遮罩。由于 `TTMediaKit` 不依赖 `libmpv`，本模块设计会话级内存缓存 `[String: ExternalSubtitleTrack]`，并严格区分**用户主动完整加载**与**后台有界预热**：
     - **用户主动完整加载与初始默认轨激活（`track > 0`）**：仅在用户手动选择某条内封主/副字幕轨（`selectSubtitle` / `selectSecondarySubtitle`）或初始发现阶段无同名外挂而激活唯一首选默认内封轨（`activateDefaultEmbeddedTrack`）时调用 `track > 0`。该模式不设 4 秒预热时间截断（`deadline: None`），完整扫描整段 `Segment` 提取全片时间轴，同时在扫描启动前、每个 EBML 元素与 `Cluster` 块边界检查按请求身份隔离的取消状态（`request_id` / `is_request_cancelled(request_id)`）。
     - **后台有界预热（`track = 0` 或编码请求 `track <= -2`）与编码边界校验**：由 `scheduleBackgroundSubtitlePrewarm` 启动，外挂队列与内封容器共享总计 `4.0s` 协作式软时间预算与最多 `8` 轨上限。外挂队列执行完毕后，`prewarmEmbeddedBatch` 计算**剩余时间预算** `remainingBudgetMs = (deadline - CFAbsoluteTimeGetCurrent()) * 1000`（绝不重置获取新的 4 秒预算；若剩余预算 `<= 0` 则直接跳过内封预热）与剩余槽位数 `remainingSlots = maxTracks - prewarmedCount`。**即使仅剩 1 个预热名额或仅有 1 条未缓存内封轨（`boundedTargets.count == 1`），后台预热也严禁调用 `track > 0` 绕过预热预算**，而是统一通过 `SubtitlePrewarmRequestCodec` / `try_encode_prewarm_request`（`track <= -2`）携带 `targetTrackID`、`maxTracks` 与**剩余预算 `remainingBudgetMs`** 下推至 Rust `container_subtitles.rs`。编码/解码层严格校验边界：`Int64.min`（`i64::MIN`）在 Rust `decode_container_request` 中通过 `track.checked_neg().ok_or_else(...)` 直接返回 `InvalidInput` 非法输入错误（Swift `SubtitlePrewarmRequestCodec.decode` 返回 `nil`），既不触发取负溢出，也**绝不回退为批量预热**；保留高位非零（`(raw >> 56) != 0`）返回 `InvalidInput` 错误；无法表示的轨道 ID（`<= 0` 或 `> UInt32.max`，如 `(1 << 32) + 3`）直接返回 `nil` / `None`，绝不静默截断为另一条轨道。Rust 仅当整段 `Segment` 完整扫描至末尾时返回 `"complete": true` 与 `trackCues`，若中途超时或被取消则立即返回 `"complete": false` 与空 `trackCues: {}`；Swift `decodeBatchTracksOffMainActor` 强制要求 `response.complete == true` 才允许写入 `subtitleCache`。
     - **请求级取消与回收生命周期（`register -> cancel -> complete -> reclaim`）、容量拒绝、分类统计与三级 XPC 证据分类**：将取消从进程全局代次改为每请求独立身份（`request_id: UInt64`，分别绑定 `activePrewarmRequestID`、`activePrimaryRequestID`、`activeSecondaryRequestID`、`activeDefaultEmbeddedRequestID`）。取消预热（`cancelSubtitlePrewarm()`）、取消/切换主字幕或取消/切换副字幕仅调用 `ttmedia_subtitles_cancel_request(request_id)` 取消对应请求，**主字幕加载中切换/取消副字幕绝不取消主字幕，反之亦然**。为杜绝“未结束请求或已注册尚未启动请求的取消记录被后续取消覆盖”以及长期运行内存无界增长，Rust `SubtitleRequestTable` 与 Swift `SubtitleSessionRequestRegistry` 均实现显式四阶段生命周期：（1）**已接收/已注册或正在扫描的请求常驻活跃表**（Rust `active_entries: HashMap<u64, Arc<ActiveRequestEntry>>`，Swift `liveRequests: [UInt64: LiveRequest]`，容量上限 `MAX_ACTIVE_REQUESTS = 4096`）：取消已注册但尚未开始扫描（`in_flight_scans == 0` / `runningDepth == 0`）或正在扫描的请求时，仅在原地设置 `cancelled = true`，**绝不将其从 `active_entries` / `liveRequests` 移出或降级到容量有界的墓碑表**；因此即使在“注册 `R` → 取消 `R` → 超过墓碑容量（`> 4096`）的其他取消 → 迟到启动 `R`”序列下，`R` 的取消状态仍保留在活跃表中并在迟到启动时立即被拒绝（`clusters_scanned == 0`），直至 `R` 完成或显式回收（`ActiveRequestGuard::drop` / `ttmedia_subtitles_reclaim_request` / `finishRequest`）后才从活跃表移除；（2）**容量满时显式拒绝新请求**：当 `active_entries.len() >= 4096`（或 `liveRequests.count >= 4096`）时，新请求注册返回 `false`、启动返回错误（`ErrorKind::WouldBlock` / `MediaWorkerError`），**严禁淘汰或静默遗忘已有请求**；（3）**分类容量统计与回收验证**：分别导出并验证 `active_request_count`（上限 `4096`，全部完成后归 `0`）、未注册乱序取消墓碑 `pre_start_tombstone_count`（上限 `4096`，消费/回收后归 `0`）、完成诊断记录 `completed_record_count`（上限 `256`）与待消费延迟记录 `pending_delay_count`（上限 `256`，消费/回收后归 `0`）。关闭旧会话（`stop()` / `closeMedia`）仅取消并回收该会话名下全部请求而不影响新会话。XPC 验证证据严格区分为三类：（1）进程内 `DirectTransport`；（2）进程内 `NSXPCListener.anonymous()` + `MediaWorkerService`（`workerPid == getpid()`，仅证明 XPC 接口序列化往返，不冒充独立进程证明）；（3）**独立 PID 的 Worker 进程（`workerPid != getpid()`）**，通过打包 `TTMediaServiceSmokeHost.app` + `launchd` `TTMediaWorker.xpc` 及跨 PID Worker 子进程验证：主动单轨扫描 `reqA`（`track = 3`）在第 1 个 Cluster 后停止（`clustersScanned == 1`、`isCancelled == true`），同一 Worker 内并发单轨扫描 `reqB`（`track = 4`）继续完成 2 个 Cluster（`clustersScanned == 2`），且 `(workerPid, clustersScanned, isCancelled)` 扫描状态通过 IPC 返回。
     - **全入口取消与缓存命中切轨竞态防护**：所有主/副字幕选择入口（`loadExternalSubtitle`、`selectSubtitle`、`selectSecondarySubtitle`）在判断 `"off"` 或 `subtitleCache` 命中前，统一递增 generation 并仅取消对应的上一主/副请求与预热任务，杜绝“未缓存 A 正在加载时手动选择已缓存 B，A 完成后覆盖 B”的竞态。切换已缓存轨道仅在主 actor 上更新引用与当前时间点文本，移除 `isSubtitleLoading` 居中遮罩，改为右上角 OSD 状态胶囊。
4. **设计问题 D：缺少字幕样式、位置、延迟、主/副双字幕轨模型与固定位置约束**
   - **现状缺陷**：此前契约声明“不提供布局设置与样式”，视图层硬编码字号与边距，不支持调节字号、缩放、颜色、描边、背景底框、垂直位置、时间轴延迟及双语主/副字幕同屏显示；此外初版样式叠字将 `bottomPadding` 与 `controlsVisible` 绑定，导致鼠标移入/移出引发字幕上下跳动，且小于 `380pt` 的视口仍残留重复的中央播放按钮 `compactCenterPlayButton`。
   - **IINA 参照与修正设计**：对标 IINA `Preference.swift` 与 `QuickSettingViewController.swift`（`sub` 面板），新增可持久化的 `SubtitleStyleConfiguration` 及副字幕轨选择（`secondarySubtitleID` / `secondarySubtitleTrack`），在控制栏提供交互式字幕设置面板（Popover），并在紧凑预览视口下按视口宽度自适应缩放字号。**严格解除字幕垂直位置（`SubtitleOverlayView.bottomPadding`）与控制栏显隐（`controlsVisible`）的关联**，确保暂停或播放时反复移入/移出鼠标字幕位置绝对不变；同时移除小于 `380pt` 视口的中央播放按钮 `compactCenterPlayButton`，仅保留底栏操作。
5. **设计问题 E：格式与字符编码覆盖缺口（TTML/DFXP、ASS/SSA 与 UTF-16/传统编码）及严禁启发式篡改时间码**
   - **现状缺陷**：实际剪辑目录中存在 `.ttml` 字幕、MKV 内 `S_TEXT/ASS` / `S_TEXT/SSA`，以及外挂 UTF-16LE/BE BOM 或 GB18030/Big5 编码。
   - **修正设计**：支持 `.srt`、`.vtt`、`.ttml`、`.dfxp`、`.ass`、`.ssa` 外挂与 `S_TEXT/UTF8`、`S_TEXT/ASCII`、`S_TEXT/WEBVTT`、`S_TEXT/ASS`、`S_TEXT/SSA` 内封轨；Rust 直接解码 UTF-8 / UTF-16LE / UTF-16BE BOM，Swift 补充 GB18030 / Big5 / UTF-16 / WindowsCP1252 回退转码；统一剥离 ASS `{...}` 覆写标签与基础 HTML 标签。**禁止对 TTML（如首条在 `3600~3660s`）自动猜测并减去一小时时间码**，默认严格保留原始时间戳，避免破坏影片一小时后才出现首条字幕的合法文件；工程时间码偏移统一通过用户显式调节 `delay` 或可靠元数据处理。
6. **实现问题 A：Matroska 容器扫描绕过缓存触发海量 `pread` 系统调用，且 `mmap` 违反跨进程文件修改安全前提**
   - **现状缺陷**：`container_subtitles.rs` 原先使用 4KB `HeaderCache`，且 `uint()` 与 `bytes()` 直接调用 `file.read_exact_at` 绕过缓存，导致每次读取时间戳、时长与跳过块都触发内核 `pread` 系统调用（耗时长达 4.58s）；而若改用 `unsafe { memmap2::Mmap::map }`，“本进程只读打开”并不能阻止外部进程在扫描期间修改或截断影片文件，一旦底层文件被截断将引发 `SIGBUS` 或 Rust 未定义行为（UB）。
   - **修正实现**：移除 `memmap2` 与 `unsafe` 映射，改用 100% 安全的 64 KiB `FileExt::read_at` 窗口缓存（`HeaderCache`），令 `vint()`、`uint()`（栈上零分配）及块内头部/短文本全部经由缓存窗口读取，大跨度视频帧直接移动偏移跳过而不读入未命中的中间负载；当底层文件被外部修改或截断时，`read_at` 安全返回短读并转为可恢复的 `UnexpectedEof` 错误。
7. **实现问题 B：SRT/VTT `split("\n\n")` 分块脆弱、拖拽挂载未接线、时间轴检索与主线程去重**
   - **现状缺陷**：`subtitles.rs` 使用 `split("\n\n")` 在字幕正文含空行时断裂；`AVSampleBufferPlayerNSView.onDropFiles` 未在 `AVSampleBufferHostRepresentable` 接线；`ExternalSubtitleTrack.text(at:)` 使用每帧全量线性遍历。
   - **修正实现**：`subtitles.rs` 改为逐行流式状态机；打通播放器视口拖拽加载外挂字幕（`onDropFiles`）；`ExternalSubtitleTrack` 预计算 `maxEndPrefix` 与跳跃前驱索引 `prevHigherEndIndex`，先以 $O(\log N)$ 二分定位 `start <= target` 上界，再仅沿活动上界跳表收集重叠块（无重叠或常规重叠下为 $O(\log N + k)$，极端全片重叠块最坏退化为 $O(N)$）；`VideoPlaybackSession` 维护去重后的 `activeSubtitleText` 与 `activeSecondarySubtitleText`。

## 2026-10-17 播放生命周期、呈现权优先级与异常恢复收尾规约（先于代码收尾）

### 1. 已证实的代码缺陷 vs 待实机确认的故障原因（证据边界）

- **已证实的代码与产物缺陷**：
  1. **旧安装目录产物与当前源码不一致**：`/Applications/TTZip.app`（UUID `652DB509-D4CB-3EB3-A13E-E8E39EA8D3C9`）构建于 `2026-10-16 09:05`，缺少 `TTMediaWorker.xpc` 及多视口租约实现，其运行日志不能直接推导新源码行为。
  2. **重复注册的呈现权抢占缺口**：`AVSampleBufferVideoPlayerView` 在 `.onAppear` 与 `.task(id:)` 均调用 `claim`。若全窗口在 `Inspector.onAppear` 与延迟执行的 `Inspector.task` 之间完成注册，无幂等与优先级约束的 `acquireViewport` 会让 `Inspector.task` 重新夺回 `activePresenterViewportID` 并搬走 `displayLayer`。
  3. **异常恢复缺少字幕实际内容与手动外挂来源迁移**：重建会话（`restart: true`）若仅复制 `selectedSubtitleID` / `selectedSecondarySubtitleID` 而不迁移 `subtitleCache`、手动挂载的外挂文件 URL（含非同目录文件）及已解析的 `subtitleTrack` / `secondarySubtitleTrack`，后续发现流程因 `selectedSubtitleID != nil` 跳过默认激活，会导致“菜单显示选中但画面无字幕”。
  4. **双视口并存时异常恢复未重绑存活视图**：当 Inspector 与全窗口同时挂载时，一侧触发 `restart: true` 创建新 `VideoPlaybackSession`，若另一侧视图仍持有旧 `@State playback` 引用，则退出全窗口回退至 Inspector 时将因引用已停止的旧会话而无画面，且其 `onDisappear` 传入旧 `sessionID` 导致新会话租约无法释放。
  5. **显式 seek 到 0 后 `lastKnownTime` 未归零**：`currentTime` 此前仅在 `> 0` 时更新 `lastKnownTime`，导致用户显式 seek 到 `0.0s` 后触发恢复仍跳回旧正数进度。
  6. **暂停首帧时钟未锚定与 Worker 预读耗尽续播归零**：暂停加载时未锚定首帧 PTS，以及 Worker 暂停在非零位置但后台预读已 `inputExhausted` 时点击播放误触发 `seek(to: 0)`。
  7. **恢复设置被新 controller 默认值覆盖与过期视口绑定残留**：`restorePreservedContext(from:)` 复制了 `volume`、`isMuted`、`previousVolume` 与 `playbackRate`，但 `finalizeLoadedController` 又用新 controller 的默认 `volume`（`1.0`）和 `playbackRate`（`1.0`）反向覆盖会话状态，未向新 controller 真正下发保存设置；同时 `VideoPlaybackSessions.release` 在旧 `sessionID` 分支直接返回而未清理对应旧会话的 `viewportBindings`，切换视频（A→B）后旧视图消失可能遗留闭包。
- **待实机确认的故障原因**：
  - 上述缺陷已在单元与集成回归测试中复现并修复，但**不能将此前用户实机遇到的所有黑屏全部归结为单一的 350ms 生命周期竞态**；真实应用窗口下的全窗口切换、XPC 跨进程传输、显卡图层合成与键鼠交互仍须通过独立临时包（`/tmp/ttzip-verify-230/TTZip.app`）完成前台 GUI 实机验收后方可关闭。

### 2. 核心生命周期与呈现权不变量

1. **重复注册的幂等性（Idempotent Viewport Registration）**：
   - 同一 `viewportID` 在同一 `sessionID` 上重复调用 `claim` / `acquireViewport`（例如 `.onAppear` 后紧跟 `.task(id:)`）必须幂等：已处于 `activeViewports` 中的视口重复注册时，不得改变 `viewportOrder` 退场回退顺序，也不得从另一个已持有呈现权的活跃视口手中夺走 `activePresenterViewportID`。
2. **全窗口呈现优先级（Full-Window Presentation Priority）**：
   - 只要 `activeViewports` 中存在任何 `isFullScreen == true` 的活跃视口，任何非全屏视口（`isFullScreen == false`，如 Inspector 侧栏）无论何时执行 `.onAppear`、`.task(id:)` 或 `updateNSView` / `layout()`，均**严禁**抢占 `activePresenterViewportID` 或挂载 `displayLayer`。
   - 仅当所有全屏视口调用 `releaseViewport` 释放租约后，呈现权才按 `viewportOrder` 自动回退至仍存活的 Inspector 视口（`PRESENTER_FALLBACK`）。
3. **会话重建时所有存活视图的自动重新绑定（Live Viewport Rebinding on Restart）**：
   - `VideoPlaybackSessions` 作为可观察（`@Observable`）会话协调器维护当前活跃会话 `current` 及按 `viewportID` 登记的会话替换回调（`bindViewport`）。
   - 当任一视口触发重建（`restart: true`）产生新会话 `nextSession` 时，`VideoPlaybackSessions` 在保留所有存活视口 ID 及其 `isFullScreen` 标志的同时，立即通知并驱动所有仍挂载的 `AVSampleBufferVideoPlayerView`（通过同步绑定回调与 `.onChange(of: VideoPlaybackSessions.shared.current?.sessionID)`）将其 `@State playback` 重绑至 `nextSession`。
   - 重绑后：全窗口退出时 Inspector 已持有 `nextSession` 并无缝接管 `displayLayer`；最后关闭 Inspector 时使用 `nextSession.sessionID` 正常释放租约并回收会话。
4. **异常恢复完整还原主/副字幕内容与手动外挂来源**：
   - `restart: true` 必须将旧会话的 `subtitleCache`、`companionSubtitles`、`directorySubtitles`、`preservedExternalSubtitles`（含用户从任意目录手动挂载的外挂字幕 URL）、`embeddedSubtitles`、`subtitleTrack`、`secondarySubtitleTrack`、`subtitleName`、`secondarySubtitleName`、`selectedSubtitleID`、`selectedSecondarySubtitleID`、`subtitleStyle` 及自定义 `externalSubtitleLoader` / `embeddedSubtitleLoader` 完整迁移至新会话；
   - 新会话 `discoverSubtitles()` 扫描同目录文件时保留 `preservedExternalSubtitles` 中的手动外挂项，并在发现完成后对已选中的主/副字幕 ID 确保激活对应轨道实例与当前时间点的 `activeSubtitleText` / `activeSecondarySubtitleText`。
5. **显式 seek 到 0 同步更新 `lastKnownTime`**：
   - 区分“会话销毁/初始化重置（`isTearingDown` / `isSettingUpPlayback`）”与“正常播放或显式 seek 更新时间”：非销毁/初始化期间任何有效非负时间（`>= 0.0s`，含显式 `seek(to: .zero)`）均同步更新 `lastKnownTime`，确保显式回到 `0.0s` 后触发恢复不会跳回旧正数时间。
6. **恢复的音量、静音与倍速真正应用到底层 Controller**：
   - 新建或恢复会话在 `setupPlayback` 创建 `VideoPlaybackController` 时及 `finalizeLoadedController` 完成加载后，统一调用 `applyPlaybackSettings(to:)` 将会话当前的有效音量（`isMuted ? 0.0 : volume`）、静音标志（`isMuted`）与倍速（`playbackRate`）写入底层 `AVAssetReaderPlaybackController` / `AVSampleBufferWorkerPlaybackController`（禁止在 `finalizeLoadedController` 中用新 controller 的默认 `1.0` 反向覆盖会话属性），确保恢复后底层 `audioRenderer` / `coordinator` / `synchronizer` 与 UI 状态严格一致。
7. **视口绑定的会话身份与代际校验（清理过期绑定且防迟到 release 误删新绑定）**：
   - `VideoPlaybackSessions` 为每个 `viewportID` 的绑定记录所属 `sessionID` 与单调递增 `generation`（`ViewportBinding`）；`restart: true` 重绑存活视口时同步推进绑定的 `sessionID` 与 `generation`。
   - 当 `release(sessionID:viewportID:)` 命中旧会话（`current?.sessionID != sessionID`）或当前会话正常释放时，通过 `unbindViewportIfMatching(viewportID:sessionID:)` 校验：若该 `viewportID` 当前绑定的 `sessionID` 与被释放的 `sessionID` 一致，则立即移除绑定（消除 A→B 切换视频后旧视图消失遗留闭包）；若同一 `viewportID` 已绑定至新会话（`existing.sessionID != sessionID` 或更高 `generation`），迟到的旧 `release` 严禁删除该视口的新绑定。


