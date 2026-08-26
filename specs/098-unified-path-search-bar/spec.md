# Feature Specification: Unified Path and Search Address Bar (一体化路径与搜索地址栏)

**Feature Branch**: `098-unified-path-search-bar`

**Created**: 2026-08-18

**Status**: Draft

**Input**: User description: "我们搜索栏要扩展成路径与搜索栏，可以直接输入路径"

## Clarifications

### Session 2026-08-18

- Q: How should the address bar transition between idle path display and active text input / search mode? → A: The address bar presents an interactive path pill (breadcrumb segments with folder icon) when idle/unfocused. Clicking anywhere on the pill or pressing `⌘L` / `⇧⌘G` immediately transforms it into a live text input with the full POSIX path pre-selected for fast typing or overwrite; pressing `Esc` or clicking away reverts to the formatted path pill.
- Q: How does the system handle dual-mode input (path vs search)? → A: Real-time prefix detection: entries starting with `/`, `~`, `.`, or `file://` activate Path Navigation mode with directory autocompletion; other text entries activate Spotlight Search mode with file/archive search results.
- Q: What happens when navigating to a directory outside sandbox permissions? → A: The address bar requests access via `RootFolderAccessManager.shared.ensureAccess` / security-scoped bookmark prompt, providing smooth access without crashing or resetting state.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Direct Path Input & Instant Navigation (Priority: P1)

As a macOS power user or developer browsing files in TTZip, I want to directly type, paste, or edit full filesystem paths (e.g., `/Users/username/Projects`, `~/Downloads`, `../backup`, `file:///Users/...`) in the top navigation address bar and press Enter, so that I can instantly jump to any directory without clicking through multiple Miller column levels or Finder dialogues.

**Why this priority**: Direct path entry is the foundational capability of an omnibar/address bar, transforming TTZip from a passive folder clicker into a high-efficiency pro-grade file manager.

**Independent Test**: Can be tested by focusing the address bar, typing `~/Downloads` (or `/tmp`), pressing Enter, and asserting that the explorer view immediately updates its root/current directory to the resolved absolute path.

**Acceptance Scenarios**:
1. **Given** the user is in TTZip Home / Explorer view, **When** they click the address bar or press `⌘L` / `⇧⌘G`, **Then** the bar transitions into text edit mode with the current directory path selected.
2. **Given** the address bar is in edit mode, **When** the user types `/Library/Caches` and presses `Enter`, **Then** TTZip validates the path, updates `viewModel.currentDirectory`, and displays the contents of `/Library/Caches`.
3. **Given** the user inputs a tilde path like `~/Documents`, **When** the user commits navigation, **Then** TTZip expands `~` to `NSHomeDirectory()` and navigates to the user's Documents folder.
4. **Given** the user inputs a path to a supported archive file (e.g. `~/Downloads/corpus.7z` or `archive.zip`), **When** pressed `Enter`, **Then** TTZip automatically opens the archive in `ArchiveExplorerView` or folder inspection mode.
5. **Given** the user inputs a non-existent or invalid directory path (e.g. `/invalid/path/xyz`), **When** pressed `Enter`, **Then** TTZip displays a non-intrusive error feedback (subtle shake animation and status tooltip "Directory not found") while preserving the input for quick correction.

---

### User Story 2 - Real-Time Path Autocomplete & Suggestion Dropdown (Priority: P2)

As a user typing a directory path, I want real-time path autocompletion suggestions as I type subfolder prefixes (e.g. typing `~/Doc` suggests `~/Documents/`, `~/Docker/`), so that I can quickly complete paths with `Tab` or navigate suggestions with arrow keys without typing full paths manually.

**Why this priority**: Autocomplete dramatically speeds up keyboard navigation and eliminates path spelling errors.

**Independent Test**: Can be tested by typing `~` or `/` in the address bar and verifying that matching directory candidates are dynamically enumerated and rendered in a liquid-glass popup, selectable via `↑`/`↓` and autocompleted via `Tab`.

**Acceptance Scenarios**:
1. **Given** the address bar contains a partial directory path (e.g. `/usr/l`), **When** typing continues, **Then** a dropdown appears displaying existing subdirectories (e.g. `/usr/lib`, `/usr/local`, `/usr/libexec`) with folder icons.
2. **Given** the path suggestions dropdown is open, **When** the user presses `Tab` or `↓`, **Then** the first matching candidate is completed into the text field.
3. **Given** the user navigates suggestions using `↑` and `↓` arrow keys and presses `Enter`, **Then** the selected candidate directory is chosen and navigated to.
4. **Given** the user presses `Esc`, **When** the dropdown is open, **Then** the suggestion dropdown dismisses and the address bar reverts to the current active directory.

---

### User Story 3 - Unified Search vs Path Dual-Mode Switching (Priority: P3)

As a user looking for files or archives, I want the address bar to automatically distinguish between a path entry (starts with `/`, `~`, `.`, or `file://`) and a keyword search query (e.g. "invoice", "silesia", "backup.tar.gz"), providing Spotlight search results when searching and path suggestions when navigating.

**Why this priority**: Unifies two disparate workflows into a single cohesive, minimalist Apple Silicon aesthetic bar without visual clutter or extra modal controls.

**Independent Test**: Can be tested by typing "project_report" (keyword) to verify Spotlight file search activates, and typing `/var/log` (path) to verify Path Navigation mode activates with distinct visual iconography.

