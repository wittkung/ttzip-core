# Data Model: Unified Path and Search Address Bar (一体化路径与搜索地址栏)

**Feature Branch**: `098-unified-path-search-bar`
**Created**: 2026-08-18

---

## 1. Core Enumerations & Entities

### 1.1 `AddressBarInputMode` (Input Mode Discriminator)
Identifies the active operational mode based on user input analysis.

| Value | Type | Description |
| :--- | :--- | :--- |
| `pathNavigation` | `String` enum | User is typing a filesystem path (triggered by `/`, `~`, `.`, or `file://`). |
| `spotlightSearch` | `String` enum | User is typing keywords for Spotlight file/archive search. |

---

### 1.2 `PathResolutionType` (Destination Classification)
Represents the evaluated destination type of a sanitized input path.

| Value | Type | Description |
| :--- | :--- | :--- |
| `directory` | `String` enum | Target is an existing local directory. |
| `archive` | `String` enum | Target is an existing supported archive file (`.zip`, `.7z`, `.tar`, etc.). |
| `file` | `String` enum | Target is a non-archive regular file. |
| `notFound` | `String` enum | Target path does not exist on the filesystem. |
| `permissionRequired` | `String` enum | Target requires user authorization/sandbox bookmark. |

---

### 1.3 `PathResolutionResult` (Resolution Outcome Entity)
Encapsulates the complete result of resolving and validating a raw path string.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `rawInput` | `String` | Yes | The original unparsed user input string. |
| `sanitizedPath` | `String` | Yes | Normalized absolute POSIX path. |
| `destinationType` | `PathResolutionType` | Yes | The evaluated category of the target destination. |
| `exists` | `Bool` | Yes | Whether the resolved path currently exists on disk. |
| `isDirectory` | `Bool` | Yes | Whether the resolved path is a directory. |
| `isArchive` | `Bool` | Yes | Whether the resolved path is a recognized archive file. |
| `errorMessage` | `String?` | No | Optional human-readable diagnostic error message. |

---

### 1.4 `PathSuggestionItem` (Autocomplete Dropdown Item)
Represents a single autocompleted filesystem entry in the suggestion list.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `id` | `String` | Yes | Unique identifier (canonical absolute path). |
| `path` | `String` | Yes | Full normalized path of the candidate item. |
| `displayName` | `String` | Yes | Name of the file/folder displayed in the list. |
| `parentPath` | `String` | Yes | Absolute path of the containing folder. |
| `isDirectory` | `Bool` | Yes | True if the candidate is a folder. |
| `isArchive` | `Bool` | Yes | True if the candidate is an archive file. |
| `systemIconName` | `String` | Yes | SF Symbol icon name (e.g. `folder.fill`, `archivebox.fill`, `doc.fill`). |
| `matchHighlightRange` | `[Int]` | Yes | Two-element array `[startIndex, length]` of matching prefix substring. |

---

### 1.5 `BreadcrumbSegment` (Idle Pill Path Segment)
Represents a single clickable segment within the idle breadcrumb trail.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `id` | `String` | Yes | Canonical path up to this breadcrumb segment. |
| `title` | `String` | Yes | Display text of the folder (e.g., `"Downloads"`, `"dev"`). |
| `fullURL` | `String` | Yes | Full file URL string of this directory. |
| `isRoot` | `Bool` | Yes | True if this segment represents the root `/` or home folder. |
| `isLast` | `Bool` | Yes | True if this segment is the current active directory. |

---

### 1.6 `AddressBarState` (Aggregate UI Component State)
Full state structure managing the Address & Search bar lifecycle.

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `isEditing` | `Bool` | Yes | Whether the omnibar is currently focused in text edit mode. |
| `text` | `String` | Yes | Current text value inside the input field. |
| `mode` | `AddressBarInputMode` | Yes | Active input mode (`pathNavigation` or `spotlightSearch`). |
| `suggestions` | `[PathSuggestionItem]` | Yes | List of autocompletion suggestions. |
| `selectedIndex` | `Int?` | No | Selected index in the suggestion dropdown (nil if none). |
| `errorMessage` | `String?` | No | Active validation error or non-existent path notice. |
| `breadcrumbs` | `[BreadcrumbSegment]` | Yes | Ordered breadcrumb segments for idle display. |
