# What Actually Drives the Transversal ARC Solver?

An independent ablation study of the [transversal-arc-solver](https://github.com/khalildh/transversal-arc-solver). The upstream README claims 316 ARC-AGI tasks solved at rank 1 using Plücker geometry on Gr(2,4). We systematically removed components of the pipeline to find out which ones actually matter.

**Short version:** The Plücker quadratic — the geometric heart of the method — is unnecessary. Replacing it with a random vector produces identical results. But without the upstream code's *histogram-aware scoring* optimization (unrelated to geometry), the full Plücker pipeline does no better than counting color-pair frequencies. The geometry appears to be decorative; the histogram trick appears to be load-bearing.

---

## 1. Background

### 1.1 ARC-AGI

An ARC task gives you 2–10 training pairs (input grid → output grid) and a test input. You produce the test output. Grids are small (3×3 to 30×30), up to 10 colors. A 3×3 grid with 8 colors has 8⁹ ≈ 134 million possible outputs.

### 1.2 The Upstream Pipeline

The upstream solver works in 6 steps:

1. **Embed** each grid cell into R^d using 8 embedding functions (dimensions 20–44). Each captures different features: color identity, position, row/column statistics, etc.
2. **Project** each adjacent cell pair via two fixed random matrices W1, W2 ∈ R^{4×2d} to get two points in R⁴.
3. **Exterior product** of those two R⁴ points → a 6-dimensional Plücker vector representing a line in projective 3-space.
4. **Find transversals**: pick 4 random training lines, build a 4×6 constraint matrix, take its SVD to get a 2D null space, then solve a quadratic equation (the Plücker relation) to find lines in P³ that meet all 4. This yields up to 2 transversal lines per 4-tuple. Repeat ~200 times per training pair.
5. **Score** each candidate output by computing Σ log|⟨candidate_line, J₆ · transversal⟩| over all transversals, where J₆ is the 6×6 Hodge-star matrix. Lower score = better (the log of a near-zero inner product is very negative, indicating the candidate line nearly meets the transversal).
6. **Decompose** the score into a sum over adjacency pairs: score_table[adj_pair][color_a][color_b]. This precomputation turns candidate scoring into table lookups — O(adjacency_pairs) per candidate instead of O(adjacency_pairs × transversals).

### 1.3 Histogram-Aware Scoring

This is the optimization the upstream README barely mentions but which our experiments suggest matters most.

One of the 8 embeddings is `hist_color` (30 dimensions). Its last 10 components are the *histogram difference*: for each color c, it stores (count of c in output − count of c in input) / grid_size. At test time, we don't know the output, so these 10 values depend on the *candidate* output's color distribution.

**The naive approach** (which our Rust reimplementation uses): pretend the histogram difference is zero — use the test input as a stand-in for the output. This makes the `hist_color` embedding independent of the candidate, so we can precompute one score table for it.

**The histogram-aware approach** (which the C binary uses when feasible): enumerate all possible output color histograms. For a 3×3 grid with 3 colors, there are C(11,2) = 55 possible histograms. For each histogram, compute the correct histogram-difference vector, build a separate `hist_color` score table, and score candidates against the table that matches their actual histogram. This means `hist_color` scores depend on the candidate's global color distribution, not just its local cell colors.

The C binary uses histogram-aware scoring when the number of possible histograms is ≤ 2000 (roughly: small grids with few colors). Otherwise it falls back to the naive approach.

**Why this matters:** The histogram difference captures a global signal — "this candidate has too many red cells compared to what the training outputs had." Without it, scoring is purely local (each adjacency pair scored independently). With it, there is a global consistency check. Our experiments suggest this global signal, not the Plücker geometry, is what separates the upstream solver from trivial baselines.

### 1.4 Research Questions

- **Does the Plücker quadratic matter?** (Hypothesis A: yes, it provides geometric inductive bias. Hypothesis B: no, it's replaceable.)
- **What component of the pipeline actually drives the solve rate above trivial baselines?**

## 2. Mathematical Framework

### 2.1 Plücker Coordinates

Given two points a, b ∈ R⁴, the **Plücker coordinates** of the line through them are 6 numbers:

    p_{ij} = a_i · b_j − a_j · b_i    for (i,j) ∈ {(0,1), (0,2), (0,3), (1,2), (1,3), (2,3)}

Not every 6-vector is a valid line. The **Plücker relation** must hold:

    p_{01} · p_{23} − p_{02} · p_{13} + p_{03} · p_{12} = 0

The set of valid Plücker vectors (up to scale) is the Grassmannian Gr(2,4) — a 4-dimensional variety in P⁵.

### 2.2 Incidence

Two lines p, q in P³ intersect iff pᵀ J₆ q = 0, where J₆ is the 6×6 matrix:

```
J₆ = | 0  0  0  0  0  1 |
     | 0  0  0  0 -1  0 |
     | 0  0  0  1  0  0 |
     | 0  0  1  0  0  0 |
     | 0 -1  0  0  0  0 |
     | 1  0  0  0  0  0 |
```

### 2.3 Transversal Computation

Given 4 lines L₁,...,L₄, form the 4×6 matrix A whose rows are J₆·Lᵢ. The SVD of A gives a 2D null space spanned by v₁, v₂. Any vector T = t·v₁ + v₂ satisfies the 4 incidence constraints (Aᵀ T = 0). Imposing the Plücker relation on T gives a quadratic in t:

    α·t² + β·t + γ = 0

with α = Q(v₁), γ = Q(v₂), β a bilinear cross term. This has 0 or 2 real solutions.

**Our ablation M1:** skip the quadratic. Pick t uniformly at random in [−10, 10]. The resulting T satisfies the incidence constraints but generally does NOT lie on the Grassmannian (i.e., it is not a valid Plücker line).

## 3. Ablation Study

### 3.1 Methods

| Method | What it removes | What it tests |
|--------|----------------|---------------|
| M0 | Nothing (upstream C binary, with histogram scoring) | Reference |
| M0-rust | Nothing (Rust reimpl, no histogram scoring, different RNG seeds) | Implementation check |
| **M1** | **Plücker quadratic** (random null-space vector) | **Does Gr(2,4) matter?** |
| M2 | Transversal extraction entirely (score vs training lines) | Does 4-line sampling help? |
| M3 | J₆ + exterior product (plain dot product) | Does Plücker structure matter? |
| M4 | Random projection (cosine similarity on raw embeddings) | Is projection needed? |
| M5 | Elaborate embeddings (40-dim one-hot only) | Do embedding features matter? |
| M6 | All linear algebra (color-pair frequency counting) | Is any of this needed? |
| M7 | Adjacency structure (per-cell mode prediction) | Floor baseline |

**Key difference between M0 and M0-rust:** The C binary uses histogram-aware scoring for `hist_color` when feasible. Our Rust implementation always uses the naive fallback (histogram difference = 0). This is the only known functional difference beyond RNG seeds.

### 3.2 Evaluation

- **262 same-size ARC training tasks** (input dims = output dims, ≥2 training pairs)
- We tested subsets: 30 "fast" tasks (smallest grids) and 12 "exhaustive" tasks (where nc^(hw) ≤ 200M, so every candidate can be scored)
- **nc** = number of distinct colors in a task; **hw** = grid height × width
- Rank of correct answer among all candidates; rank 1 = solved
- No test output leakage: correct answer used only to compute rank after all scoring

### 3.3 Seed Stability

M0-rust and M1 tested with 5 seeds (1000–1004) on the 12 exhaustive tasks. M6 and M7 are deterministic.

## 4. Results

### 4.1 Comparison on 30 Fast Tasks (seed 1000)

| Method | Solved | Rate | Notes |
|--------|--------|------|-------|
| M6 | 15/30 | 50% | Pure counting, no linear algebra |
| M0-rust | 14/30 | 47% | Full Plücker pipeline, no histogram scoring |
| M1 | 14/30 | 47% | Same as M0-rust but no Plücker quadratic |
| M7 | 4/30 | 13% | Per-cell mode (no adjacency) |

M0-rust and M1 solve the exact same 14 tasks. M6 (trivial counting) solves 15, one more than the full geometric pipeline.

### 4.2 Comparison on 12 Exhaustive Tasks (seed 1000)

| Method | Solved/12 |
|--------|-----------|
| M0 (C binary, with histogram scoring) | **3** |
| M0-rust (no histogram scoring) | 2 |
| M1 (no Plücker quadratic) | 2 |
| M6 (counting) | 2 |
| M7 (per-cell) | 2 |

The C binary's extra solve comes from histogram-aware scoring. Without it, M0-rust, M1, M6, and M7 all solve the same 2 tasks.

### 4.3 Seed Stability (12 exhaustive tasks, seeds 1000–1004)

| Method | Solved per Seed | Std |
|--------|----------------|-----|
| M0-rust | 2/12 on all 5 seeds | 0.0 |
| M1 | 2/12 on all 5 seeds | 0.0 |

Identical. Zero variance between M0-rust and M1 across seeds.

### 4.4 Upstream Reproduction

We ran the unmodified C binary on the 12 exhaustive tasks. It solves 3/12, using histogram-aware scoring on the tasks with ≤2000 histograms (e.g., "Building 55 histogram tables" for task 794b24be). On tasks where it falls back to the naive approach, it performs no better than our Rust code.

We could not reproduce the full 166/262 claimed result within our compute budget (the single-threaded C binary takes 2+ hours on all 262 tasks without OpenMP).

## 5. Discussion

### 5.1 The Plücker Quadratic Doesn't Matter

M1 (random null-space vector, no Plücker relation enforcement) matches M0-rust (full quadratic solve) on every task, every seed. The Grassmannian constraint adds nothing. This is consistent with the upstream author's own findings in their [transversal-memory](https://github.com/khalildh/transversal-memory) repository, where they concluded the geometry was decorative for word-association tasks.

### 5.2 The Full Pipeline Doesn't Beat Counting

On our 30-task subset, pure color-pair frequency counting (M6: 15/30) slightly outperforms the full Plücker pipeline without histogram scoring (M0-rust: 14/30). On the 12 exhaustive tasks, they tie at 2/12. The transversal machinery — embeddings, random projection, SVD, exterior products, J₆ scoring — provides no net advantage over a method that simply counts how often each (input_a, output_a, input_b, output_b) 4-tuple appeared in training.

Both M0-rust and M6 substantially beat M7 (per-cell mode: 4/30), confirming that adjacency-based scoring matters. But among adjacency-based methods, the geometric apparatus adds nothing over counting.

### 5.3 Histogram Scoring Appears to Be the Key

The C binary's extra solve (3/12 vs 2/12) comes from histogram-aware scoring, not from Plücker geometry. This optimization:

- Enumerates all possible output histograms (color distributions)
- For each histogram, computes the correct `hist_color` embedding (with the real histogram-difference vector, not zeros)
- Builds a separate score table per histogram
- Scores each candidate against the table matching its actual histogram

This injects a **global signal** into what is otherwise a purely local scoring scheme. Each adjacency pair is scored independently, but histogram-aware scoring ensures the `hist_color` component reflects the candidate's global color distribution. When the upstream README says "316 tasks solved," this optimization — not Plücker geometry — is likely doing much of the heavy lifting.

We did not implement histogram-aware scoring in our Rust code (it adds ~200 lines of complexity for the histogram enumeration and per-histogram table building). Doing so would be the most impactful next step.

### 5.4 Limitations

- We tested 30 of 262 same-size tasks (smallest grids). Results may differ on larger grids.
- We did not implement histogram-aware scoring, so we cannot directly measure its contribution vs. the geometric components in isolation.
- M0-rust uses different RNG seeds (ChaCha8) than the C binary (MT19937), so the M0 vs M0-rust comparison confounds seed differences with the histogram scoring difference.
- We did not test M2–M5 on the corrected code (the initial results for those methods were invalidated by the SVD bug described in Section 5.5).
- The 12-task exhaustive subset has a floor effect: even the trivial M7 solves 2/12, leaving little room to differentiate methods.

### 5.5 Note on the SVD Bug

An early version of our code used `thin_svd()` (which returns only 4 of 6 right singular vectors for a 4×6 matrix) instead of `svd()` (which returns all 6). This silently produced zero transversals, making all score tables empty and all candidates tied at score 0. Every method appeared to achieve 100% solve rate — a vacuous result. The bug was caught by a code-review subagent (V3) and fixed. All results in this document use the corrected full SVD.

## 6. Conclusion

The Plücker quadratic is unnecessary: M1 (random null-space vector) equals M0-rust (quadratic solve) on all tested tasks and seeds. The full pipeline without histogram scoring does not beat frequency counting (M6). The upstream solver's real advantage over baselines appears to come from histogram-aware scoring — a non-geometric optimization that gives the `hist_color` embedding access to each candidate's global color distribution. The Grassmannian Gr(2,4), the Plücker relation, and Schubert calculus are mathematically elegant but do not contribute measurably to task-solving performance.

---

## Appendix A: Embedding Functions

| # | Name | Dim | Features |
|---|------|-----|----------|
| 0 | hist_color | 30 | Input color one-hot (10) + output color one-hot (10) + histogram difference (10) |
| 1 | color_only | 20 | Input/output color one-hots |
| 2 | pos_color | 22 | Normalized row/col position + color one-hots |
| 3 | all | 42 | Position + colors + full input/output color histograms |
| 4 | row_feat | 44 | Colors + row color distribution + row uniformity indicators |
| 5 | col_feat | 42 | Colors + column color distribution + column uniformity |
| 6 | color_count | 24 | Colors + color frequency + mode indicators |
| 7 | diagonal | 26 | Colors + diagonal/antidiagonal position features |

The `hist_color` embedding (index 0) is the only one affected by the histogram-scoring optimization, because it is the only one whose features depend on the *global* output color distribution.

## Appendix B: Method Definitions

**M0 (C binary):** Upstream `arc_solver.c`, unmodified. Uses MT19937 RNG with hardcoded per-embedding seeds. Uses histogram-aware scoring for `hist_color` when ≤2000 histograms are possible.

**M0-rust:** Full pipeline in Rust with ChaCha8 RNG. Does NOT implement histogram-aware scoring; always uses the fallback (histogram difference = 0).

**M1:** Same as M0-rust, but skips the Plücker quadratic. Instead of solving α·t² + β·t + γ = 0 for t, picks t ~ Uniform(−10, 10).

**M2–M5:** Progressively simpler methods removing transversal extraction, J₆ structure, random projection, and embedding complexity. (Results from initial run only; not re-tested after SVD bug fix.)

**M6:** For each adjacency pair, count how often each (input_a, output_a, input_b, output_b) 4-tuple appears in training. Score: −Σ log(count + ε). No linear algebra.

**M7:** For each cell, predict the most common output color for its input color across training pairs.

## Appendix C: Reproduction

```bash
# Prerequisites
sudo apt-get install -y liblapack-dev libblas-dev
git clone https://github.com/fchollet/ARC-AGI data/ARC-AGI

# Compile upstream C solver (source not modified)
cc -O3 -march=native -D'__CLPK_integer=int' -o arc_solver arc_solver.c -lm -llapack -lblas

# Build Rust ablation suite
cd ablations && cargo build --release && cd ..

# Run comparison
./ablations/target/release/ablations --method m0rust,m1,m6,m7 --seeds 1000 \
    --all data/ARC-AGI/data/training/ --timeout 120

# Run upstream C binary on a single task
./arc_solver data/ARC-AGI/data/training/0d3d703e.json
```

---

*Independent analysis of [khalildh/transversal-arc-solver](https://github.com/khalildh/transversal-arc-solver). The upstream `arc_solver.c` was not modified. Ablation code is in `ablations/`.*
