# Data Model: In-Process Parallel Zopfli/Advzip Engine

**Feature**: `specs/101-inprocess-parallel-zopfli-advzip-engine`

## 1. Zopfli Block Configuration (`ZopfliBlockConfig`)
- `numIterations`: Int (1 .. 100, Level 6 = 5, Level 7 = 15)
- `blockSplitting`: Bool (true for Level 7)
- `maxBlockSplits`: Int (0 .. 15)
- `historyWindowBytes`: Int (32768)
- `earlyExitThreshold`: Double (0.0001)

## 2. Pigz Benchmark Matrix Spec (`PigzMatrixSpec`)
- `level`: Int (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11)
- `threads`: Int (18)
- `isStoreMode`: Bool (true for level 0)
- `isZopfliMode`: Bool (true for level 11)
