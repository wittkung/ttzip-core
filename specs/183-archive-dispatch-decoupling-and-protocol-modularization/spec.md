# Feature Specification: 183-archive-dispatch-decoupling-and-protocol-modularization

## 1. Executive Summary & Strategic Motivation
This feature addresses the remaining oversized files ($> 350\text{ LOC}$) identified in the eighth-round audit:
1. **Archive Engine Dispatch Decoupling (`ArchiveEngineBridge.swift` & `ArchiveWriter+Dispatch.swift`)**:
   - Refactor monolithic dispatch tables into high-cohesion, format-specific delegate routes.
2. **Strategy & Component Protocol Modularization (`CompressionStrategyProtocol.swift`, `ArchiveComponentProtocol.swift`, `ArchiveProtocols.swift`)**:
   - Modularize large multi-protocol aggregations into focused SRP interfaces.
3. **Test Terminal Renderer & CLI Commands SRP Split (`TestTerminalRenderer.swift`, `CompressCommand.swift`)**:
   - Decompose terminal ANSI formatting from test execution rendering, and separate compress command argument validation from execution.

---

## 2. User Scenarios & Acceptance Criteria

### User Scenario 1: Clean Architectural Boundaries
- **Given** developers and automated tools inspecting codebase files
- **When** checking file lengths across all first-party Swift files in `TTZipCore` and `TTZipBench`
- **Then** 100% of files strictly obey $< 350\text{ LOC}$ with zero circular dependencies.

---

## 3. Success Metrics
1. **SRP & LOC Budget**: 100% of files in `TTZipCore` and `TTZipBench` $< 350\text{ LOC}$.
2. **Zero Regression**: 100% pass rate across 175+ Rust tests, 893+ Swift tests, and 7/7 local CI stages.
