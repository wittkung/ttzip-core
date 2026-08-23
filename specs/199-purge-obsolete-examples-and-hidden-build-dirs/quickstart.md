# Quickstart: 199-purge-obsolete-examples-and-hidden-build-dirs

## Validation Scenarios

### Scenario 1: Verify Purge of examples/ and .build_*
- **Command**: `ls -d examples .build_custom .build_di_test .build_tmp 2>/dev/null || echo "All purged cleanly"`
- **Expected Output**: "All purged cleanly"

### Scenario 2: Local CI/CD Gate
- **Command**: `./scripts/run_local_ci_gate.sh`
- **Expected Output**: 4/4 stages PASS in $<10\text{s}$.
