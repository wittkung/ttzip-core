# Quickstart Guide: macOS 原生 Quick Look 与 Finder 拖拽集成 (Feature 168)

## Scenario 1: 在 SwiftUI 归档浏览器中空格键触发 Quick Look
- **Action**:
  在 `HomeExplorerContainerView` 或 `ArchiveExplorerView` 中选中压缩包内的任意文件，按下键盘空格键（Space bar）。
- **Expected Outcome**:
  - `QuickLookPreviewCoordinator` 拦截 `keyCode == 49`；
  - 调用 `ArchiveSelectiveExtractor.extractSingleEntryData` 毫秒级提取数据；
  - `EphemeralPreviewCacheManager` 在 `0o700` 沙盒中写入临时文件；
  - 弹出 macOS 原生 Quick Look 预览浮窗；
  - 再次按下空格键或 Esc 键即刻关闭预览并释放临时文件。

---

## Scenario 2: 将压缩包内条目拖拽至访达 (Finder)
- **Action**:
  拖拽文件行至 Finder 窗口或桌面。
- **Expected Outcome**:
  - `ArchiveFilePromiseProvider` 接收到 Finder drop 信号；
  - 延迟执行流式提取并直接落盘至目标目录；
  - 目标目录出现提取完毕的文件，界面无任何卡顿。
