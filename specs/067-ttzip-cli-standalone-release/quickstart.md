# Quickstart & Verification Guide: ttzip-cli Standalone

## Scenario 1: CLI Help & Version Verification
- **Command**:
  ```bash
  swift run ttzip-cli --version
  ```
- **Expected Output**:
  ```text
  ttzip-cli version 1.0.0 (Apple Silicon M-Series & x86_64)
  ```
- **Failure Diagnostic**:
  Ensure SPM product `ttzip-cli` is registered in `Package.swift`.

---

## Scenario 2: High-Speed Archive Creation & Inspection Roundtrip
- **Command**:
  ```bash
  echo "Hello TTZip CLI Engine" > /tmp/sample.txt
  swift run ttzip-cli create /tmp/test.tar.zst /tmp/sample.txt -l 3
  swift run ttzip-cli inspect /tmp/test.tar.zst
  ```
- **Expected Output**:
  ```text
  [CLI-Event] ✅ 打包完成: test.tar.zst
  ================================================================
  归档文件: test.tar.zst (ZSTD 格式)
  包含条目: 1
  ================================================================
  ```

---

## Scenario 3: Release Packaging Verification
- **Command**:
  ```bash
  ./scripts/package_cli.sh
  ```
- **Expected Output**:
  ```text
  [SUCCESS] Packaged release binary: build_dist/ttzip-cli-v1.0.0-macos-universal.tar.gz
  SHA256: [64-hex-char hash]
  ```
- **Failure Diagnostic**:
  Verify Xcode command line tools and `lipo` / `strip` are present.
