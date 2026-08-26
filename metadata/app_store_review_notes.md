# Apple App Store Reviewer Notes (审核备注与演示指南)

Dear Apple Review Team,

Thank you for reviewing **TTZip**. 

### Application Overview
TTZip is a high-performance native macOS archiver built specifically for Apple Silicon and macOS 14.0+. It provides file compression, decompression, and in-archive Quick Look previews across standard archive formats (ZIP, 7Z, TAR, GZ, RAR, etc.).

### Key Architectural & Compliance Information
1. **App Sandbox Compliance**: The application is strictly sandboxed. All file system accesses are initiated via standard macOS file open/save panels (`NSOpenPanel` / `NSSavePanel`) with standard security-scoped URL bookmarks.
2. **Zero Network Calls / Zero Data Tracking**: The app does not collect user data, track telemetry, or transmit any network requests.
3. **No Third-Party Framework Dependencies**: Zero dynamic analytics SDKs. Uses native Apple technologies (SwiftUI, AppKit, CryptoKit, UniformTypeIdentifiers, QuickLookThumbnailing).
4. **Offline Full Feature Capability**: There are no hidden paywalls, account registrations, or in-app purchase gates. Purchasing the upfront application in the Mac App Store unlocks 100% of all features permanently.

### Testing Instructions for Reviewer
1. Launch TTZip.
2. Drag and drop any folder or file into the main drop zone, select "ZIP" or "7Z", and click "Start Compression" to verify archive creation.
3. Double click on any created `.zip` or `.7z` file to explore directory contents via the native Miller Column view.
4. Highlight any file inside the archive and press `Spacebar` to test the in-archive Quick Look preview.

Should you have any questions or require further assistance, please contact us at `witt.w.kung@gmail.com`.

Warm regards,  
TTZip Development Team
