# Writeup Review: "What Actually Drives the Transversal ARC Solver?"

Reviewer notes. Line references are to `/workspaces/transversal-arc-solver/README.md`.

---

## 1. Terms Not Defined Before Use

- **Line 3:** "Gr(2,4)" appears in the abstract before any definition. The formal definition arrives at line 47. An MSc reader who has not seen Grassmannians will be lost for two pages.
- **Line 3:** "Plücker geometry" is name-dropped but never actually defined as a term. Section 2.1 defines Plücker *coordinates*; the phrase "Plücker geometry" is left informal throughout.
- **Line 5:** "Schubert calculus" appears with no definition anywhere in the document. Line 63 mentions it again ("By Schubert calculus, this has 0 or 2 real solutions") but never states what Schubert calculus is or why it implies the claimed count of solutions. A reader at MSc level in linear algebra will not know this term.
- **Line 22:** "Plücker line" is used before the Plücker coordinate definition at line 39. The pipeline description in Section 1.2 relies on Section 2 material.
- **Line 24:** "J₆" appears in the pipeline summary (line 24) before its definition at line 51.
- **Line 51:** "Hodge star on Plücker coordinates" -- this is a loose usage. The Hodge star is an operator on exterior algebras; J₆ is a specific matrix representation of the Klein correspondence's duality. Calling it "the Hodge star" without qualification will mislead geometers who expect the Hodge star on differential forms.

## 2. Theorem/Proof Gaps

