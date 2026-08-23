# Quickstart: 197-purge-broken-scripts-and-deduplicate-site-docs

## Validation Scenarios

### Scenario 1: Verify Purge of Duplicates and Broken Scripts
- **Command**: `ls -d site scripts/run_delta_audit.sh docs/index.html 2>/dev/null || echo "All duplicate and broken files purged"`
- **Expected Output**: "All duplicate and broken files purged"

### Scenario 2: Local CI/CD Gate
- **Command**: `./scripts/run_local_ci_gate.sh`
- **Expected Output**: 4/4 stages PASS in $<10\text{s}$.
