# Tasks: Media Playback Keyboard Controls & Miller Column Auto-Scroll Navigation

**Feature Identifier**: `001-media-keyboard-controls`  
**Classification**: `[Lean SDD]`  
**Status**: `Completed`

---

## Phase 1: Foundational Service

- [x] T001 Create `MediaPlaybackCoordinator` in `apple/Sources/TTZipApp/Services/MediaPlaybackCoordinator.swift` with thread-safe session registration, active state tracking, and key handling for Space (49), Left Arrow (123), and Right Arrow (124).

## Phase 2: Player Integration

- [x] T002 [P] [US1] Integrate `MediaPlaybackCoordinator` with `UnifiedVideoPlayerView` & `SharedVideoPlayerStore` in `apple/Sources/TTZipApp/Views/Preview/VideoAudioPlayerPreviewView.swift` to support Space toggle and Left/Right seek (5s/15s).
- [x] T003 [P] [US1] Integrate `MediaPlaybackCoordinator` with `UnifiedAudioPlayerView` in `apple/Sources/TTZipApp/Views/Preview/UnifiedAudioPlayerView.swift` and `apple/Sources/TTZipApp/Views/Preview/UnifiedAudioPlayerView+Controls.swift`.
- [x] T004 [P] [US1] Update `FullScreenMediaWindowController` and full screen preview in `apple/Sources/TTZipApp/Views/MediaPreviewView.swift` to support direct media key events during immersive presentation.

## Phase 3: Parent Event Filtering & Non-Interference

- [x] T005 [US2] Update `FinderMillerColumnsView.swift` key down monitor to check `MediaPlaybackCoordinator.shared.shouldInterceptMediaKeys()` before consuming arrow keys.
- [x] T006 [US2] Update `ArchiveExplorerView.swift` key down monitor to check `MediaPlaybackCoordinator.shared.shouldInterceptMediaKeys()` before consuming arrow keys.
- [x] T007 [US2] Update `QuickLookPreviewCoordinator.swift` to ensure Space bar handling coordinates seamlessly with media playback.

## Phase 4: Miller Column Auto-Scroll Visibility

- [x] T008 [US3] Refactor `SingleMillerColumnView.swift` with `ScrollViewReader` + `ConfigureNSScrollView` to track `selectedPath` and `isColumnActive`, triggering smooth `scrollTo(selectedPath, anchor: nil)` whenever selection or hierarchy level changes.
- [x] T009 [US3] Verify `FinderMillerColumnsView+Navigation.swift` left/right/up/down navigation dispatches item selection and ensures target column scroll synchronization.

## Phase 5: Automated Testing & Verification

- [x] T010 [P] [US4] Add unit and integration tests in `apple/Tests/TTZipAppTests/MediaPlaybackKeyboardControlTests.swift` validating play/pause toggling, 5s seek backward/forward, text field bypassing, and lifecycle cleanup.
- [x] T011 Run full test suite (`swift test`) and ensure zero regressions across all 112 tests.
