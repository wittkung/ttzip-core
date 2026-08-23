# Quickstart: 196-purge-legacy-c-test-harness-obsolete-cli-and-relic-build-dirs

## Validation Scenarios

### Scenario 1: Verify Purge of Dead Folders
- **Command**: `ls -d cli Tests/c Tests/fuzz build build_asan build_dist scratch 2>/dev/null || echo "All dead directories cleanly purged"`
- **Expected Output**: "All dead directories cleanly purged"

### Scenario 2: Local CI/CD Gate
- **Command**: `./scripts/run_local_ci_gate.sh`
- **Expected Output**: 4/4 stages PASS in $<10\text{s}$.
