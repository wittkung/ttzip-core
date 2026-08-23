# Quickstart & Verification Guide: Feature 071

**Feature Directory**: `specs/071-cli-pipe-streaming-completion-manpage`  
**Purpose**: Validation procedures for UNIX Pipe streaming, Shell completion generation, BSD Man Page formatting, and Local CI gate runner.

---

## Scenario 1: UNIX Pipe Streaming Roundtrip (stdout -> stdin)

### Command
```bash
# Compress a directory to stdout pipe and decompress from stdin pipe to target
mkdir -p /tmp/ttzip_stream_src /tmp/ttzip_stream_dst
echo "TTZip Pipeline Data" > /tmp/ttzip_stream_src/payload.txt
ttzip-cli create -f tar.zst -o - /tmp/ttzip_stream_src | ttzip-cli extract -i - -d /tmp/ttzip_stream_dst
cat /tmp/ttzip_stream_dst/payload.txt
```

### Expected Output
```text
TTZip Pipeline Data
```

### Failure Diagnostic
- If exit code 141 occurs: downstream consumer exited early before reading was complete.
- If archive corruption error occurs: ensure progress bars or logs were not emitted to `stdout`.

---

## Scenario 2: Single-Entry Stdout Extraction (`cat` / `extract -O -`)

### Command
```bash
ttzip-cli create -f zip -o /tmp/sample.zip /tmp/ttzip_stream_src/payload.txt
ttzip-cli cat /tmp/sample.zip payload.txt
```

### Expected Output
```text
TTZip Pipeline Data
```

### Failure Diagnostic
- If error indicates `"stdout is a terminal; binary output suppressed"`: pass `-f`/`--force` to override TTY safety detection.

---

## Scenario 3: Dynamic Shell Completion Generation

### Command
```bash
ttzip-cli completion zsh | head -n 15
ttzip-cli completion fish | head -n 15
```

### Expected Output
```text
#compdef ttzip-cli ttzip
...
_arguments -C \
  '1: :->command' \
  '*:: :->args'
```

### Failure Diagnostic
- Verify that `#compdef ttzip-cli` header is present and format completions contain all 16 supported formats (`zip`, `7z`, `tar.zst`, `tar.gz`, etc.).

---

## Scenario 4: BSD Man Page Generation & Validation

### Command
```bash
ttzip-cli man | mandoc -Tlint
```

### Expected Output
```text
(No output or warnings emitted, indicating 100% clean mdoc syntax)
```

### Failure Diagnostic
- Check for unmatched `.Bl` / `.El` blocks or unescaped macro names.

---

## Scenario 5: Local CI/CD Automated Test Gate

### Command
```bash
./scripts/run_local_ci_gate.sh
```

### Expected Output
```text
================================================================================
   🚀 [TTZip Local CI/CD Industrial Gate] Execution Summary
================================================================================
+----+--------------------------------------------------+----------+----------+
| ST | STAGE NAME                                       | STATUS   | DURATION |
+----+--------------------------------------------------+----------+----------+
| 01 | Build Release Binary (ttzip-cli)                 |  PASS    | ...      |
| 02 | Standards Compliance Suite (--standard all)      |  PASS    | ...      |
| 03 | Differential Oracle Suite (--differential all)   |  PASS    | ...      |
| 04 | Malformed Stream Fuzzing Gate (--fuzz)           |  PASS    | ...      |
| 05 | Performance Floor (XCTestPerformanceMeasure)     |  PASS    | ...      |
| 06 | Pipeline Stream E2E SHA-256 Roundtrip            |  PASS    | ...      |
+----+--------------------------------------------------+----------+----------+
   🏆 [ALL 6 GATES PASSED] Total Pipeline Duration: ...
================================================================================
```

### Failure Diagnostic
- Check output stage breakdown to identify which of the 6 sub-gates failed.
