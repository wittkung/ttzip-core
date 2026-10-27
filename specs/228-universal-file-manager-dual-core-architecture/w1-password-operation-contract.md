# W1 Contract: Archive Password Validation, Original Operation Resumption & Window Isolation (Q1-ISSUE-07)

**Feature Branch / Spec**: `228-universal-file-manager-dual-core-architecture`  
**Workstream**: W1 (Supplement Q1-ISSUE-07 — Archive Password Verification & Resumption)  
**Status**: Approved Design Contract

---

## 1. Password Evidence Model Across Operations

Many archive formats (most notably standard ZIP with AES-256 or ZipCrypto, as well as 7z/RAR archives without header encryption) store directory listings (Central Directory / file headers) in **plaintext** while encrypting only the file payload streams. Consequently, **succeeding at `inspectArchive` / `openArchiveSession` only proves that the directory catalog is readable; it does NOT prove that a candidate password is valid for encrypted file contents.**

### 1.1 Three-State Password Evidence Classification (`ArchivePasswordEvidenceState`)

| State | Meaning | Permitted UI & Cache Behavior |
| :--- | :--- | :--- |
| **1. `directoryBrowsableUnverified`** | Directory catalog was listed without decrypting encrypted payloads (e.g., plaintext ZIP Central Directory where `entries.contains(where: \.isEncrypted)`). | Archive tree may be browsed in the window. **Must NOT** mark the archive as "Unlocked with password", **must NOT** store any candidate password into `ArchivePasswordStore.shared`, and **must NOT** treat content as verified. |
| **2. `candidate`** | User entered a password in `PasswordPromptSheetView`, selected a vault entry, or passed a password hint, prior to cryptographic payload/header decryption. | Held strictly in transient operation memory (`ArchivePasswordOperationContext`). Never written to `ArchivePasswordStore.shared` until cryptographic verification succeeds. |
| **3. `verified(scope)`** | Cryptographic decryption succeeded:<ul><li>`.headerCatalog`: Encrypted header (`ArchiveEncryptionTier.headerAndData`) was decrypted.</li><li>`.selectedEntries([String])`: Specific encrypted entry payloads were decrypted and CRC/MAC-verified.</li><li>`.fullArchive`: All encrypted entries in the archive were decrypted.</li></ul> | Password may be cached in `ArchivePasswordStore.shared` for `archivePath` and recorded in the window's `explorerState.activePassword`. |

### 1.2 Explicit Four-Way Probe Outcome (`ArchivePasswordProbeOutcome`) & Probe Selection Policy (`ArchivePasswordProbePolicy`)

To prevent conflating verification scope (`ArchivePasswordVerificationScope`) with the unverified browsable state (`ArchivePasswordEvidenceState.directoryBrowsableUnverified`), encrypted payload probing returns a dedicated 4-way outcome:

```swift
public enum ArchivePasswordProbeOutcome: Sendable, Equatable {
    case verified(ArchivePasswordVerificationScope)
    case noAvailableEvidence
    case wrongPassword(message: String)
    case otherFailure(category: ArchivePasswordFailureCategory, message: String)
}
```

1. **Deterministic Probe Entry Selection (`ArchivePasswordProbePolicy.selectProbeEntry`)**:
   - Budget constant: `ArchivePasswordProbePolicy.maxProbeEntryBytes = 64 * 1024 * 1024` (64 MiB).
   - Filters archive entries to `!entry.isDirectory && entry.isEncrypted && entry.uncompressedSize > 0 && entry.uncompressedSize <= maxProbeBytes`.
   - Selects the entry with the **smallest non-zero `uncompressedSize`** (tie-broken deterministically by normalized lexicographical path `entry.path < $1.path`).
   - When no entry satisfies these criteria (e.g., all encrypted entries exceed `64 MiB`, only `0`-byte empty encrypted entries exist, or no encrypted file entries exist), `selectProbeEntry` returns `nil` and the probe returns `.noAvailableEvidence` with **0 calls** to the underlying driver.
