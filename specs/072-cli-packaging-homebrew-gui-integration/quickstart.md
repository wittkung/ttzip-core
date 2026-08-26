# Quickstart & Validation Guide: 072-cli-packaging-homebrew-gui-integration

## Scenario 1: Standalone CLI Release Packaging

### Command
```bash
./scripts/package_cli_release.sh --version 1.0.0 --output-dir /tmp/ttzip_dist
```

### Expected Output
```
======================================================================
           TTZip Standalone CLI Packaging & Homebrew Tap Pipeline      
======================================================================
Version:      1.0.0
Architecture: Universal 2 (arm64 + x86_64)
Output Dir:   /tmp/ttzip_dist

[1/5] Compiling Release Binary...
  ➔ Slices: arm64 x86_64 (Universal 2 Mach-O)
[2/5] Extracting Debug Symbols & Stripping Local Symbols...
  ➔ Generated dSYM: /tmp/ttzip_dist/ttzip-cli.dSYM
  ➔ Stripped:       /tmp/ttzip_dist/bin/ttzip-cli
[3/5] Self-Generating Man Page & Completion Scripts...
  ➔ Man page:   share/man/man1/ttzip-cli.1
  ➔ Zsh:        share/zsh/site-functions/_ttzip-cli
  ➔ Bash:       share/bash-completion/completions/ttzip-cli
  ➔ Fish:       share/fish/vendor_completions.d/ttzip-cli.fish
[4/5] Creating Clean Release Tarball...
  ➔ Tarball: /tmp/ttzip_dist/ttzip-cli-v1.0.0-darwin-universal.tar.gz
  ➔ SHA-256: [64-char hex string]
[5/5] Generating Homebrew Formula...
  ➔ Formula: Formula/ttzip-cli.rb

✅ Standalone CLI release packaged successfully!
```

### Failure Diagnostic
- If `lipo` fails: Verify Xcode command line tools via `xcode-select -p`.
- If `._*` files appear in tarball: Verify `COPYFILE_DISABLE=1` is exported.

---

## Scenario 2: Homebrew Formula Verification

### Command
```bash
swift test --filter CLIPackagingTests
```

### Expected Output
```
Test Suite 'CLIPackagingTests' passed at ...
  Executed 3 tests, with 0 failures in 0.05s
```

### Failure Diagnostic
- If SHA-256 mismatch occurs: Assert tarball content was not modified after checksum calculation.

---

## Scenario 3: GUI Inspector View Model Diagnostic

### Command
```bash
swift test --filter ArchiveInspectorViewTests
```

### Expected Output
```
Test Suite 'ArchiveInspectorViewTests' passed at ...
  Executed 4 tests, with 0 failures in 0.08s
```

### Failure Diagnostic
- If UI scan timeout occurs: Ensure `Task.detached` uses `.mappedIfSafe` data reading.
