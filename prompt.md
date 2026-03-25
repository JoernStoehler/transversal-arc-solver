# CLAUDE.md — Independent Investigation of the Transversal ARC Solver

## Mission

This repo is a fork of https://github.com/khalildh/transversal-arc-solver. It already contains `arc_solver.c` (~1900 lines) which implements an ARC-AGI solver using Plücker geometry on Gr(2,4). The upstream README.md claims 316 tasks solved with "zero learning."

Our goal: determine **what actually drives the performance** by building a ladder of progressively simpler methods and comparing them rigorously. The final deliverable is a new `README.md` replacing the upstream one with our findings.

## What's Already in This Repo

- `arc_solver.c` — the upstream solver. Do not modify this file. It is our reference implementation (Method M0).
- `README.md` — the upstream claims. We will replace this at the end with our writeup.

## Context & Hypotheses

The upstream method:
1. Embeds each grid cell into R^d (8 embedding types, d ∈ {20..44})
2. For each adjacent cell pair, concatenates embeddings → R^{2d}
3. Projects via two fixed random matrices W1, W2 ∈ R^{4×2d} → two points in R^4
4. Takes the exterior product (Plücker coordinates) → a line in P^3 (R^6)
5. From training pairs, samples 4 random lines, finds transversals via SVD null space + quadratic solve
6. Scores test candidates by sum of log|⟨candidate_line, J6 · transversal⟩| over all transversals
7. Decomposes score by adjacency pair for O(1)-per-candidate table lookup

**Hypothesis A (geometry matters):** The Plücker quadric constraint and transversal structure provide genuine geometric inductive bias beyond what simpler methods achieve.

**Hypothesis B (geometry is decorative):** The method works because (a) the embeddings capture useful features, (b) the random projection + degree-2 kernel creates a reasonable similarity measure, and (c) the log-sum scoring aggregates many weak signals. The Plücker relation, Grassmannian, and Schubert calculus are mathematically beautiful but functionally replaceable. The upstream author's own ablation studies in their `transversal-memory` repo explicitly conclude this for discriminative tasks.

