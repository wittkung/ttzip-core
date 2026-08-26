# Quickstart Guide: SwiftUI 桌面端与 C11 微内核打通及设计系统验证 (Feature 164)

## Scenario 1: 验证 60fps C-to-Swift 无锁流式进度与即时取消
- **Command**:
  ```bash
  swift test --filter ProgressStreamingBridgeTests
  ```
- **Expected Output**:
  - `[PASS] test_c_progress_bridge_emits_throttled_events_at_60fps`
  - `[PASS] test_c_progress_cancellation_aborts_in_under_5ms`
  - 验证在 10,000 个文件的压缩任务中，进度更新保持 16.6ms 间隔，UI 主线程 0 阻塞。
- **Failure Diagnostic**:
  - 若事件延迟超过 50ms，检查 C 桥接中的 `mach_absolute_time()` 节流阈值是否正确。

---

## Scenario 2: 验证三栏 Y=90pt Kintsugi Gold 金线绝对对齐
- **Command**:
  ```bash
  swift test --filter DesignSystemLayoutAlignmentTests
  ```
- **Expected Output**:
  - `[PASS] test_sidebar_workspace_inspector_golden_rule_aligned_at_y90`
  - `[PASS] test_52pt_header_bar_typography_tokens`
  - 三栏顶栏金线高度绝对统一为 `Y = 90.0pt`，边框颜色与 Kintsugi Gold `#D4AF37` 100% 匹配。
- **Failure Diagnostic**:
  - 若对齐偏移，检查 `MainView.swift` 和 `HomeExplorerContainerView.swift` 的 top padding 是否为 38pt。

---

## Scenario 3: 验证单文件即时 Quick Look 预览与拖拽解压
- **Command**:
  ```bash
  swift test --filter SelectiveSingleItemExtractionTests
  ```
- **Expected Output**:
  - `[PASS] test_single_entry_stream_extraction_under_10ms`
  - `[PASS] test_quicklook_temporary_pipeline_cleanup`
  - 针对 100MB 归档中的单张图片提取，耗时 < 10ms 且不解压完整归档。
