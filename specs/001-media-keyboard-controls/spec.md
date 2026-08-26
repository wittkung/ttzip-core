# Spec: Media Playback Keyboard Controls & Miller Column Auto-Scroll Navigation

**Feature Identifier**: `001-media-keyboard-controls`  
**Classification**: `[Lean SDD]` (Internal UI keyboard event routing, media player control mapping & viewport auto-scrolling)  
**Status**: `Approved / In Implementation`

---

## 1. Background & Problem Statement

1. **Media Playback Keyboard Shortcuts**:
   When previewing or playing video/audio media files (via `UnifiedVideoPlayerView`, `UnifiedAudioPlayerView`, or full-screen preview), keyboard events for Space (toggle play/pause) and Left/Right Arrow keys (seek backward/forward) do not respond or are unconditionally consumed by parent file navigation components (`FinderMillerColumnsView`, `ArchiveExplorerView`, `QuickLookPreviewCoordinator`).
   
   Users expect standard macOS media controls:
   - **Space Bar**: Toggle Play / Pause.
   - **Left Arrow**: Seek backward (by 5 seconds, or 15 seconds with Shift).
   - **Right Arrow**: Seek forward (by 5 seconds, or 15 seconds with Shift).

2. **Miller Column Hierarchy Switching & Scroll-Into-View**:
   When navigating directories using arrow keys (e.g. pressing Left Arrow to navigate back to the parent directory level or Up/Down within long columns), the selected item in the parent or target column may be off-screen. The column scroll view does not automatically scroll to make the focused item visible, causing user disorientation. The column must automatically scroll the focused/selected item into the visible viewport.

## 2. User Scenarios

### Scenario 1: Video Playback in Preview & Full Screen
- **Given** a user is previewing a video file in the inspector or in full-screen mode,
- **When** the user presses the **Space** key,
- **Then** the video toggles between Playing and Paused states smoothly.
- **When** the user presses the **Left Arrow** key,
- **Then** the video seeks backward by 5 seconds (bounded at 0.0s).
- **When** the user presses the **Right Arrow** key,
- **Then** the video seeks forward by 5 seconds (bounded at video duration).

### Scenario 2: Audio Playback in Preview
- **Given** a user is previewing an audio file in the inspector,
- **When** the user presses the **Space** key,
- **Then** audio playback starts or pauses.
- **When** the user presses the **Left / Right Arrow** keys,
- **Then** audio seeks backward / forward by 5 seconds.

### Scenario 3: Miller Column Parent/Hierarchy Level Switching Scroll
- **Given** a user is in a child directory and presses **Left Arrow** to return to the parent directory level,
- **When** the focused parent directory item is located far down a long list (off-screen),
- **Then** the parent column immediately and smoothly scrolls to bring the selected directory item into the visible viewport.

### Scenario 4: Up/Down Keyboard Navigation Auto-Scroll
- **Given** a user navigates items using **Up / Down** arrow keys in any Miller column,
- **When** the selection reaches the upper or lower boundary of the visible area,
- **Then** the column scroll view continuously follows the active selection into view.

### Scenario 5: Text Input Non-Interference
- **Given** a user is typing in a search bar, rename field, or any `NSTextView` / `NSTextField`,
- **When** the user presses Space or Left/Right/Up/Down arrows,
- **Then** text cursor and characters function normally without triggering media or navigation actions.

## 3. Functional Requirements

1. **Centralized Media Playback Coordinator (`MediaPlaybackCoordinator`)**:
   - Manages active media player sessions (video, audio, full-screen).
   - Provides thread-safe `@MainActor` state for `isPlaying`, `isMediaActive`, and callback handlers for `togglePlayPause()` and `seekBy(seconds: Double)`.
   - Listens to key down events and accurately dispatches Space (KeyCode 49), Left Arrow (KeyCode 123), and Right Arrow (KeyCode 124).
2. **Unified Video Player Integration**:
   - `UnifiedVideoPlayerView` / `SharedVideoPlayerStore` registers with `MediaPlaybackCoordinator` upon appearance/playback and unregisters on teardown.
   - Provides `seekBy(seconds: Double)` method to adjust current playback time safely.
3. **Unified Audio Player Integration**:
   - `UnifiedAudioPlayerView` registers with `MediaPlaybackCoordinator` and forwards play/pause and seek requests.
4. **Parent View Event Filter Updates**:
   - `FinderMillerColumnsView` and `ArchiveExplorerView` local event monitors check `MediaPlaybackCoordinator.shared.shouldInterceptMediaKeys()` before consuming Left/Right arrow events.
   - `QuickLookPreviewCoordinator` coordinates space bar handling so active media players receive space bar toggle.
5. **Miller Column Auto-Scroll Visibility**:
   - `SingleMillerColumnView` incorporates `ScrollViewReader` with `ConfigureNSScrollView` overlay scroller.
   - Listens to `selectedPath` and `isColumnActive` changes and invokes `proxy.scrollTo(targetPath, anchor: nil)` to ensure selected items are always in the visible viewport.

## 4. Success Criteria

- [x] Pressing Space toggles play/pause on active video and audio preview.
- [x] Pressing Left Arrow seeks backward by 5 seconds (15s with Shift).
- [x] Pressing Right Arrow seeks forward by 5 seconds (15s with Shift).
- [x] Navigating to parent level via Left Arrow auto-scrolls the parent column so the selected item is fully visible.
- [x] Up/Down arrow navigation auto-scrolls the active column to keep the focused item in view.
- [x] Text fields and editors retain normal cursor and space bar input.
- [x] Automated unit and integration tests pass with 100% success.