2. **Real-Decryption Probe Bypassing Preview Cache (`verifyEncryptedEntryPayload -> Data?`)**:
   - `VFSLz4CachePool.shared.getCachedEntry(archivePath:entryPath:)` is keyed only by `(archivePath, entryPath)` without binding to the candidate password. Therefore, `DefaultArchivePasswordOperationsDriver.verifyEncryptedEntryPayload` **MUST bypass `VFSLz4CachePool.shared`** and perform direct, uncached stream decryption (`extractSingleEntryStreamGuarded`) on the exact normalized `entryPath` using the current candidate `password`, enforcing the `64 MiB` budget and verifying CRC32 / AEAD authentication tags.
   - If the driver returns `nil` (or empty `Data` for a non-empty entry) or the target entry is not found, the probe outcome is `.noAvailableEvidence`.
   - When `.noAvailableEvidence` is returned, the coordinator **MUST NOT** call `applyVerifiedBrowseResult` and **MUST NOT** write the candidate password to `ArchivePasswordStore.shared`; the archive remains in `.directoryBrowsableUnverified` state (`explorerState.activePassword = nil`).
   - Budget-exceeded (`errOutOfMemory`), corrupted archive (`corruptHeader`), unsupported format, and I/O errors are classified as `.otherFailure(category:message:)` and **never** disguised as `.wrongPassword`.
3. **Single Source of Truth (SSOT) for Password Types**:
   - `ArchivePasswordLocaleKey`, `ArchivePasswordVerificationScope`, `ArchivePasswordEvidenceState`, `ArchivePasswordProbeOutcome`, `ArchivePasswordFailureCategory`, `ArchivePasswordSubmissionResult`, and `ArchivePasswordErrorClassifier` are declared in a single canonical file (`Sources/TTZipApp/Services/Windows/ArchivePasswordLocaleKey.swift`) with zero duplicate declarations and zero `#if` macros.

### 1.3 Evidence Required Per Operation Type

1. **Browse Archive (`browseArchive` / `loadArchive`)**:
   - **Plaintext-Directory Encrypted Archive (`ArchiveEncryptionTier.dataOnly`)**:
     - Opening the archive with `password == nil` lists the directory immediately in `.directoryBrowsableUnverified` state (`activePassword = nil`).
     - **No Forced Full Decompression for Browsing**: Opening a plaintext-header archive to browse its directory tree does **not** force decompression of large files.
     - If a candidate password is supplied during browse/unlock, `inspectArchive` alone is **insufficient evidence**. The coordinator selects the smallest non-empty encrypted entry `<= 64 MiB` via `ArchivePasswordProbePolicy.selectProbeEntry` and invokes `passwordDriver.verifyEncryptedEntryPayload(archivePath:entryPath:password:)`:
       - `.wrongPassword`: Rejected as `.wrongPassword`; password cache remains untouched.
       - `.verified(.selectedEntries([probedEntry]))`: Calls `applyVerifiedBrowseResult` and stores the verified password in `ArchivePasswordStore.shared`.
       - `.noAvailableEvidence`: Does **not** call `applyVerifiedBrowseResult` and does **not** write to `ArchivePasswordStore.shared`; keeps the directory browsable in `.directoryBrowsableUnverified` state (`activePassword = nil`).
       - `.otherFailure(category, message)`: Surfaces as `.nonPasswordFailure(category:message:)`; never mislabeled as wrong password.
   - **Header-Encrypted Archive (`ArchiveEncryptionTier.headerAndData`)**:
     - `inspectArchive(archivePath, password: nil)` fails with `ArchiveError.passwordRequired`.
     - Submitting a candidate password requires `inspectArchive(archivePath, password: candidate, autoVaultUnlock: false)` to succeed **plus** (when a qualifying encrypted file entry exists within probe budget) passing the uncached content probe.

2. **Full Archive Extraction (`quickExtractArchive` / `extractArchive`)**:
   - **Evidence Required**: Actual execution of `quickExtract` (with `autoVaultUnlock: false` when verifying an explicit user candidate) completing without a password error.
   - `inspectArchive` is **never** used to validate the password for `quickExtractArchive`.
   - Only when `quickExtract` completes decryption of the archive payload does the candidate password become `.verified(.fullArchive)` and get persisted to `ArchivePasswordStore.shared`.

