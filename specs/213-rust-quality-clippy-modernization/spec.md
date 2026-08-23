# Spec: Rust Code Quality & Modernization Audit (rust-skills Compliance)

**Feature**: `213-rust-quality-clippy-modernization`  
**Classification**: `[Lean SDD]` (Internal Rust code quality, performance, idioms, and zero-warning clippy compliance)  
**Status**: `IN_PROGRESS`  

---

## 1. Objectives & Guidelines (rust-skills)

This feature systematically audits and modernizes the entire Rust codebase (`ttzip-glue` and `ttzip-tui`) based on the 265 rules in `rust-skills`:

1. **Unsafe & FFI Safety (`unsafe-safety-comment`, `unsafe-minimize-scope`)**:
   - Modernize all C-string literals in C-ABI exports to modern Rust `c"..."` literals.
   - Ensure every `unsafe` block in FFI, pointer dereferencing, and NEON/PMULL SIMD contains an explicit `// SAFETY:` rationale comment.

2. **Memory Optimization & Zero Allocations (`mem-arrayvec`, `mem-zero-copy`, `own-borrow-over-clone`)**:
   - Replace redundant `vec![0u8; N]` heap allocations in sniffer and header parsing with stack arrays `[0u8; N]`.
   - Eliminate unnecessary `.clone()` calls across slice constructors (`std::slice::from_ref`).
   - Direct struct initialization with `Default::default()` base instead of separate reassignments.

3. **Idiomatic Testing & Code Health (`test-`, `lint-clippy-deny`)**:
   - Resolve all `clippy::module_inception`, `clippy::bool_assert_comparison`, `clippy::identity_op`, and `clippy::needless_range_loop` warnings.
   - Clean up test module hierarchies.

4. **Zero-Warning Gate**:
   - Achieve `cargo clippy --all-targets --all-features -- -D warnings` with 0 warnings.
   - Maintain 100% pass rate across unit, property, fuzz, and differential test suites.
