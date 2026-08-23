# Data Model: 096-full-codebase-standards-and-defensive-hardening

## Defensive Type Models & Invariants

### 1. `TTZipDefensiveHandle` (Opaque Struct Pattern)
```c
typedef struct {
    uint32_t magic;           /**< TTZIP_STRUCT_MAGIC (0x545A4950U) when active; TTZIP_POISON_FREE (0xDEADBEEFU) when freed */
    uint32_t flags;           /**< State flags (bit 0: INITIALIZED, bit 1: BUSY, bit 2: ERROR) */
    size_t capacity_bytes;    /**< Bounded buffer capacity */
    void* internal_state;     /**< Opaque internal engine state */
} ttzip_defensive_handle_t;
```

### 2. `TTZipDocContract` (DocC / Doxygen Specification Matrix)
```markdown
- @brief       1-sentence concise description
- @param[in]  Input pointer/value with range constraints
- @param[out] Output pointer with minimum capacity constraints
- @return      Meaning of return value and error sentinels
- @pre         Preconditions required to avoid UB
- @post        Guaranteed state upon normal return
- @invariant   Loop or class invariants maintained
- @complexity  Time and space asymptotic bounds
- @threadsafe  Concurrency guarantees (Reentrant, Thread-Safe, Actor-Isolated)
```