3. **Selected-Entry / Single-Entry Extraction (`extractEntries` / `extractSingleEntry`)**:
   - **Evidence Required**: Actual execution of `extractEntries` on the captured `entryPaths` completing without a password error.
   - Decrypting a subset of entries proves `.verified(.selectedEntries(entryPaths))` — it does **not** claim that every other entry in a multi-password ZIP shares the same password. If a subsequent extraction of a different entry fails with `.wrongPassword`, the cached password is invalidated and a fresh password prompt is shown.

---

## 2. Original Operation Context (`ArchivePasswordOperationContext`)

When any archive operation encounters `ArchiveError.passwordRequired`, `.passwordRequiredDetailed`, or `.wrongPassword`, the window's `AppViewState` captures an immutable `ArchivePasswordOperationContext` **before** presenting `PasswordPromptSheetView`:

```swift
public struct ArchivePasswordOperationContext: Identifiable, Sendable, Equatable {
    public let operationID: UUID
    public let windowID: UUID
    public let tabID: UUID?
    public let captureEpoch: UInt64
    public let archivePath: String
    public let kind: OperationKind
    public var failedPasswordFingerprints: Set<Int>
    public var isResumingOrCompleted: Bool
}
```

### 2.1 Captured Fields (`OperationKind`)
1. **Window & Session Ownership**:
   - `windowID: UUID` — The owning `AppViewState.windowID`.
   - `tabID: UUID?` — The active `WorkspaceTabSession.id` in that window when the operation was initiated.
   - `operationID: UUID` — Unique identifier for this pending password-gated operation.
   - `captureEpoch: UInt64` — Snapshot of `viewModel.passwordOperationEpoch` at prompt creation.
2. **Archive Identity**:
   - `archivePath: String` — Canonicalized/normalized archive path.
3. **Operation-Specific Parameters**:
   - `.browse(bookmarkSessionID: UUID?)`: Browse / unlock archive catalog in the owning window.
   - `.quickExtract(targetDir: String?, resolvedDestinationDir: String, isSmartExtract: Bool, trashSourceAfterExtract: Bool, collisionPolicyOverride: CollisionPolicy?)`: Full archive extraction preserving the exact target directory, smart-extract flag, post-extract source trash option, and collision policy.
   - `.extractEntries(entryPaths: [String], destinationDir: String, collisionPolicyOverride: CollisionPolicy?)`: Selected-entry or single-entry extraction preserving the **exact `entryPaths` array**, `destinationDir`, and collision policy captured when the operation was triggered.

### 2.2 Strict Context Resumption Rule
- Resuming after password submission **MUST** use `context.kind` (including `context.entryPaths`, `context.destinationDir`, `context.trashSourceAfterExtract`, and `context.collisionPolicyOverride`).
- It **MUST NOT** re-read `viewModel.selectionState.selectedEntryIDs` or `viewModel.navigationState.currentDirectory` (which the user or another action may have changed), and **MUST NOT** downgrade a `.quickExtract` or `.extractEntries` into `loadArchive`, nor turn an `.extractEntries` into a full-archive extraction.

---

## 3. Password Candidate Lifecycle, Caching & Cross-Window Invalidation

1. **Candidate Isolation**:
   - While a password is being tested, it is passed directly as an explicit parameter to the verification/extraction call.
   - Note that `TTZipEngineFacade.inspectArchive` in `TTZipCore` opportunistically writes `explicitPwd` to `ArchivePasswordStore.shared` whenever `reader.inspect` succeeds—even on plaintext-header ZIPs! To prevent `TTZipCore` from polluting `ArchivePasswordStore.shared` with unverified candidates:
     - For plaintext-header archives, `inspectArchive` is called with `password: nil, autoVaultUnlock: false`.
     - Whenever `inspectArchive` is invoked with a candidate password, `AppViewState` snapshots the pre-existing verified password in `ArchivePasswordStore.shared` before the call and immediately restores/clears any unverified write unless content verification also succeeds.
2. **When a Password May Be Cached as Verified**:
   - Only after:
     - `quickExtract` succeeds without password errors, OR
     - `extractEntries` succeeds without password errors, OR
     - `browse` decrypts an encrypted header catalog (`headerAndData`) and/or passes the encrypted content entry probe.
