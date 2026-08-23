# Quickstart & Validation Guide: Multiplatform SDK & Local CI/CD

**Feature**: `215-multiplatform-sdk-and-dual-license-architecture`  
**Status**: `READY_FOR_VERIFICATION`  

---

## 1. Prerequisites

- **macOS / Linux**: Rust toolchain (`rustc` $\ge 1.80$, `cargo`), Xcode 15+ / Swift 6.0 toolchain, Python 3.10+.
- **Git Repository**: Configured with local hooks.

---

## 2. Setup & Git Hook Verification

Install the zero-cloud local Git hooks:

```bash
# 1. Install Git hooks locally
./scripts/install_local_git_hooks.sh

# 2. Verify pre-push hook permissions
ls -la .git/hooks/pre-push
```

---

## 3. Local CI/CD Gate Verification (Zero Cloud Quota)

Execute the full 4-stage local regression and quality gate:

```bash
# Run all local stages with bail on first error
./scripts/run_local_ci_gate.sh --bail

# Or generate structured JSON verification report
./scripts/run_local_ci_gate.sh --json /tmp/local_ci_report.json
```

---

## 4. End-to-End CLI Validation

Test standalone CLI operations directly against real archives:

```bash
# 1. Build release CLI binary
cargo build --release --manifest-path rust/Cargo.toml --bin ttzip

# 2. Create encrypted 7z archive with solid mode
./rust/target/release/ttzip create -f 7z -l 9 -p "P@ssw0rd2026" -o /tmp/test.7z Sources/

# 3. Test integrity without disk extraction
./rust/target/release/ttzip check /tmp/test.7z

# 4. Stream extraction progress as NDJSON
./rust/target/release/ttzip extract -o /tmp/unpacked -p "P@ssw0rd2026" --json /tmp/test.7z
```

---

## 5. Dual-License Manifest Audit

Validate that all required license headers, `NOTICE`, `LICENSE-BSD`, and `LICENSE-APACHE` are intact:

```bash
test -f LICENSE-BSD && test -f LICENSE-APACHE && test -f NOTICE && echo "✅ Dual-license files verified."
```
