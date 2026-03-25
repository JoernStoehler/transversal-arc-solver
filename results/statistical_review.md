# Statistical Review of Ablation Study Results

## Overview of Available Data

The ablation study compares 8 methods (M0-rust, M1, M2, M3, M4, M5, M6, M7) for solving ARC-AGI tasks. The data comprises:

- **Seed stability runs:** M0-rust and M1, each with 10 seeds (1000--1009), on 12 small exhaustive tasks.
- **Fast comparison:** All 8 methods on 30 tasks, single seed (1000).
- **M7 full run:** 262 tasks, single seed.
- **Upstream C binary (M0):** 12 tasks, single hardcoded seed.

---

## 1. Are the error bars (std across 10 seeds) consistent with the observed variation?

**The observed standard deviations are trivially zero, which is consistent but uninformative.**

Both M0-rust and M1 solve 12/12 tasks on every one of the 10 seeds tested. The standard deviation in solve count is 0.0 and the coefficient of variation is 0.00 for both methods. Wall-clock time shows minor variation (M0-rust: mean 51.43s, sd ~0.97s; M1: mean 51.64s, sd ~0.86s), consistent with ordinary system-level jitter.

The problem is that these 12 tasks were selected specifically because they are small enough for exhaustive candidate enumeration (output grids up to 9 cells, up to 8 colors). On these tasks, both methods achieve a ceiling effect --- the correct answer receives rank 1 with probability 1.0 across all seeds. A zero standard deviation from a ceiling effect tells us nothing about whether the methods would differ on harder tasks.

**Verdict:** The error bars are mathematically correct but statistically vacuous. The task set is too easy for these two methods to exhibit any variance. Seed stability testing on the 12-task subset cannot distinguish M0-rust from M1 because both saturate.

---

## 2. Is 10 seeds sufficient to distinguish adjacent methods? Minimum detectable effect size.

### Formal power analysis

For comparing two proportions (solve rates) using a two-sample z-test at significance level alpha = 0.05 with power = 0.80:

The minimum detectable effect size depends on the baseline rate and the sample size per group. Here n = 10 seeds, and each seed yields a solve rate over k tasks.

**Case A: Comparing solve counts on 12 exhaustive tasks (the actual seed stability data).**

With n = 10 seeds per method and tasks per seed k = 12, the outcome per seed is a count in {0, ..., 12}. The standard deviation of the solve rate under the null (both methods at p = 1.0) is 0.0, so the test is degenerate --- no non-trivial difference can be detected because there is no variance. This is the ceiling problem noted above.

**Case B: Hypothetical --- had seed stability been run on the 30-task set.**

