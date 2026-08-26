# Quickstart & Verification Guide: Two-Repository Topology

**Feature**: `216-repository-split-and-independent-release-pipeline`  
**Status**: `READY_FOR_VERIFICATION`  

---

## 1. Split Execution

Run the deterministic repository splitter:

```bash
# Execute local two-repository physical split
./scripts/split_repositories.sh
```

---

## 2. Verify `ttzip-core` (Standalone SDK & CLI)

```bash
cd ../ttzip-core

# 1. Install local Git hooks
./scripts/install_local_git_hooks.sh

# 2. Run local CI regression gate (0 cloud cost)
./scripts/run_local_ci_gate.sh --bail

# 3. Test standalone CLI
./rust/target/release/ttzip doctor
```

---

## 3. Verify `ttzip-apple` (Apple Client Applications)

```bash
cd ../ttzip-apple

# 1. Install local Git hooks
./scripts/install_local_git_hooks.sh

# 2. Run local UI test suite
swift test
```
