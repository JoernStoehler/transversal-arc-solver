# The Transversal ARC Solver Doesn't Work

The [upstream repo](https://github.com/khalildh/transversal-arc-solver) claims 316 ARC-AGI tasks "solved" using Plücker geometry. We investigated. The number is an artifact of a broken evaluation methodology. On tasks where the result can actually be verified, the method solves 3 out of 12 — barely above guessing.

## What the upstream claims

The repo contains a 1,900-line C solver and a README claiming:

- 166/262 same-size training tasks solved at rank 1
- 150/270 same-size evaluation tasks solved at rank 1
- 316 total, using "zero learning" and projective geometry

## Why the number is meaningless

### The evaluation methodology

The solver assigns a score to every possible output grid for a task. "Solved" (rank 1) means the correct answer got the best score — no other candidate beat it.

For small grids (e.g., 3×3 with 3 colors = 19,683 candidates), this can be checked exhaustively. For large grids, it cannot: a 10×10 grid with 3 colors has 3^100 ≈ 5×10^47 possible outputs.

The solver handles large grids by **sampling**: draw 10 million random candidates, check if any scored better than the correct answer. If none did, declare "solved."

### Why sampling doesn't work

10 million out of 5×10^47 is a ratio of 2×10^-41. If the correct answer's true rank were 1,000,000 (meaning 999,999 candidates actually score better), the probability of finding even one of them in 10 million samples is:

    999,999 × 10^7 / 5×10^47 ≈ 2×10^-35 ≈ zero

The sampling test cannot distinguish "rank 1" from "rank 1,000,000" on large tasks. Any scoring function with even minimal signal — including trivially counting color-pair frequencies — will pass this test on nearly every task.

We confirmed this: a method that simply counts how often each pair of adjacent colors appeared in training (method M6, ~30 lines of logic, no linear algebra) "solves" **233/262 tasks** by the same sampling standard. That doesn't mean counting works on 89% of ARC tasks. It means the test is broken.

### What happens when you actually check

Only 12 of the 262 tasks have search spaces small enough to check every candidate (≤200 million). On those 12 tasks:

| Method | Solved | What it does |
|--------|--------|-------------|
| Upstream C binary | 3/12 | Full Plücker pipeline (1,900 lines of C) |
| M0-rust (our reimplementation) | 3/12 | Same pipeline, different random seeds |
| M6 (color-pair counting) | 2/12 | Count adjacent color frequencies in training |
| M7 (per-cell mode) | 2/12 | Guess the most common output color per cell |

M6 and M7 solve the same 2 tasks. The upstream pipeline adds 1 extra, thanks to a histogram lookup optimization (unrelated to the geometry). The Plücker coordinates, Grassmannian, transversals, and Schubert calculus contribute nothing: replacing the geometric quadratic solve with a random number (method M1) gives identical results on every task and seed tested.

## What we tested

We reimplemented the upstream pipeline in Rust (`ablations/`) and built a ladder of progressively simpler methods:

| Method | Description | Verified (12 tasks) | Sampling (262 tasks) |
|--------|-------------|---------------------|---------------------|
| M0 | Upstream C binary | 3/12 | unverified |
| M0-rust | Full reimplementation | 3/12 | — |
| M1 | Skip Plücker quadratic (random null-space vector) | 3/12 | — |
| M6 | Count color-pair frequencies | 2/12 | 233/262* |
| M7 | Per-cell most-common-color | 2/12 | 4/262 |

*Inflated by sampling, as explained above. M7's number is real because it makes a single deterministic prediction without scoring/sampling.

M0-rust and M1 were tested across 5 random seeds (1000–1004) on the 12 verified tasks. Both solve 3/12 on every seed. The Plücker quadratic never makes a difference.

## What we didn't test

- Full upstream run on all 262 tasks (C binary is single-threaded, takes 2+ hours, container CPU limits prevented completion)
- Methods M2–M5 (intermediate ablations; results were invalidated by a `thin_svd` bug and not re-run after fix, since the core finding was already clear)
- The 270 evaluation tasks

## The geometry is decorative

The upstream README devotes extensive space to Plücker coordinates, the Grassmannian Gr(2,4), Hodge duality, and Schubert calculus. Our method M1 replaces the only component that requires the Grassmannian (the quadratic solve for transversals) with a random number. Results are identical. This is consistent with the upstream author's own finding in their [transversal-memory](https://github.com/khalildh/transversal-memory) repo, where they concluded the geometry was decorative for word-association tasks.

## What actually helps (marginally)

The upstream C binary's one extra solve (3/12 vs 2/12) comes from **histogram-aware scoring**: for small tasks, it precomputes separate score tables for each possible output color distribution, giving the scoring function access to global information (how many cells of each color the candidate has). This is a lookup table optimization, not geometry. We confirmed this by implementing it in our Rust code — with histogram scoring, M0-rust matches the C binary at 3/12; without it, M0-rust drops to 2/12.

## What this tells us about ARC

Same-size ARC tasks are diverse. The 2 tasks solvable by per-cell guessing are trivial (the output is predictable cell-by-cell from training examples). The 1 additional task solved by the full pipeline is barely out of reach of guessing. The remaining 9/12 tasks require reasoning that no variant of local pairwise scoring captures — regardless of how much geometry you put around it.

The upstream's "316 solved" number obscures this by using a verification method that can't distinguish success from failure on large tasks. The actual verified solve rate is ~25%, most of which is achievable by guessing.

## Reproduction

```bash
sudo apt-get install -y liblapack-dev libblas-dev
git clone https://github.com/fchollet/ARC-AGI data/ARC-AGI

# Upstream C binary (single task)
cc -O3 -march=native -D'__CLPK_integer=int' -o arc_solver arc_solver.c -lm -llapack -lblas
./arc_solver data/ARC-AGI/data/training/794b24be.json

# Ablation suite
cd ablations && cargo build --release && cd ..

# Run on the 12 verifiable tasks
./ablations/target/release/ablations --method m0rust,m1,m6,m7 --seeds 1000-1004 --timeout 120 \
    --task data/ARC-AGI/data/training/0d3d703e.json \
    --task data/ARC-AGI/data/training/25ff71a9.json \
    --task data/ARC-AGI/data/training/5582e5ca.json \
    --task data/ARC-AGI/data/training/67a3c6ac.json \
    --task data/ARC-AGI/data/training/6e02f1e3.json \
    --task data/ARC-AGI/data/training/74dd1130.json \
    --task data/ARC-AGI/data/training/794b24be.json \
    --task data/ARC-AGI/data/training/9565186b.json \
    --task data/ARC-AGI/data/training/a85d4709.json \
    --task data/ARC-AGI/data/training/b1948b0a.json \
    --task data/ARC-AGI/data/training/d037b0a7.json \
    --task data/ARC-AGI/data/training/ed36ccf7.json
```

## Notes

- The upstream `arc_solver.c` was not modified
- The initial Rust reimplementation had a `thin_svd` bug (returning 4 instead of 6 right singular vectors), caught by a code-review subagent. All reported numbers use the corrected code.
- The upstream repo's single commit is co-authored by Claude. There are no logs, result files, or other evidence the code was run before publication.

---

*Independent analysis of [khalildh/transversal-arc-solver](https://github.com/khalildh/transversal-arc-solver). Ablation code in `ablations/`.*
