# Quickstart: 198-modernize-multilingual-readmes-and-homebrew-formulas

## Validation Scenarios

### Scenario 1: Verify Purge of Chinese Install Script
- **Command**: `[ ! -f "安装TTZip.command" ] && echo "Cleanly purged"`
- **Expected Output**: "Cleanly purged"

### Scenario 2: Verify Formula Matching
- **Command**: `grep "darwin" Formula/ttzip.rb && grep "darwin" Formula/ttzip-cli.rb`
- **Expected Output**: Both formulas reference darwin tarball.

### Scenario 3: Local CI/CD Gate
- **Command**: `./scripts/run_local_ci_gate.sh`
- **Expected Output**: 4/4 stages PASS in $<10\text{s}$.
