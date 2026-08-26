# Feature Specification: 179-sink-path-defense-charset-scanner-fuzz-to-rust

## 1. Executive Summary & Strategic Motivation
Following the fourth-round comprehensive codebase audit, several critical infrastructure and security layers in `Sources/TTZipCore` and `Sources/TTZipBench` still suffer from platform lock-in (e.g. Apple CoreFoundation / Darwin sysctl), memory safety hazards (Swift `withUnsafeBytes` pointer escape UAF, Dead-Store elimination on key zeroing), subtle bugs (SHA-256 hex string truncation), and heap allocation overhead in path sanitization and directory tree walking.

This feature sinks these remaining 6 critical domains into **Safe Rust (`rust/ttzip-glue`)**:
1. **Zero-Allocation Path Sanitizer & ZipSlip Defense (`ttzip-glue::security::path_sanitizer`)**:
   - Single-pass byte-level `.` / `..` traversal normalization with explicit boundary escape reporting.
   - Strict Windows reserved device filter (`CON`, `PRN`, `AUX`, `NUL`, `COM0..9`, `LPT0..9` with trailing space/dot stripping).
   - NTFS Alternate Data Stream (ADS `::$DATA`) extraction and invalid control character sanitization.
   - Unicode NFC canonical mapping without Darwin Foundation dependency.
2. **Bigram Statistical CJK Charset Sniffing & Transcoding (`ttzip-glue::charset`)**:
   - Two-byte Bigram frequency transition model distinguishing GBK/GB18030, Shift-JIS, Big5, EUC-KR, Windows-1252, and UTF-8.
   - SIMD-accelerated zero-allocation transcoding via `encoding_rs`, completely removing Apple `CoreFoundation` dependency.
3. **Streaming Cauchy RS-FEC & Fixed Recovery Records (`ttzip-glue::crypto::rs_fec`)**:
   - Streamed chunk-by-chunk Reed-Solomon generation and repair, replacing all-in-memory $O(N)$ RAM loading with $O(K \times \text{chunk})$ bounded buffers.
   - Elimination of Swift pointer escape UAF hazards and bug-fix for 32-byte raw binary SHA-256 recovery root digest.
4. **Multi-Threaded Parallel Directory Scanner & Loop Guard (`ttzip-glue::fs::scanner`)**:
   - Rayon work-stealing parallel recursive tree walking with `(dev_id, inode)` DAG cycle detection for hardlinks/symlinks.
   - Configurable hidden file, VCS, and OS metadata inclusion flags.
5. **SIMD 16B Fast Hex Diff & Mutation Fuzzing Harness (`ttzip-glue::testing`)**:
   - 16-byte SIMD vectorized differential comparison generating colorized ASCII difference logs.
   - In-memory mutation fuzzing operators (>15,000 iter/s) for continuous local security evaluation.
6. **Hardware-Guaranteed Memory Zeroize & Dynamic CPUID Topology (`ttzip-glue::platform`)**:
   - Compiler-barrier `zeroize` preventing Dead-Store Elimination of decrypted keys and passwords.
   - Runtime dynamic CPUID instruction sniffing and P/E core topology discovery.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Path Traversal & ZipSlip Injection Attack Defense
- **Given** an archive containing malicious paths like `../../../../etc/passwd` or `CON .txt::$DATA`
- **When** the archive is inspected or extracted
- **Then** the Rust path sanitizer flags `has_traversal_attack = true` or strips ADS streams, ensuring safe in-sandbox landing without panics.

### User Scenario 2: Legacy CJK Archive Opening Without Mojibake
- **Given** a legacy zip created on Japanese Windows (Shift-JIS) or Taiwanese Windows (Big5)
- **When** opened on macOS or Linux
- **Then** the bigram statistical detector identifies the encoding with >95% confidence and transcodes file paths to valid UTF-8 without Apple CoreFoundation.

### User Scenario 3: Bounded RAM Self-Healing on Large Archives
- **Given** a 10GB archive with recovery records
- **When** generating or validating recovery records
- **Then** the operation uses constant $<32\text{MB}$ memory stream chunks with zero pointer escape hazards and 100% accurate 32-byte SHA-256 verification.

---

## 3. Success Metrics
1. **Memory Safety**: 0 dangling pointers, 0 dead-store eliminations, 0 unbounded allocations.
2. **Cross-Platform Independence**: 0 calls to `CoreFoundation` or Darwin-only private APIs for charset/path/memory handling.
3. **Directory Walk Speed**: 100,000-file directory scan completed in $<150\text{ms}$ using Rayon multi-threading.
4. **Zero Regression**: 100% pass rate across 200+ Rust tests, 872+ Swift tests, and 7/7 local CI stages.
