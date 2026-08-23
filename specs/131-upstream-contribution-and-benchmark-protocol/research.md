# Technical Research & Architecture Decisions: Feature 131

## R001: Compiler Flag Asymmetry Elimination & Pre-Flight CMake Audit
- **Decision**: Implement an automated pre-flight flag validator that parses `CMakeCache.txt` and compile commands from both baseline and candidate builds, asserting 100% equivalence on `-DCMAKE_BUILD_TYPE`, `-DWITH_NATIVE_INSTRUCTIONS`, `-DWITH_ARMV8CRC32_HW`, and optimization levels before launching benchmarks.
- **Rationale**: PR #2416 initially showed skewed results because the baseline build defaulted to generic scalar while the candidate enabled native hardware instructions. Enforcing pre-flight cache assertions makes flag divergence impossible.
- **Alternatives Considered**: Manual checklist review (rejected: error-prone and vulnerable to developer oversight).
- **Source**: `Vendor/worktrees/zlib-ng/feat-arm64-swar-compare256/CMakeLists.txt`, CMake Cache Specification.

---

## R002: Dual Cross-Over In-Memory Benchmarking Engine (DVFS / Thermal Drift Immunity)
- **Decision**: Adopt a mirrored Latin Square / Cross-Over A/B & B/A execution harness (Order 1: Candidate first -> Baseline second; Order 2: Baseline first -> Candidate second) with 5-repetition statistical aggregation in 100% in-memory RAM mode.
- **Rationale**: Apple Silicon and modern high-IPC CPUs exhibit 1%~2% clock modulation due to thermal hysteresis during multi-minute single-core saturation. Whichever binary runs first receives cold-silicon boost. Mirrored cross-over execution mathematically cancels the thermal gradient.
- **Alternatives Considered**: Fixed cooldown sleep intervals (rejected: does not fully equalize thermal state across dynamic background workloads).
- **Source**: Google Benchmark statistical aggregation docs, LLVM Benchmarking Best Practices.

---

## R003: Zero-Hallucination AST/Template Report Generation
- **Decision**: Build an automated reporting generator that dynamically extracts all numeric metrics, minimums, maximums, and means directly from raw benchmark JSON into parameterized markdown templates, strictly banning literal numeric constants in template text.
- **Rationale**: In PR #2416, a hardcoded string placeholder in the summary paragraph differed slightly from the dynamically computed table cell (+6.8% vs +6.2%). Pure variable interpolation enforces the Single Source of Truth principle.
- **Alternatives Considered**: Regex post-processing of hand-written markdown (rejected: fragile and prone to miss newly added paragraphs).
- **Source**: `scratch/generate_crossover_tables.py`, Python JSON and Jinja2/f-string interpolation.

---

## R004: Pair-Programming Sovereignty & Remote Mutation Permission Gate
- **Decision**: Impose a strict architectural boundary where the AI agent is physically prohibited from executing any remote write API calls (GitHub comments, PR updates, issue edits, package uploads) until local files in `scratch/` are finalized and explicit user authorization is recorded in the dialogue.
- **Rationale**: Preserves developer agency, prevents premature commentary from reaching upstream maintainers, and allows thorough human-in-the-loop review.
- **Alternatives Considered**: Automatic optimistic posting with rollback (rejected: remote comments cannot be easily erased without trace on open source repositories).
- **Source**: GitHub REST API guidelines, Upstream Contribution Golden Protocol.

---

## R005: Inline Assembly Encapsulation, DRY Macro Architecture & Heritage Preservation
- **Decision**: Establish an immutable pre-submission code craftsmanship standard: all AArch64/x86 inline assembly MUST be extracted into scoped file-level helper macros (e.g. `LOAD_16B_PAIR`), original maintainer comments regarding microarchitecture quirks must be strictly preserved above the macro definition, all fallback branches must have `Z_UNUSED` parameter protection, all helper macros must have matching `#undef` guards at the bottom of the compilation unit, and the refactor must be split into a bit-exact Commit 1 prior to any algorithmic changes in Commit 2.
- **Rationale**: In the initial submission of PR #2416, scattered raw `__asm__` blocks inside the loop body drew immediate maintainer review comments regarding code duplication and comment displacement. Resolving this via clean helper macro extraction in Commit 1 earned Nathan's explicit approval: *"I have no other comments on code quality."*
- **Alternatives Considered**: Inlining assembly directly inside functions (rejected: violates DRY, increases maintenance burden, and creates code smells in upstream reviews).
- **Source**: zlib-ng PR #2416 review thread, Linux Kernel inline assembly guidelines.