**Hypothesis C (it's mostly the embeddings):** Even simpler scoring works comparably, and the random projection is also unnecessary.

## Step 0: Compile and Run the Upstream Code

Before writing any new code:

1. Clone ARC data: `git clone https://github.com/fchollet/ARC-AGI data/ARC-AGI`
2. Compile `arc_solver.c`:
   - Try: `cc -O3 -march=native -o arc_solver arc_solver.c -lm -llapack` (Linux)
   - If LAPACK headers are missing: `sudo apt-get install -y liblapack-dev libblas-dev`
   - If that fails, check compiler errors and fix build flags. Do NOT modify the C source.
3. Run smoke test: `./arc_solver data/ARC-AGI/data/training/0d3d703e.json`
4. Run on all training tasks: `./arc_solver --all data/ARC-AGI/data/training/`
5. Save the output verbatim to `results/upstream_raw.txt`

Parse the output to count: tasks attempted, tasks solved (rank 1), tasks timed out, and produce `results/upstream_summary.tsv`. This is our ground truth for M0.

**If the upstream code does not compile or does not reproduce the claimed results, that is itself a finding. Document it.**

## Language & Architecture for Ablations

Use **Rust** with a single crate in `ablations/`. Reasons:
- Need to score up to ~10^8 candidates per task; Rust release builds give C-like speed
- Cargo gives us tests, dependencies, and parallelism (rayon) trivially
- `faer` for pure-Rust SVD (no LAPACK dependency headaches), or `nalgebra` with `nalgebra-lapack`
- `serde` + `serde_json` for ARC task parsing (replacing 300 lines of hand-rolled JSON parser)

### Crate structure
```
ablations/
  Cargo.toml
  src/
    main.rs          — CLI entry point, orchestration
    task.rs          — ARC task parsing, same-size filtering
    embeddings.rs    — All 8 embedding functions (reproduce upstream logic)
    plucker.rs       — Plücker coordinates, Hodge dual, incidence, transversal solve
    scoring.rs       — Score table building, candidate enumeration/sampling
    methods.rs       — The different method variants (M1–M6, plus M0-rust)
    stats.rs         — Statistical analysis utilities
    rng.rs           — Deterministic seeded RNG (NOT matching upstream seeds — our own)
    lib.rs           — Re-exports for testing
  tests/
    plucker_tests.rs
    embedding_tests.rs
    smoke_tests.rs
```

Methods are a trait or enum so a single CLI invocation runs all variants and produces a comparison table.

## Ablation Ladder (Methods to Implement)

Implement these **in order**. Each is a self-contained solver.

### M0: Upstream C code (already exists)
Just compile and run `arc_solver.c` as-is. This is the reference. We do NOT reimplement it in Rust for comparison purposes — the C binary is the ground truth for what the upstream method actually achieves.

### M0-rust: Rust reimplementation of the full method
Reimplement the upstream pipeline in Rust with our own RNG seeds. This will NOT match M0's numbers exactly (different random projections). The purpose is: (a) validate we understand the method by checking it scores in the same ballpark as M0, (b) provide a common Rust codebase that M1–M5 can be derived from by deleting/replacing components.

### M1: No Plücker constraint (random null-space vector)
Same as M0-rust but in the transversal-finding step: after computing the 2D null space {v1, v2} from the SVD, pick a random unit vector t·v1 + v2 (uniform random t) instead of solving the Plücker quadratic. This tests whether enforcing Q(T) = 0 matters.

### M2: Random 6-vectors instead of transversals
Same embeddings + random projection as M0-rust, but skip transversal extraction entirely. Instead: from training pairs, collect all Plücker lines as "reference vectors." Score each candidate line as sum of log|⟨candidate, J6 · reference⟩| over all training reference lines. This tests whether the transversal extraction step adds value over directly comparing to training lines.

### M3: Random projection + dot product (no Plücker structure)
Same embeddings, project via a single random matrix W ∈ R^{6×2d} → R^6. From training, collect these 6-vectors. Score candidate 6-vectors by sum of log|dot(candidate, reference)| (plain dot product, no J6). Collect "pseudo-transversals" as random vectors in the null space of 4 random training vectors (plain SVD, no quadratic). This tests whether J6 and the exterior product structure matter.

### M4: Cosine similarity on raw embeddings (no projection)
Same embeddings, no random projection. For each adjacency pair, concatenate two cell embeddings → R^{2d}. From training, collect these. Score candidate by sum of cosine similarities to training vectors. This tests whether the random projection and all geometry are necessary.

### M5: One-hot pairwise scoring (minimal features)
For each adjacency pair, represent state as (input_color_a, output_color_a, input_color_b, output_color_b) — a 40-dimensional one-hot vector. Score by cosine similarity to training vectors. Tests whether the elaborate embedding functions matter.

### M6: Color transition frequency (trivial baseline)
Count how often each (in_a, out_a, in_b, out_b) 4-tuple appears in training adjacency pairs. Score candidate by sum of log(count + epsilon). Pure frequency counting, no linear algebra.

### M7: Per-cell independent mode (degenerate baseline)
For each cell, find the most common output color for its input color across training pairs. Pick the mode. No adjacency structure. Establishes the floor.

## Seed Stability Protocol

**Critical. Do not skip this.**

The upstream results may be inflated by seed selection. For every stochastic method (M0-rust, M1–M6; M7 is deterministic):
1. Run with **20 different random seeds** (seeds 1000, 1001, ..., 1019)
2. Report: mean solve rate, standard deviation, min, max
3. A method's "score" is its **mean across seeds**, not its best seed
4. Report the **coefficient of variation** (CV = std/mean) — high CV means unreliable

For M0 (the C binary): it has a single built-in seed. Run it once. Note in the writeup that we cannot test its seed stability without modifying the source (which we don't do). If M0-rust shows high CV, that tells us the upstream result is seed-dependent.

## ARC Data & Evaluation Protocol

- Clone: `git clone https://github.com/fchollet/ARC-AGI data/ARC-AGI`
- Filter to **same-size tasks only** (input dims = output dims for all pairs in a task). Record count.
- **Timeout: 30 seconds per task**, hard cutoff. Task counts as unsolved (rank = ∞).
- **Denominator is always ALL same-size tasks.** Never exclude timeouts from the denominator. Report timeout counts separately.
- For each task: record the rank of the correct answer among all candidates (rank 1 = solved).
- **No test set contamination.** Scoring must never access test output colors. The correct output is used only to compute rank after all candidates are scored.

## Output Format

Each method run produces a line in `results/summary.tsv`:
```
method	seed	n_tasks	n_solved	n_timeout	solve_rate	wall_seconds
```

A script `scripts/compare.sh` should:
1. Aggregate `results/summary.tsv` into `results/comparison.md` — a Markdown table with mean ± std
2. For each pair of adjacent methods (M0-rust vs M1, M1 vs M2, etc.), compute the number of tasks where the better method solves and the worse doesn't (and vice versa). This is more informative than aggregate rates.

## Verification Architecture

### V1: Unit tests (`cargo test`)
- `plucker_tests.rs`: Verify Plücker relation for constructed lines. Verify incidence: lines from coplanar point pairs should satisfy ⟨p, q⟩_★ = 0. Verify transversal solve: construct 4 lines with analytically known transversals, confirm recovery.
- `embedding_tests.rs`: Verify dimensions. Verify determinism (same input → same output).
- `smoke_tests.rs`: Run M0-rust on task `0d3d703e`, verify the correct answer gets rank 1 (or at least top 10).

Run `cargo test` before every experiment. CI should enforce this if possible.

### V2: Cross-validation against upstream C binary
Run both `./arc_solver` (C) and the Rust M0-rust on the same 10 tasks. They use different seeds so won't match exactly, but both should solve the same easy tasks (e.g., `0d3d703e`). If M0-rust fails on tasks that M0 solves, investigate — it may indicate an implementation bug.

### V3: Subagent — Code Review
After implementing each method, spawn a subagent (separate context, no shared history) with this prompt:

```
You are reviewing Rust code for an ARC-AGI solver. The method being reviewed is: [METHOD_NAME].
It is supposed to: [ONE_SENTENCE_DESCRIPTION].

Review the code in the following files for:
1. Off-by-one errors in grid indexing (grids are row-major, 0-indexed)
2. Correct normalization (vectors normalized when they should be?)
3. Correct sign conventions (J6 matrix entries, Plücker relation signs)
4. Numerical stability (division by near-zero, log of zero, NaN propagation)
5. Any way the scoring could accidentally leak the correct answer
6. Any way the method could accidentally share state/RNG between tasks (each task must be independent)

Report issues as a numbered list with file:line references. If no issues found, say so.
```

### V4: Subagent — Statistical Review
After all experiments, spawn a subagent with:

```
You are a statistician reviewing experimental results comparing 8+ methods for solving ARC-AGI tasks.
Read results/comparison.md and results/summary.tsv (attached).
Answer:
1. Are the error bars (std across 20 seeds) consistent with the observed variation?
2. Is 20 seeds sufficient to distinguish adjacent methods? Compute minimum detectable effect size at 80% power.
3. Does timeout rate vary by method in a way that biases comparisons?
4. Are there tasks solved by simpler methods but not complex ones (or vice versa)? What does the overlap look like?
5. Suggest additional analyses if warranted.
Write your review to results/statistical_review.md.
```

### V5: Subagent — Writeup Review
Before finalizing README.md, spawn a subagent with:

```
You are a mathematics PhD student proofreading a technical writeup about Plücker geometry applied to ARC-AGI.
Target audience: mathematicians at MSc level, familiar with linear algebra and basic algebraic geometry.
Read README.md (attached).
Check:
1. All terms defined before use?
2. Theorem/proof pairs logically complete? Flag gaps.
3. Notation consistent throughout?
4. Experimental conclusions supported by the data in the tables?
5. Any claims stronger than the evidence?
6. Bad style: verbosity, unclear antecedents, ambiguous quantifiers, missing punctuation.
Write review to results/writeup_review.md with line-number references. Be harsh.
```

## README.md Structure (Final Deliverable)

Replace the upstream README.md with this structure. Write incrementally as results arrive.

```markdown
# [Title TBD — based on findings, e.g., "What Actually Drives the Transversal ARC Solver?"]

## Abstract
[3–4 sentences: question, method of investigation, key finding, implication]

## 1. Background
### 1.1 ARC-AGI
### 1.2 The Transversal Method
[Concise description of upstream pipeline. Reference the original repo.]
### 1.3 Research Questions

## 2. Mathematical Framework
### 2.1 Plücker Coordinates and Gr(2,4)
[Definition, Theorem (Plücker relation), Proof]
### 2.2 Incidence and the Hodge Star
[Definition (J6), Theorem (incidence criterion), Proof]
### 2.3 Transversal Computation
[Theorem (Schubert), constructive algorithm with SVD + quadratic, Proposition (coefficient formulas)]
### 2.4 Score Decomposition
[Proposition (table decomposition), why it enables exhaustive search]

## 3. Ablation Study
### 3.1 Methods M0–M7
[Table: method name, what it tests, what it removes from the pipeline]
### 3.2 Evaluation Protocol
### 3.3 Seed Stability Protocol

## 4. Results
### 4.1 Main Comparison Table
### 4.2 Seed Stability (CV analysis)
### 4.3 Per-Task Overlap Analysis
### 4.4 Upstream Reproduction (M0 vs claimed results)

## 5. Discussion
### 5.1 Does geometry matter?
### 5.2 What actually drives performance?
### 5.3 Relation to upstream author's own ablation findings
### 5.4 Limitations of this study

## 6. Conclusion

## Appendix A: Embedding Function Definitions
## Appendix B: Full Results Tables (all seeds)
## Appendix C: Reproduction Instructions
```

## Workflow

1. **Step 0 — Run upstream** (15 min): Compile `arc_solver.c`, run on all training tasks, save raw output. Confirm or refute upstream claims. Clone ARC data.
2. **Step 1 — Rust scaffolding** (45 min): `cargo init ablations`, implement task parsing + same-size filter + CLI skeleton + M7 (trivial baseline). Run M7 on all tasks. This validates the evaluation harness.
3. **Step 2 — M0-rust** (2–3 hrs): Full reimplementation. Run V1 unit tests. Run V3 code review subagent. Cross-validate against C binary (V2). Run with 3 seeds to sanity-check.
4. **Step 3 — Ablations M1–M6** (1 hr each): Implement top-down from M1 to M6. After each: `cargo test`, smoke test on `0d3d703e`, run on all tasks with 3 seeds.
5. **Step 4 — Full seed sweep** (batch): All methods × 20 seeds. `cargo run --release -- --method all --seeds 1000-1019 --all data/ARC-AGI/data/training/ > results/summary.tsv`
6. **Step 5 — Analysis** (1 hr): Generate comparison table. Run V4 statistical review subagent.
7. **Step 6 — Writeup** (2 hrs): Draft README.md. Run V5 writeup review subagent. Revise. Commit.

## Important Constraints

- **Do not modify `arc_solver.c`.** It is the upstream reference. If it doesn't compile, document the issue and work around it.
- **No seed shopping.** Never select results based on best seed. Always report mean ± std across all 20 seeds.
- **Same task set for all methods.** Every method evaluated on the same same-size tasks with the same 30s timeout.
- **No test set contamination.** Candidate scoring must never access test output colors.
- **Reproducibility.** Every result reproducible from a single CLI command. Document exact invocations in README.
- **Honesty.** If the full Plücker method genuinely outperforms simpler methods in a seed-stable way, report that. Do not assume the conclusion. The upstream author's `transversal-memory` ablations found geometry decorative for word association — ARC may differ.
- **The final README.md replaces the upstream one.** It must be a complete, self-contained document. A reader should not need to visit the upstream repo to understand the method or results.

## Quick Reference: Key Mathematical Objects

- **Plücker coordinates:** Given a,b ∈ R^4, line through them: p_{ij} = a_i·b_j - a_j·b_i for (i,j) ∈ {(01),(02),(03),(12),(13),(23)}.
- **Plücker relation:** p_{01}·p_{23} - p_{02}·p_{13} + p_{03}·p_{12} = 0.
- **J6 (Hodge star):** 6×6 matrix, J6 = antidiag(1, -1, 1, 1, -1, 1). Symmetric, J6² = I.
- **Incidence:** Lines p, q meet iff p^T J6 q = 0.
- **Transversal solve:** 4 lines L_i → A = [J6·L_1; ...; J6·L_4] (4×6) → SVD → null space {v1,v2} → solve Q(t·v1+v2) = 0 → ≤2 transversals.
- **Score:** Σ_T log(|L^T J6 T| + ε). More negative = line nearly meets transversal = good.