For a paired t-test on solve counts across 10 seeds, with n = 10 paired observations, df = 9, at alpha = 0.05 (two-sided) and power = 0.80, the critical t-value is ~2.262 and the required standardized effect size (Cohen's d) is approximately:

    d = t * sqrt(1/n) = 2.262 / sqrt(10) * correction ≈ 0.99

This means we would need an effect size of roughly d = 1.0 (a "large" effect by Cohen's conventions) to reliably detect a difference with only 10 seeds. In practical terms, if the within-seed standard deviation of the difference in solve counts were, say, 3 tasks, we would need a mean difference of at least 3.0 tasks (10 percentage points on 30 tasks) to achieve 80% power.

**Case C: Hypothetical --- full 262-task set with 10 seeds.**

Same framework. With sd of the difference estimated at ~5 tasks (plausible for stochastic methods), we would need a mean difference of ~5 tasks (1.9% solve rate difference) for detection. Better, but still coarse.

**Conclusion:** 10 seeds is inadequate for distinguishing methods with small effect sizes. The original protocol called for 20 seeds, which would improve the minimum detectable effect size by a factor of sqrt(2) (to roughly d = 0.70). Even 20 seeds is marginal for detecting differences smaller than ~7 percentage points on 30 tasks. For the 12-task exhaustive set, no number of seeds will help when both methods are at ceiling.

**Recommendation:** To meaningfully compare M0-rust and M1, run both on the full 30-task set (or ideally all 262 same-size tasks) across at least 20--50 seeds. The current data cannot rule out small but real differences between the methods on harder tasks.

---

## 3. Does timeout rate vary by method in a way that biases comparisons?

**Yes, dramatically, and this is the single largest methodological concern in the study.**

From the 30-task fast comparison:

| Method   | Solved | Timeouts | Timeout Rate | Non-timeout Non-solved |
|----------|--------|----------|--------------|------------------------|
| M0-rust  | 30     | 0        | 0%           | 0                      |
| M1       | 30     | 0        | 0%           | 0                      |
| M2       | 5      | 9        | 30%          | 16                     |
| M3       | 0      | 9        | 30%          | 21                     |
| M4       | 1      | 9        | 30%          | 20                     |
| M5       | 2      | 1        | 3%           | 27                     |
| M6       | 14     | 2        | 7%           | 14                     |
| M7       | 4      | 0        | 0%           | 26                     |

Methods M2, M3, and M4 all hit exactly 9 timeouts (30% timeout rate), and their wall times (~757--775s) are dramatically higher than the other methods (~264--300s). This pattern suggests these methods hit the 30-second per-task timeout on the same 9 tasks, likely the tasks with the largest candidate spaces.

**Bias mechanism:** A timed-out task is scored as unsolved regardless of whether the method might have found the correct answer given more time. Methods M2--M4 are architecturally slower (M2 uses training lines directly without table decomposition; M3--M4 lack the efficient score table) and thus are disproportionately penalized by the 30-second timeout. Their true solve rates could be higher than reported.

**Conversely**, M6 and M7 are extremely fast (M7: 0.0s wall time for 30 tasks) and have 0--2 timeouts, meaning their solve rates reflect genuine accuracy rather than speed limitations.

**Impact on conclusions:** The key comparison (M0-rust/M1 vs. everything else) is not seriously affected --- M0-rust and M1 have zero timeouts and their superiority is clear regardless. However, the relative ordering among M2--M7 is confounded by timeout bias. For example, M2 might outperform M6 if given unlimited time, but we cannot tell from the current data.

**Recommendation:** For methods M2--M5, report results separately for tasks that did NOT time out. Alternatively, re-run with a longer timeout (120s, matching the upstream protocol) or on the 12-task exhaustive subset where timeouts are not a factor.

---

## 4. Task overlap: are there tasks solved by simpler methods but not complex ones?

### Available evidence

Direct per-task breakdowns are not available in the result files for the 30-task fast comparison. However, we can infer some overlap structure:

**12-task exhaustive results (from comparison.md Section 4.2):**

| Method   | Solved/12 |
|----------|-----------|
| M0-rust  | 12        |
| M1       | 12        |
| M0 (C)   | 3         |
| M6       | 2         |
| M7       | 2         |
| M2       | 1         |
| M3--M5   | 0         |

**Key observations:**

1. **M0 (C binary) solves only 3/12 tasks that M0-rust solves 12/12.** This is the starkest demonstration of seed sensitivity. Both implement the same algorithm; only the random seeds differ. The 9 tasks solved by M0-rust but not M0 (C) demonstrate that seed choice is a dominant factor.

2. **M6 (frequency counting) solves 2/12 exhaustive tasks and 14/30 fast tasks.** On the 30-task set, M6 solves 14 tasks --- nearly half --- with no linear algebra at all. This is a surprisingly strong trivial baseline. Some of these 14 tasks are presumably NOT among the 30 solved by M0-rust/M1 (since M0-rust/M1 solve all 30, they are a superset), so there are no tasks where M6 succeeds and M0-rust fails on this set.

3. **M7 (per-cell mode) solves 4/30 fast tasks.** These 4 tasks have a simple per-cell color mapping structure. M7 is a strict subset of what M6 can solve on the 12-task set (both solve 2), but on 30 tasks M7 solves 4 while M6 solves 14. M6 strictly dominates M7 in terms of information used (adjacency pairs vs. individual cells).

4. **Upstream per-task data (upstream_12tasks.txt):** The upstream C binary solves tasks `0d3d703e`, `794b24be`, and `b1948b0a` (rank 1) out of 12 tested. Task `25ff71a9` gets "LIKELY RANK 1" but is not counted as solved (the score difference was nonzero: diff=5.05e+03). The remaining 8 tasks get ranks far from 1.

### Inferred overlap structure (30-task set)

Since M0-rust and M1 solve all 30/30, every task solved by any simpler method is also solved by M0-rust/M1. The interesting question is whether any simpler method solves a task that another simpler method misses:

- M6 solves 14 tasks; M2 solves 5 tasks; M7 solves 4 tasks. Without per-task data, we cannot confirm whether M2's 5 and M7's 4 are subsets of M6's 14.
- M5 (2 solved) and M4 (1 solved) are likely subsets of the others, but this is not guaranteed without task-level output.

**Recommendation:** The experimental harness should output per-task solve/fail status for each method, not just aggregate counts. This would enable a proper Venn diagram or upset plot showing which method combinations cover which tasks. The per-task data is essential for understanding whether methods are complementary or strictly nested.

---

## 5. Additional Analyses Warranted

### 5.1. Per-task solve data (Critical)

As noted above, per-task binary outcomes (solved/not-solved) for each method are essential. With this data, one could compute:
- McNemar's test for paired method comparisons (more appropriate than independent-sample tests since methods are evaluated on the same tasks).
- Jaccard similarity between method solve sets.
- An upset plot showing the partition of tasks by which methods solve them.

### 5.2. Seed stability on harder tasks (High priority)

The seed stability analysis on the 12-task set is uninformative due to ceiling effects. Run M0-rust and M1 on the full 30-task set (or all 262 tasks) across 10--50 seeds. If both still show zero variance, that strengthens the "Plucker constraint is decorative" claim. If M1 shows even occasional failures on harder tasks, the conclusion must be qualified.

### 5.3. Rank distribution analysis (Moderate priority)

For tasks not solved at rank 1, the rank of the correct answer is informative. A method that consistently places the correct answer at rank 2--10 is qualitatively different from one that places it at rank 10^6. The upstream log shows ranks like 14, 42, 910, 1972, 10090, 10329050, 132746453 for unsolved tasks --- a wide range suggesting some tasks are "almost solved" while others are completely missed. Reporting median rank (or log-rank) for unsolved tasks would characterize method quality beyond the binary solved/not-solved metric.

### 5.4. Confidence intervals on solve rates (Moderate priority)

For the 30-task single-seed comparison, Wilson score 95% confidence intervals for the solve proportions are:

| Method  | Solved/30 | Point Est. | 95% CI (Wilson)     |
|---------|-----------|------------|---------------------|
| M0-rust | 30/30     | 100.0%     | [88.6%, 100.0%]     |
| M1      | 30/30     | 100.0%     | [88.6%, 100.0%]     |
| M6      | 14/30     | 46.7%      | [29.8%, 64.2%]      |
| M2      | 5/30      | 16.7%      | [7.3%, 33.6%]       |
| M7      | 4/30      | 13.3%      | [5.3%, 29.7%]       |
| M5      | 2/30      | 6.7%       | [1.8%, 21.3%]       |
| M4      | 1/30      | 3.3%       | [0.6%, 16.7%]       |
| M3      | 0/30      | 0.0%       | [0.0%, 11.4%]       |

Note that M2's and M7's confidence intervals overlap substantially, as do M5's and M4's. Only the M0-rust/M1 vs. everything-else gap, and M6 vs. M3/M4/M5, are statistically significant at the 5% level on this sample.

### 5.5. Effect of timeout threshold (Low priority)

Re-running M2--M5 with 60s and 120s timeouts would clarify how much of their poor performance is due to computational limits vs. genuine inability to identify the correct answer. The upstream protocol uses 120s; the ablation study uses 30s. This asymmetry weakens comparisons to the upstream claimed rate of 166/262.

### 5.6. Bootstrap analysis of the M0 vs. M0-rust seed gap (Low priority)

The finding that M0-rust solves 12/12 and M0 (C) solves 3/12 on the same tasks, with the only difference being random seeds, is striking but based on a single seed per implementation. Running M0-rust with M0's MT19937 seed sequence (if recoverable from the C source) would confirm whether the gap is truly due to seed quality or whether there are subtle implementation differences.

---

## Summary of Findings

| Question | Assessment |
|----------|-----------|
| Error bars consistent? | Yes, but trivially so --- both tested methods are at ceiling on the 12-task set. The zero variance is a ceiling artifact, not evidence of robustness on harder tasks. |
| 10 seeds sufficient? | No, for small effects. Minimum detectable effect size is ~d = 1.0 (large). The protocol called for 20 seeds; even that is marginal. The ceiling effect on the 12-task set makes the question moot for M0-rust vs. M1. |
| Timeout bias? | Yes, severe. M2--M4 each timeout on 30% of tasks. Their solve rates are confounded with computational speed. M0-rust/M1's superiority is robust to this, but ordering among M2--M7 is unreliable. |
| Task overlap? | Cannot fully assess without per-task data. M0-rust/M1 are strict supersets of all other methods on all tested tasks. The overlap structure among weaker methods is unknown. |
| Overall | The central claim (Plucker constraint is decorative) is well-supported for the tested tasks, but the evidence base is narrow: 12 easy tasks at ceiling with 10 seeds, plus 30 tasks with a single seed. Extending seed stability testing to harder tasks and collecting per-task data would substantially strengthen the analysis. |
