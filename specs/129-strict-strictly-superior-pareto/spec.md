# Feature Specification: Strict Both-Faster-and-Smaller Pareto Dominance

**Feature ID**: `129-strict-strictly-superior-pareto`  
**Status**: Ready for Planning  
**Branch**: `129-strict-strictly-superior-pareto`  
**Primary Stakeholder**: CTO & Performance Engineering  

---

## 1. Problem Statement & Motivation

The user has explicitly defined the strict criterion for Pareto superiority:
For **every single competitor test point** $p = (S_{\text{lib}}, \text{Size}_{\text{lib}})$ across all benchmark levels (Levels 1 to 12) on all 4 standard 100MB corpora:
TTZip MUST have at least one test point $q = (S_{\text{ttzip}}, \text{Size}_{\text{ttzip}})$ that is **STRICTLY BOTH FASTER AND SMALLER**:
$$\forall p \in \text{libdeflate}, \exists q \in \text{TTZip}: S(q) > S(p) \land \text{Size}(q) < \text{Size}(p)$$

No ties. No points merely identical. Every point of competitor $p$ must be strictly surpassed on both axes simultaneously.

---

## 2. Mathematical Definition & Success Criteria

### SC-001: Strict Dual-Axis Superiority
For every evaluated libdeflate point $p_i = (S_{\text{lib}, i}, \text{Size}_{\text{lib}, i})$:
$$\text{SpeedAdvantage}(p_i) = \frac{S(q_j)}{S_{\text{lib}, i}} > 1.00$$
$$\text{SizeAdvantage}(p_i) = \frac{\text{Size}_{\text{lib}, i}}{\text{Size}(q_j)} > 1.00$$

### SC-002: 100% Corpus Coverage
- **Corpus 1**: `Structured Logs & JSON 100MB` (10/10 points strictly surpassed)
- **Corpus 2**: `Binary Mach-O / ARM64 100MB` (10/10 points strictly surpassed)
- **Corpus 3**: `Mixed Modality 100MB Real-World Workspace` (10/10 points strictly surpassed)
- **Corpus 4**: `Text & Web: enwik8 100MB` (10/10 points strictly surpassed)
- **Total**: **40 / 40 points strictly surpassed simultaneously**.

### SC-003: 100% Decompression Bit-Exactness
All emitted Deflate payloads must pass standard `/usr/bin/unzip -t` and `/usr/bin/unzip -p` verification with 0 byte differences.

### SC-004: Zero CI/CD Bypass
All 1,138 unit tests, 13 hard performance gates, and 6-stage pre-push CI hooks must pass cleanly. All commits pushed to GitHub `main`.
