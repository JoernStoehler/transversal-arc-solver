# Ablation Study Results (Final — SVD bug fixed)

**Note:** An initial version had a critical SVD bug (`thin_svd` returning 6×4 instead of 6×6 V matrix) that prevented transversal computation. All results below use the corrected full SVD.

## 30-Task Comparison (seed=1000, 120s timeout)

| Method | Solved/30 | Solve Rate |
|--------|-----------|------------|
| M6 (counting) | 15 | 50.0% |
| **M0-rust** | **14** | **46.7%** |
| **M1** | **14** | **46.7%** |
| M7 (per-cell) | 4 | 13.3% |

## 12-Task Exhaustive Comparison (seed=1000)

| Method | Solved/12 |
|--------|-----------|
| M0 (C binary) | 3 |
| M0-rust | 2 |
| M1 | 2 |
| M6 | 2 |
| M7 | 2 |

## Seed Stability (12 exhaustive tasks, seeds 1000-1004)

```
method  seed  n_tasks  n_solved  solve_rate
m0rust  1000  12       2         0.1667
m0rust  1001  12       2         0.1667
m0rust  1002  12       2         0.1667
m0rust  1003  12       2         0.1667
m0rust  1004  12       2         0.1667
m1      1000  12       2         0.1667
m1      1001  12       2         0.1667
m1      1002  12       2         0.1667
m1      1003  12       2         0.1667
m1      1004  12       2         0.1667
```

## Key Findings

1. **M0-rust ≡ M1:** Identical results across all tasks and seeds. The Plücker quadratic is unnecessary.
2. **M6 ≥ M0-rust:** Pure frequency counting matches or beats the full geometric pipeline (without histogram scoring).
3. **C binary advantage:** The C binary's 3/12 vs M0-rust's 2/12 comes from histogram-aware scoring, not geometry.
4. **Adjacency helps:** All adjacency-based methods (M0-rust, M1, M6) significantly beat M7 (per-cell mode).

Generated: 2026-03-25
