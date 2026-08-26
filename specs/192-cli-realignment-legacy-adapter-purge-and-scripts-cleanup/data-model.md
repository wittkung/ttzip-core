# Data Model: 192-cli-realignment-legacy-adapter-purge-and-scripts-cleanup

## 1. Clean SPM Target Architecture
- **`TTZipCore` Target**:
  - `ArchiveWriter`, `ArchiveExtractor`, `ArchiveReader`, `ArchiveRepairEngine`
  - `PasswordVaultManager`, `PasswordRecoveryEngine`, `ReedSolomonFEC`
  - `TTZipLocalizationManager`, `Platform*`
- **`TTZipCLI` Target**:
  - `POSIXCLIArgumentParser`, `CLIOptions`, `CLICommandRouter`
  - `ManPageGenerator`, `ShellCompletionGenerator`, `TerminalRenderEngine`
- **`TTZipBench` Target**:
  - `main.swift` (105 LOC Rust FFI caller)
- **`TTZipApp` Target**:
  - SwiftUI/AppKit GUI and macOS system integration