**Acceptance Scenarios**:
1. **Given** the user enters query text without path delimiters (e.g. `report 2026`), **Then** the address bar displays the magnifying glass icon with Bamboo Green accent and queries `SpotlightSearchService`.
2. **Given** the user enters path prefixes (e.g. `/`, `~/`, `./`, `../`), **Then** the leading icon dynamically switches to a folder / path icon with Kintsugi Gold accent and displays path directory suggestions.
3. **Given** the user pastes a standard macOS file URL (e.g. `file:///Users/username/Desktop`), **Then** the omnibar automatically sanitizes and unescapes it into a POSIX path.

---

### Edge Cases

- **Root & Sandbox Access Restrictions**: When navigating to a directory outside the currently granted sandbox scope, the address bar triggers `RootFolderAccessManager` to prompt the user for permission bookmark or request root access without crashing.
- **Relative Path Resolution**: Relative paths like `../` or `./subfolder` must resolve relative to `viewModel.currentDirectory`.
- **Trailing Slashes and Redundant Slashes**: Paths like `///Users//john///Downloads///` must be normalized via `(path as NSString).standardizingPath` or URL standardization.
- **Spaces and Escaped Characters**: Paths with spaces (e.g. `/Users/john/My Documents/`) or backslash escapes (e.g. `/Users/john/My\ Documents/`) must be parsed properly whether pasted with or without quotes/escapes.
- **Symlinks & Aliases**: Navigating to a symlinked folder must resolve the target directory gracefully or navigate to the symlink URL.
- **Empty or Whitespace-Only Input**: Pressing Enter on empty string restores the current directory path.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide a unified Address and Search Bar (`LiquidGlassAddressBar` / `UnifiedAddressSearchBar`) in the top navigation area of TTZip.
- **FR-002**: Address bar MUST support direct typing and pasting of POSIX paths (`/path/to/folder`), tilde home paths (`~/folder`), relative paths (`./`, `../`), and `file://` URLs.
- **FR-003**: Address bar MUST expand tilde (`~`) to user home directory (`NSHomeDirectory()`) and normalize redundant slashes and dots.
- **FR-004**: When user commits a valid directory path via `Enter`, System MUST update `viewModel.currentDirectory` to the target directory and refresh directory views.
- **FR-005**: When user commits a path pointing to a recognized archive file (`.zip`, `.7z`, `.tar`, `.gz`, `.bz2`, `.xz`, `.zst`, `.rar`, `.cab`, etc.), System MUST open the archive directly for inspection.
- **FR-006**: When user inputs a partial path, System MUST provide asynchronous, non-blocking path autocompletion suggestions matching local subdirectories.
- **FR-007**: System MUST support keyboard navigation: `Tab` to autocomplete the top match, `↑`/`↓` to highlight dropdown items, `Enter` to navigate to the selected directory, and `Esc` to cancel.
- **FR-008**: System MUST provide global keyboard shortcuts `⌘L` and `⇧⌘G` to focus and select the address bar text for instant path navigation.
- **FR-009**: System MUST automatically discriminate between Path Navigation mode (triggered by `/`, `~`, `.`, `file://`, or valid path structure) and Keyword Search mode (triggered by non-path keywords).
- **FR-010**: In Keyword Search mode, System MUST preserve full backwards-compatible Spotlight search functionality via `SpotlightSearchService`.
- **FR-011**: When a path does not exist or cannot be accessed, System MUST provide non-blocking visual feedback (shake animation + hairline warning outline + tooltip) without throwing fatal exceptions.
- **FR-012**: When idle/unfocused, Address bar MUST show the current path with an elegant breadcrumb / path pill style or clean monospaced path format matching Zen / WSJ Editorial guidelines.

### Key Entities *(include if feature involves data)*

- **`AddressBarMode`**: Enum representing active input mode (`.pathNavigation`, `.spotlightSearch`).
- **`PathSuggestionItem`**: Represents an autocompleted directory candidate with attributes: `path: String`, `displayName: String`, `isDirectory: Bool`, `isArchive: Bool`, `parentPath: String`.
- **`PathResolutionResult`**: Result of parsing and validating an input path: `.directory(URL)`, `.archive(URL)`, `.file(URL)`, `.notFound(String)`, `.permissionRequired(URL)`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can navigate to any arbitrary filesystem directory in under 2 seconds by typing/pasting its path.
- **SC-002**: Path autocompletion latency for local directories with <= 1,000 subfolders is under 15 ms, maintaining 60fps UI responsiveness.
- **SC-003**: 100% of standard macOS path formats (`~`, `/`, `../`, `file://`, unquoted spaces, escaped spaces) resolve accurately to the intended destination.
- **SC-004**: 0% regression in existing Spotlight search capabilities or archive opening performance.
- **SC-005**: Keyboard accessibility allows 100% mouse-free path entry, autocomplete selection, and navigation.

## Assumptions

- Target operating system is macOS 14.0+ (Sonoma) running AppKit + SwiftUI.
- Filesystem I/O for path suggestions runs on background threads (`DispatchQueue.global(qos: .userInitiated)`) to ensure zero main-thread hitching.
- Sandboxing access for paths outside existing bookmarks is delegated to `RootFolderAccessManager.shared`.
- The UI follows TTZip design tokens (`TTZipTheme.bambooGreen`, `TTZipTheme.kintsugiGold`, `TTZipTheme.hairlineBorder`, Liquid Glass blur backgrounds).