3. **Invalidation on Wrong Password & Multi-Window Credential Protection**:
   - If an operation using a cached password `P_cached` fails with `.wrongPassword`, `AppViewState` invalidates `P_cached` using **compare-and-remove** (`if store.getPassword(for: archivePath) == P_cached { store.removePassword(for: archivePath) }`).
   - **Cross-Window Guarantee**:
     - Invalidating a password for `archiveA.zip` in Window 1 never touches `archiveB.zip` in Window 2.
     - Even if Window 1 and Window 2 both reference `archiveA.zip`, compare-and-remove ensures that if Window 2 already verified and stored `P_correct`, a failing stale attempt in Window 1 with `P_stale` will **not** delete `P_correct`.
   - **No Infinite Auto-Retry Loop**:
     - Each `ArchivePasswordOperationContext` records hashes/fingerprints of failed passwords (`failedPasswordFingerprints` — never storing plaintext passwords).
     - When a cached or vault password fails once for an operation, it is immediately invalidated and added to `failedPasswordFingerprints`; the coordinator transitions directly to interactive `PasswordPromptSheetView` without automatically re-running the failed password.

---

## 4. Terminal States for Retry, Cancel, Archive Switch, Window Close & Stale Callbacks

| Event | Terminal State & Required Behavior |
| :--- | :--- |
| **Wrong Password Submitted** | Returns `.wrongPassword(message:)` to `PasswordPromptSheetView`. `isResumingOrCompleted` resets to `false`. The existing `PasswordPromptSheetView` stays open, displays `message` inline, keeps `pendingPasswordContext` alive for user retry, and **never** spawns a duplicate modal sheet. |
| **Corrupted Archive / Unsupported Format / Permission Error** | Returns `.nonPasswordFailure(category:message:)` (distinct from `.wrongPassword`). `pendingPasswordContext` is cleared, `showPasswordPrompt` closes, and the specific error (Corrupted Archive / Unsupported Format / Permission Denied) is reported in `taskState.statusMessage` (or inline if appropriate) without claiming "Incorrect password". |
| **Duplicate Submit / Rapid Enter Key** | While `isVerifying` or `context.isResumingOrCompleted` is `true`, additional `submitPassword` calls for the same `operationID` are ignored (`at-most-once` resumption guarantee). |
| **User Cancels Password Prompt (`cancelPasswordPrompt`)** | Increments `passwordOperationEpoch`, clears `pendingPasswordContext = nil`, sets `showPasswordPrompt = false`, and sets `taskState.statusMessage = "Decryption cancelled"`. If the archive has already-loaded plaintext directory entries, those entries remain browsable in `.directoryBrowsableUnverified` state; if no entries were loaded, `currentArchivePath` is cleared. Any in-flight verification task whose `operationID` / `captureEpoch` no longer matches is discarded upon completion. |
| **User Switches Archive or Navigates Away in Same Window** | Loading a different archive or closing the current archive increments `passwordOperationEpoch` and invalidates any pending `ArchivePasswordOperationContext` for the previous archive so a late async callback cannot dismiss a new prompt or resume an obsolete extraction. |
| **Window Closes (`prepareForWindowClose`)** | Increments `passwordOperationEpoch`, cancels any in-flight password verification/resumption task owned by the window, clears `pendingPasswordContext = nil`, and resolves any pending `collisionPrompt` continuation with `.stop`. Other windows' password prompts and tasks are unaffected. |

---

## 5. Preservation of Collision Handling, Source Cleanup & Completion Notifications

When an extraction operation (`.quickExtract` or `.extractEntries`) is resumed after password verification:
1. **Collision Handling**: Uses the window-owned `CollisionPromptPresenter` (or explicit `CollisionHandling` policy from `context`), routing any interactive collision prompts strictly to the owning window's `overlayState.collisionPrompt`.
2. **Terminal Status & Source Archive Cleanup (`trashSourceAfterExtract`)**:
   - **Completed (`.completed`)**: Updates `taskState.statusMessage = "Extraction complete: ..."`, reveals output in Finder (when configured), and moves the source archive to Trash **only if** `trashSourceAfterExtract == true` and extraction succeeded completely.
   - **Cancelled / Stopped / Partial Failure (`.cancelled`, `.extractionStopped`, `.partialExtractionFailure`)**:
     - Never displays a normal completion status message.
     - **Never** deletes/trashes the source archive even if `trashSourceAfterExtract == true`.
     - Reports the exact cancelled/stopped/partial-failure status in the owning window's `taskState`.
