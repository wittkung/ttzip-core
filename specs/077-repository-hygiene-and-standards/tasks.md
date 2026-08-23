# Tasks: Repository Hygiene, Governance Standards, and Contribution Architecture

**Input**: Design documents from `specs/077-repository-hygiene-and-standards/`
**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/`, `quickstart.md`

## Format: `[ID] [P?] [Story] Description`
- **[P]**: Parallelizable task (independent files)
- **[Story]**: User story tag (US1, US2, US3, US4)
- Clear, unambiguous file paths in every task

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Initialize directory structures for governance documents and templates

- [x] T001 Create governance directory in `docs/governance/`
- [x] T002 [P] Ensure `.github/ISSUE_TEMPLATE/` directory exists

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Establish contract validation assertions before creating templates and scripts

- [x] T003 Validate contract definitions against JSON Schema Draft-07 in `specs/077-repository-hygiene-and-standards/contracts/`


---

## Phase 3: User Story 1 - Standardized Issue Reporting & PR Contribution Workflow (Priority: P1) 🎯 MVP

**Goal**: Provide structured YAML Issue Forms and an enforceable multi-gate PR template for all bug reports, performance regressions, and feature proposals.

**Independent Test**: Validate YAML syntax of all issue forms and check that PR template contains all 5 verification gates.

- [x] T004 [P] [US1] Create Bug Report Form in `.github/ISSUE_TEMPLATE/bug_report.yml`
- [x] T005 [P] [US1] Create Performance Regression Form in `.github/ISSUE_TEMPLATE/performance_regression.yml`
- [x] T006 [P] [US1] Create Feature Request Form in `.github/ISSUE_TEMPLATE/feature_request.yml`
- [x] T007 [P] [US1] Create Issue Config in `.github/ISSUE_TEMPLATE/config.yml`
- [x] T008 [US1] Create Pull Request Template with 5 verification gates and benchmark delta table in `.github/pull_request_template.md`

**Checkpoint**: User Story 1 complete — all issue templates and PR template ready.

---

## Phase 4: User Story 2 - Comprehensive Repository Cleanliness & `.gitignore` / `.gitattributes` Hardening (Priority: P1)

**Goal**: Ensure zero untracked temporary build outputs, benchmark dumps, or vendor upstream checkouts pollute the repository, and configure LF normalization with Linguist overrides.

**Independent Test**: Execute `git status` to verify temporary directories and `.DS_Store` are excluded, and inspect `.gitattributes` Linguist attributes.

- [x] T009 [P] [US2] Update and harden `.gitignore` in `.gitignore` to cover `.build_*/`, `Vendor/*-upstream/`, benchmark scratch, and OS metadata
- [x] T010 [P] [US2] Create and configure `.gitattributes` in `.gitattributes` with `* text=auto eol=lf`, binary patterns, and Linguist overrides
- [x] T011 [US2] Clean up workspace of any stray `.DS_Store` files and verify clean worktree state

**Checkpoint**: User Story 2 complete — repository hygiene and ignore/attribute configuration fully hardened.


---

## Phase 5: User Story 3 - Formalized Git Branching Strategy & Release Lifecycle (Priority: P2)

**Goal**: Document and enforce the Git branching taxonomy (`feat/`, `perf/`, `fix/`, `upstream/`, `release/`) and Conventional Commits standards.

**Independent Test**: Verify markdown structure and guidelines in `docs/governance/BRANCHING_STRATEGY.md` and `CONTRIBUTING.md`.

- [x] T012 [P] [US3] Create Branching Strategy and Commit Conventions guide in `docs/governance/BRANCHING_STRATEGY.md`
- [x] T013 [US3] Update `CONTRIBUTING.md` with complete branching taxonomy, Conventional Commits, local test commands, and performance gates in `CONTRIBUTING.md`
- [x] T014 [P] [US3] Update `SECURITY.md` and `CODE_OF_CONDUCT.md` with standard disclosure policies and community standards

**Checkpoint**: User Story 3 complete — governance documentation and contribution rules synchronized.

---

## Phase 6: User Story 4 - CI/CD Quota Protection & Offline Local Pre-Flight Verification Gate (Priority: P2)

**Goal**: Ensure GitHub Actions automated triggers remain strictly disabled (0 runner minutes consumed), and provide a fast, single-command offline local verification script.

**Independent Test**: Confirm `.github/workflows/ci-cd.yml` trigger configuration and execute `./scripts/pre_flight_check.sh`.

- [x] T015 [US4] Verify and harden `.github/workflows/ci-cd.yml` to ensure automatic push/PR triggers are disabled and only `workflow_dispatch` is active
- [x] T016 [US4] Create local pre-flight quality verification script in `scripts/pre_flight_check.sh`
- [x] T017 [US4] Make `scripts/pre_flight_check.sh` executable via `chmod +x`

**Checkpoint**: User Story 4 complete — CI quota protected and local pre-flight automation active.


---

## Phase 7: Polish & End-to-End Validation

**Purpose**: Execute all verification scenarios from `quickstart.md` and assert full repository quality gates pass.

- [x] T018 Execute validation scenarios from `specs/077-repository-hygiene-and-standards/quickstart.md`
- [x] T019 Run local pre-flight gate script `./scripts/pre_flight_check.sh` to assert full repository hygiene, linting, tests, and performance gates


---

## Dependencies & Execution Order

- **Phase 1 (Setup) & Phase 2 (Foundational)**: Must complete first.
- **Phase 3 (US1) & Phase 4 (US2)**: Core P1 priorities; can execute in parallel.
- **Phase 5 (US3) & Phase 6 (US4)**: P2 governance and automation; can execute in parallel.
- **Phase 7 (Polish & Validation)**: Depends on all implementation tasks (T001–T017) being complete.
