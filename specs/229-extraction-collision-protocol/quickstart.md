# Quickstart: Verifying the Extraction Name-Collision Protocol

**Feature**: `229-extraction-collision-protocol`

All commands run from the repository root.

## 1. Automated gates

```bash
# Rust: collision corpus (>= 200 scenarios), fault injection, property tests
cargo test --manifest-path products/ttzip/core/rust/Cargo.toml -p ttzip-archive collision
cargo test --manifest-path products/ttzip/core/rust/Cargo.toml -p ttzip-engine extraction_session
cargo clippy --manifest-path products/ttzip/core/rust/Cargo.toml --workspace -- -D warnings

# Existing suites that must stay green
cargo test --manifest-path products/ttzip/core/rust/Cargo.toml -p ttzip-engine \
  --test libarchive_differential_oracle_tests --test container_matrix_tests

# Contracts
bash products/ttzip/core/.specify/scripts/bash/lint-contracts.sh \
  products/ttzip/core/specs/229-extraction-collision-protocol/contracts

# Swift
bazel test //products/ttzip/apple:all_tests --cache_test_results=no
python3 tools/ci/lint_loc_gate.py && python3 tools/ci/lint_code_quality.py
```

Expected: all green; the corpus test prints the scenario count (≥ 200) and `unchanged_without_decision = 0`.

## 2. No silent overwrite remains (static check)

```bash
# No decoder or orchestrator entry point accepts a raw destination path
git grep -n "fn extract_archive_with_sheet\|fn extract_selected_entries" -- products/ttzip/core/rust/crates/ttzip-archive/src/archive/unified
#   -> signatures take `&StagingRoot`

# The removed field is gone everywhere
git grep -n "overwrite_existing" -- products/ttzip
#   -> no matches

# The old promotion code is gone
git grep -n "promote_dir_contents\|StagingSandbox" -- products/ttzip
#   -> no matches
```

## 3. Manual check in the app (5 minutes)

Prepare a fixture:

```bash
mkdir -p /tmp/ttzip229/src/docs && cd /tmp/ttzip229/src
printf 'archive v1\n' > notes.txt; printf 'pdf\n' > docs/report.pdf
zip -r ../Project.zip notes.txt docs >/dev/null
mkdir -p /tmp/ttzip229/dest && printf 'my edits\n' > /tmp/ttzip229/dest/notes.txt
```

1. Launch `products/ttzip/apple/dist/TTZip.app`, open `/tmp/ttzip229/Project.zip`, choose **Extract to…** `/tmp/ttzip229/dest`.
   Expected: the collision panel appears before anything changes; it shows `notes.txt`, both sizes and dates, `1 / 1`. Return selects **Keep Both**.
2. Choose **Keep Both** → `dest/notes.txt` still contains `my edits`; `dest/notes 2.txt` contains `archive v1`; `dest/docs/report.pdf` exists.
3. Repeat and choose **Skip** → `dest/notes.txt` unchanged; no new numbered file.
4. Repeat and choose **Stop** → `ls -la /tmp/ttzip229/dest` identical to before the attempt (no `.ttzip_staging_*`).
5. Repeat and choose **Replace** → `dest/notes.txt` contains `archive v1`.
6. In the archive browser, extract the single entry `docs/report.pdf` into `dest` twice and choose **Keep Both** the second time → `dest/report.pdf` and `dest/report 2.pdf`.
7. Settings → General → "When extracted items already exist" = **Replace**; extract again → no panel; `notes.txt` replaced.
8. Finder → right-click `Project.zip` → **Extract Here** twice → no panel; second run produces numbered items.

## 4. Crash recovery check

```bash
# Start an extraction of a large archive into /tmp/ttzip229/dest, then:
pkill -9 TTZip
ls -a /tmp/ttzip229/dest | grep ttzip_staging     # staging present
open products/ttzip/apple/dist/TTZip.app          # relaunch
ls -a /tmp/ttzip229/dest | grep ttzip_staging     # -> nothing
```

## 5. Performance check (SC-004, SC-005)

```bash
cargo bench --manifest-path products/ttzip/core/rust/Cargo.toml -p ttzip-engine --bench bench_extract_collision
```

Expected: no-conflict extraction within 3 % of the `main` baseline recorded in the bench output; preflight for 100k overlapping entries ≤ 500 ms.