3. **Security & Privacy Invariant**:
   - Plaintext passwords are **never** logged, included in task IDs, written to status messages, or persisted in `ArchivePasswordOperationContext` descriptions.

---

## 6. Acceptance Criteria & Verification Matrix

1. **Plaintext-Directory Encrypted ZIP**: `inspectArchive` lists entries in `.directoryBrowsableUnverified` state, but submitting a wrong password fails verification, does not mark the archive unlocked, and does not write to `ArchivePasswordStore.shared`.
2. **Wrong Password -> Correct Password Full Extraction**: Submitting a wrong password shows an inline error inside the same prompt; subsequently submitting the correct password resumes the original full extraction **exactly once** and produces bit-exact output (verified by SHA-256).
3. **Selected-Entry Extraction Resumption**: Triggering extraction of selected entries `["docs/a.txt"]` to `destDir` with policy `.keepBoth`, failing on missing/wrong password, changing UI selection while the prompt is open, and then submitting the correct password extracts **only** `["docs/a.txt"]` to `destDir` with `.keepBoth` (never full-archive extraction or the mutated selection).
4. **Rapid Submit, Cancel, Window Close & Archive Switch**: Rapid double-submit starts at most 1 extraction; cancelling the prompt, switching archives, or closing the window prevents any late async verification result from resuming the cancelled operation or closing a newer prompt.
5. **Dual-Window Encrypted Archive Isolation**: Window A (`encA.zip`) and Window B (`encB.zip`) handle password prompts, wrong-password errors, cancellations, and credential invalidation independently without cross-window state mutation.
6. **Stale Verified Password Invalidation Without Retry Loop**: If a previously cached password in `ArchivePasswordStore.shared` fails during extraction (e.g., archive replaced or entry uses a different password), it is evicted via compare-and-remove and opens `PasswordPromptSheetView` once without entering an infinite retry loop.
7. **Distinct Error Classification**: Corrupted archives (`.corruptHeader`, `.corruptedData`), budget-exceeded entries (`errOutOfMemory`), unsupported formats/encryption methods (`.unsupportedEncryptionMethod`, `.invalidFormat`), and permission errors (`EACCES` / `fileReadNoPermission`) are classified and displayed with their respective error messages—never mislabeled as "Incorrect password".
8. **Cancelled / Stopped / Partial Failure Safety**: Cancelled, stopped, or partially failed extractions never display normal completion and never trash the source archive.
9. **Probe Selection Boundaries (All Over-Budget / Only Empty / Mixed Sizes)**:
   - When all encrypted entries exceed `64 MiB` (`全部条目超限`) or when only `0`-byte encrypted entries exist (`只有空条目`), the probe makes `0` driver calls, retains `.directoryBrowsableUnverified`, does not call `applyVerifiedBrowseResult`, and leaves `ArchivePasswordStore.shared` `nil`.
   - When empty, small, and oversized encrypted entries are mixed (`大小条目混合`), the probe deterministically selects the smallest non-empty entry `<= 64 MiB` and makes `1` driver call on that exact entry.
10. **Probe Returns `nil` (`noAvailableEvidence`)**: When `verifyEncryptedEntryPayload` returns `nil` (or empty `Data`), `applyVerifiedBrowseResult` is never called, `ArchivePasswordStore.shared` remains `nil`, and the window stays in `.directoryBrowsableUnverified`.
11. **Preview Cache Poisoning Resistance**: Populating `VFSLz4CachePool.shared` with decrypted entry data using the correct password and subsequently submitting a **wrong password** (after clearing `ArchivePasswordStore.shared`) still rejects the wrong password as `.wrongPassword` and leaves `ArchivePasswordStore.shared` `nil`.

