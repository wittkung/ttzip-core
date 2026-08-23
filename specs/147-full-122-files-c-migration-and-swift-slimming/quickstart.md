# Quickstart: 147-full-122-files-c-migration-and-swift-slimming

## Validation Scenarios

### Scenario 1: Multi-Volume Split & In-Place Mutation Test
- **Command**: `./build/ttzip-cli --benchmark`
- **Expected Output**: Full microkernel throughput table across all SOTA codecs, VFS, and security modules.
- **Failure Diagnostic**: Check split volume naming boundaries and file descriptors.

### Scenario 2: Full Local CI Verification
- **Command**: `./scripts/local-ci.sh`
- **Expected Output**: `ALL LOCAL CI CHECKS PASSED SUCCESSFULLY (0 Quota)`
- **Failure Diagnostic**: Review test logs.