- **Line 43-46 (Plücker relation):** Stated as a fact with no proof or citation. Fine for a writeup, but the word "iff" carries logical weight. The forward direction (Plücker coordinates of a line satisfy the relation) is trivial; the converse (every point on the quadric is the Plücker vector of some line) is the non-trivial content of the Grassmannian embedding theorem. A citation is needed.
- **Line 55:** "Two lines p, q meet iff p^T J₆ q = 0." Again stated without proof or citation. This is the reciprocity condition and deserves at minimum a reference (e.g., Hodge & Pedoe, or Harris's *Algebraic Geometry*).
- **Line 59-63 (Transversal computation):** This is the mathematical core and it is underspecified:
  - The claim "SVD gives null space {v₁, v₂}" assumes the 4x6 matrix A has rank exactly 4. No argument is given for why this should hold generically. What happens when the 4 lines are not in general position? The method samples random training lines, so degeneracy is plausible.
  - "By Schubert calculus, this has 0 or 2 real solutions" -- this is stated as a theorem but no proof or reference is provided. This is the central geometric claim of the upstream method, and a reader cannot verify it from what is written. The Schubert calculus argument requires showing the intersection number sigma_1^4 = 2 in the Chow ring of Gr(2,4). None of this is even sketched. The claim also conflates the algebraic intersection number (always 2, counted with multiplicity over the algebraic closure) with the number of *real* solutions (which can be 0 or 2). This distinction matters and is not addressed.
- **Line 65:** "The resulting T is a valid null-space vector but does NOT necessarily lie on the Grassmannian." This is the ablation's key mathematical claim. It would be stronger if accompanied by a brief argument: a generic point in the null space does not satisfy the Plücker relation because... (e.g., the quadric is codimension 1 in P^5, so a random point in a 2-dimensional subspace of R^6 has measure-zero probability of landing on it). As written, it is merely asserted.

## 3. Notation Inconsistencies

- **Line 20:** "R^d" uses plain text. Line 22 uses "R^{4×2d}" and "R⁴" (Unicode superscript). Lines 39-41 use subscript notation "p_{ij}" mixing underscore-brace with Unicode subscripts (p₀₁ at line 45). Pick one convention and stick with it.
- **Line 21:** "d ∈ {20..44}" -- the ".." range notation is informal. Use "d ∈ {20, 22, ..., 44}" or state the set explicitly.
- **Line 22:** "W1, W2 ∈ R^{4×2d} → two points in R⁴" -- the arrow is doing double duty as both a mapping symbol and a "resulting in" indicator. This is ambiguous. Does W1 map R^{2d} to R^4? Then write W1 : R^{2d} -> R^4, W1 in R^{4 x 2d}.
- **Line 41:** The index set "{(01),(02),(03),(12),(13),(23)}" uses concatenated digits without separators. This works for single-digit indices but is non-standard. Use (0,1), (0,2), etc.
- **Line 53:** "antidiag(1, −1, 1, 1, −1, 1)" -- "antidiag" is not standard notation. Define it or write the matrix explicitly.
- **Line 59:** "A = [J₆·L₁; ...; J₆·L₄] (4×6)" -- semicolons for row stacking is MATLAB convention, not standard mathematical notation. Use "the matrix whose i-th row is (J₆ L_i)^T" or similar.
- **Line 60:** "α·t² + β·t + γ = 0" uses centered dot for multiplication, but line 44 uses plain juxtaposition ("a_i · b_j" vs "p₀₁ · p₂₃"). Inconsistent.
- **Line 69:** "L(candidate)" -- L is suddenly a function; it was previously used for lines L₁,...,L₄. Overloaded.

## 4. Experimental Conclusions vs. Data

### 4.1 The central claim is tested on far too few tasks.

- **Line 133:** "The Plücker constraint adds nothing" is the paper's main conclusion. It rests on 12 tasks with 10 seeds. Twelve tasks is a minuscule fraction of the 400+ ARC tasks, and these are specifically the *smallest* tasks (<=9 cells, <=8 colors, per line 177). The authors acknowledge this limitation at line 177 but the conclusion at line 133 is stated without any hedge. The conclusion should say "adds nothing on the 12 smallest tasks we tested" rather than making a blanket assertion.

### 4.2 Table 4.1 vs. underlying data (M7 discrepancy).

- **Line 112:** Table 4.1 reports M7 as "Solved/30 = 4". However, the data file `m7_results.tsv` shows M7 was run on **262 tasks** (not 30), solving 4. If those 4 solves are distributed across the full 262, we cannot know without per-task data how many fall in the 30-task subset. The table may be reporting the wrong denominator or the wrong numerator. This needs clarification or per-task results.

### 4.3 Table 4.2 is misleading about the C binary.

- **Lines 118-124:** Table 4.2 shows M0 (C binary) solving 3/12. The raw upstream output (`upstream_12tasks.txt`) shows that task 25ff71a9 achieves rank 1 but is reported as "LIKELY RANK 1" rather than "SOLVED" by the C binary's own heuristic. So the C binary *does* rank the correct answer first on 4 of 12 tasks, but its internal verification flag accepts only 3. This distinction is relevant to the seed-sensitivity argument in Section 5.3 and should be disclosed. The gap between M0-rust (12/12) and M0 (3/12) is partly inflated by the C binary's conservative verification.

### 4.4 "100% solve rate" is ceiling-limited.

- **Lines 105-106, 130-131:** Both M0-rust and M1 achieve 100% on all tested tasks. When a metric is saturated, you cannot distinguish the methods. The correct conclusion is "M1 is no worse than M0-rust on these tasks," not "M1 equals M0-rust." The methods could diverge on harder tasks where neither achieves 100%. This is a fundamental limitation of the evaluation that is never explicitly discussed.

### 4.5 The 30-task and 12-task subsets are not characterized.

- **Lines 89, 101, 114:** We are told tasks are "same-size" and "small." What is the distribution of grid sizes, number of colors, and number of training pairs in each subset? Without this, the reader cannot assess how representative the sample is.

## 5. Claims Stronger Than Evidence

- **Line 5:** "The Plücker quadratic constraint ... is decorative." This is the strongest possible claim, and it rests on 12 tiny tasks. On the same 12 tasks, both methods hit the ceiling (100%), so the experiment cannot detect any performance difference even if one exists. The claim should be hedged: "appears decorative on the tasks we tested."
- **Line 5:** "Replacing it with a random vector from the null space produces identical results." "Identical" means exactly equal scores on every candidate for every task. The data only shows identical *solve rates* (both rank 1). The actual numerical scores may differ. This conflation of "same rank-1 outcome" with "identical results" is imprecise.
- **Line 145:** "This directly confirms Hypothesis B." A sample of 12 tasks does not "directly confirm" anything. It is *consistent with* Hypothesis B.
- **Line 168:** "functionally irrelevant" -- again, only demonstrated on the smallest tasks with saturated performance.
- **Line 172:** "the upstream result of 316 tasks is partly an artifact of seed selection" -- this is a serious accusation supported by a comparison of *different implementations* (C vs. Rust) with different RNGs (MT19937 vs. ChaCha8), different embedding implementations, and different seeds. The claim that "the ONLY difference being the random projection seeds" (line 172) is false by the authors' own admission at line 178 ("M0-rust uses different random projections than M0"). The comparison confounds implementation differences with seed differences.
- **Line 183:** "happens to be dressed in beautiful mathematics" -- editorializing. Either the geometry contributes or it does not; calling it "dressing" presupposes the conclusion.

## 6. Style Issues

- **Line 3:** "which claims 316 ARC-AGI tasks solved at rank 1 using Plücker geometry on the Grassmannian Gr(2,4)" -- dangling modifier. The relative clause "which claims" has "transversal-arc-solver" as its antecedent, but a GitHub repository does not "claim" anything. The authors or the paper do.
- **Line 5:** "Bottom line:" is informal for a technical writeup. Use "Summary" or fold into the abstract.
- **Line 9:** Sentence beginning "We built 8 progressively simpler methods (M0-rust through M7)..." -- M0-rust is the full reimplementation, so the methods are not "progressively simpler" starting from M0-rust. M0-rust is the baseline. The progression is M1 through M7.
- **Line 15:** "8⁹ ≈ 134 million" -- 8^9 = 134,217,728. Fine, but this example is misleading because the next sentence's context suggests this is the typical search space. Most ARC grids are much larger than 3x3.
- **Line 21:** "d ∈ {20..44}" has no antecedent for what d represents in this context. Is it per-cell embedding dimension? Per-pair? State explicitly.
- **Line 25:** "for O(1) per-candidate table lookup" -- mixing big-O notation into a pipeline description is jarring. Say "constant-time" or explain the complexity model.
- **Line 63:** "this has 0 or 2 real solutions" -- omits the possibility of a repeated root (1 solution). Over the reals, the discriminant can be zero. This case should be mentioned even if it is generically avoided.
- **Line 92:** "nc^(h·w)" -- "nc" is not defined. Presumably "number of colors." Define it.
- **Line 137:** "the single-threaded C binary takes 2+ hours on all 262 tasks" -- is this an excuse for not reproducing the full result? If so, state explicitly that this is a limitation and what resources were available. "2+ hours" is not an unreasonable runtime for a research study.
- **Line 153-159:** The "performance hierarchy" lists methods with percentages, but these are from the 30-task subset (Table 4.1), which was only tested at a single seed. This is not acknowledged in the discussion.
- **Line 172:** "even though no deliberate seed shopping occurred (the seeds are deterministic from embedding name hashes)" -- the parenthetical is an important methodological point buried in the middle of a paragraph. It deserves its own sentence.
- **Line 200:** "ChaCha8 RNG" -- not defined or cited. A reader may not know this is a cryptographic PRNG.

## 7. Structural Issues

- The abstract (line 7-9) states results for "262 same-size ARC training tasks," but the actual experiments use 30 tasks (Table 4.1) and 12 tasks (Tables 4.2-4.3). The abstract is misleading about the scale of evaluation.
- Section 2 (Mathematical Framework) defines objects that are used in Section 1.2's pipeline description. Either move the math before the pipeline or remove technical terms from Section 1.2.
- There are no confidence intervals, p-values, or statistical tests anywhere. With binary outcomes (solved/not) on 12-30 tasks, even a McNemar test or exact binomial confidence interval would help. The absence of any statistical framework makes claims like "zero variance" (line 133) vacuously true -- of course there is zero variance when both methods saturate to 100%.
- The paper never reports per-task ranks for M0-rust vs. M1. Showing that they achieve the same rank on every task would be much stronger evidence than showing they achieve the same solve rate. If the ranks differ but both happen to be rank 1, the "identical" claim is weaker than it appears.

## Summary

The writeup presents an interesting ablation study but systematically overstates its conclusions. The central finding -- that the Plucker quadratic can be replaced by a random null-space vector without loss on 12 small tasks -- is genuinely informative but is presented as a definitive refutation of the geometric approach. The evaluation is too small (12-30 tasks), too easy (100% ceiling), and too confounded (different RNGs, different implementations) to support the sweeping claims made in the abstract and conclusion. The mathematical exposition uses terms before defining them, lacks citations for key results, and is notationally inconsistent. The seed-sensitivity argument in Section 5.3 confounds implementation differences with seed differences, undermining the claim that "316 tasks is partly an artifact of seed selection."
