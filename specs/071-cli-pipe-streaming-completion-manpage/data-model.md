# Data Model: CLI UNIX Pipe Streaming, Shell Auto-Completion, BSD Man Page, and Local CI/CD Gate

**Feature Directory**: `specs/071-cli-pipe-streaming-completion-manpage`  
**Creation Date**: 2026-08-17  
**Status**: Formalized & Validated

---

## 1. Core Data Structures & Types

### 1.1 `StreamExecutionMode`
Defines the I/O channel topology for archive creation, extraction, and inspection commands.

```swift
public enum StreamExecutionMode: String, Sendable, Equatable, CaseIterable {
    /// Reading and writing to regular filesystem paths.
    case directFile = "directFile"
    /// Reading archive bytes from stdin, writing extracted files to disk.
    case standardInputPipe = "standardInputPipe"
    /// Reading files from disk, writing compressed archive bytes to stdout.
    case standardOutputPipe = "standardOutputPipe"
    /// Reading archive from stdin, streaming output or telemetry to stdout/stderr.
    case duplexPipe = "duplexPipe"
    /// Extracting a single archive entry directly to stdout without disk intermediate.
    case singleEntryStdout = "singleEntryStdout"
}
```

### 1.2 `StreamPipelineConfig`
Strongly typed configuration driving stream execution pipelines.

```swift
public struct StreamPipelineConfig: Sendable, Equatable {
    public let mode: StreamExecutionMode
    public let inputPath: String?
    public let outputPath: String?
    public let singleEntryName: String?
    public let forceBinary: Bool
    public let progressRouting: StreamProgressRouting
    public let streamBlockSize: Int
    
    public init(
        mode: StreamExecutionMode,
        inputPath: String? = nil,
        outputPath: String? = nil,
        singleEntryName: String? = nil,
        forceBinary: Bool = false,
        progressRouting: StreamProgressRouting = .standardError,
        streamBlockSize: Int = 65536
    ) {
        self.mode = mode
        self.inputPath = inputPath
        self.outputPath = outputPath
        self.singleEntryName = singleEntryName
        self.forceBinary = forceBinary
        self.progressRouting = progressRouting
        self.streamBlockSize = streamBlockSize
    }
}
```

### 1.3 `StreamProgressRouting`
Controls where interactive UI components (progress bars, spinners, ANSI color badges) are written.

```swift
public enum StreamProgressRouting: String, Sendable, Equatable, CaseIterable {
    /// All progress indicators suppressed (e.g. non-interactive script or --quiet).
    case suppressed = "suppressed"
    /// Redirected to stderr to keep stdout binary data stream 100% untainted.
    case standardError = "standardError"
    /// Written to stdout directly (only valid when stdout is a TTY and not streaming binary).
    case inlineTty = "inlineTty"
}
```

### 1.4 `ShellTarget`
Target shell dialects supported by the dynamic completion generator.

```swift
public enum ShellTarget: String, Sendable, Equatable, CaseIterable {
    case zsh = "zsh"
    case bash = "bash"
    case fish = "fish"
    case nushell = "nushell"
}
```

### 1.5 `LocalCIGateStage` & `LocalCIGateReport`
Represents the stages and final outcome of the 6-stage local CI gate runner.

```swift
public enum GateStatus: String, Sendable, Equatable, CaseIterable {
    case pass = "pass"
    case fail = "fail"
    case skip = "skip"
}

public struct LocalCIGateStage: Sendable, Equatable {
    public let stageIndex: Int
    public let name: String
    public let command: String
    public let status: GateStatus
    public let durationSeconds: Double
    public let diagnosticMessage: String?
}

public struct LocalCIGateReport: Sendable, Equatable {
    public let totalStages: Int
    public let passedStages: Int
    public let failedStages: Int
    public let totalDurationSeconds: Double
    public let isSuccess: Bool
    public let stages: [LocalCIGateStage]
}
```

---

## 2. Bidirectional Schema Mapping

| Swift Struct / Enum | Schema Contract File | JSON Type | Validation Rule |
| :--- | :--- | :--- | :--- |
| `StreamPipelineConfig` | `contracts/stream_pipeline_config.json` | `object` | `mode`, `progressRouting`, `forceBinary` required |
| `ShellTarget` | `contracts/shell_completion_request.json` | `string` | Enum `["zsh", "bash", "fish", "nushell"]` |
| `LocalCIGateReport` | `contracts/local_ci_gate_report.json` | `object` | `totalStages`, `passedStages`, `isSuccess`, `stages` required |
