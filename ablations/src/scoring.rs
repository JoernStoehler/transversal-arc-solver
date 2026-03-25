use crate::embeddings::{self, EMB_DIMS, EMB_NAMES, N_EMBEDDINGS};
use crate::plucker;
use crate::task::{self, ArcTask, Grid};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rand_distr::{Distribution, StandardNormal};
use rayon::prelude::*;

const N_TRANS_PER_PAIR: usize = 200;
const MAX_HIST_TABLES: usize = 2000;

/// Generate all histograms: compositions of `total` into `nc` bins.
/// Each histogram is a Vec of nc counts summing to total.
fn gen_all_histograms(total: usize, nc: usize, max_count: usize) -> Vec<Vec<usize>> {
    let mut result = Vec::new();
    let mut cur = vec![0usize; nc];
    fn recurse(pos: usize, rem: usize, nc: usize, cur: &mut Vec<usize>, result: &mut Vec<Vec<usize>>, max_count: usize) {
        if result.len() > max_count { return; }
        if pos == nc - 1 {
            cur[pos] = rem;
            result.push(cur.clone());
            return;
        }
        for k in 0..=rem {
            if result.len() > max_count { return; }
            cur[pos] = k;
            recurse(pos + 1, rem - k, nc, cur, result, max_count);
        }
    }
    recurse(0, total, nc, &mut cur, &mut result, max_count);
    result
}

/// Build histogram-aware score table for hist_color embedding.
/// For a given histogram (color distribution), compute the correct diff vector
/// and build score_table[adj][ca][cb].
fn build_hist_score_table(
    adj_pairs: &[[usize; 4]],
    used_colors: &[i32],
    nc: usize, h: usize, w: usize,
    test_inp: &[i32],
    inp_hist: &[f32; 10], // input color counts (not normalized)
    hist: &[usize],       // candidate histogram: counts per color index
    w1: &[f32], w2: &[f32],
    jtm: &[f32], n_trans: usize,
) -> Vec<f32> {
    let hw = h * w;
    let dim = 30; // hist_color
    let n_adj = adj_pairs.len();
    let inv_sz = 1.0 / (hw.max(1) as f32);

    // Compute diff vector for this histogram
    let mut diff = [0.0f32; 10];
    for ci in 0..nc {
        let out_count = hist[ci] as f32;
        let color = used_colors[ci] as usize;
        if color < 10 {
            diff[color] = (out_count - inp_hist[color]) * inv_sz;
        }
    }

    let mut table = vec![0.0f32; n_adj * nc * nc];
    for ai in 0..n_adj {
        let [r1, c1, r2, c2] = adj_pairs[ai];
        let in_c_a = test_inp[r1 * w + c1];
        let in_c_b = test_inp[r2 * w + c2];

        for ca in 0..nc {
            // Build ea = [in_oh_a(10), out_oh(ca)(10), diff(10)]
            let mut ea = [0.0f32; 30];
            if (in_c_a as usize) < 10 { ea[in_c_a as usize] = 1.0; }
            let out_a = used_colors[ca] as usize;
            if out_a < 10 { ea[10 + out_a] = 1.0; }
            ea[20..30].copy_from_slice(&diff);

            for cb in 0..nc {
                let mut eb = [0.0f32; 30];
                if (in_c_b as usize) < 10 { eb[in_c_b as usize] = 1.0; }
                let out_b = used_colors[cb] as usize;
                if out_b < 10 { eb[10 + out_b] = 1.0; }
                eb[20..30].copy_from_slice(&diff);

                let sc = if let Some(line) = plucker::make_line_f32(&ea, &eb, dim, w1, w2) {
                    plucker::score_line(&line, jtm, n_trans)
                } else {
                    0.0
                };
                table[ai * nc * nc + ca * nc + cb] = sc;
            }
        }
    }
    table
}

/// Score a candidate using histogram tables. Computes the candidate's histogram,
/// finds the matching table, then sums adjacency scores from that table.
fn score_candidate_hist(
    cand: &[usize],
    adj_pairs: &[[usize; 4]],
    nc: usize, w: usize,
    all_hists: &[Vec<usize>],
    hist_tables: &[Vec<f32>], // one table per histogram, each (n_adj * nc * nc)
) -> f32 {
    // Compute histogram of candidate
    let mut hist = vec![0usize; nc];
    for &ci in cand {
        hist[ci] += 1;
    }

    // Find matching histogram
    let found = all_hists.iter().position(|h| h == &hist);
    let Some(hi) = found else { return 0.0 };

    let table = &hist_tables[hi];
    let mut total = 0.0f32;
    for (ai, &[r1, c1, r2, c2]) in adj_pairs.iter().enumerate() {
        let ca = cand[r1 * w + c1];
        let cb = cand[r2 * w + c2];
        total += table[ai * nc * nc + ca * nc + cb];
    }
    total
}

/// Generate random projection matrices W1, W2 for a given embedding.
/// Each is 4 x (2*dim), stored row-major.
pub fn gen_projections(seed: u64, dim: usize) -> (Vec<f32>, Vec<f32>) {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let sz = 4 * 2 * dim;
    let w1: Vec<f32> = (0..sz)
        .map(|_| { let v: f64 = StandardNormal.sample(&mut rng); v as f32 * 0.1 })
        .collect();
    let w2: Vec<f32> = (0..sz)
        .map(|_| { let v: f64 = StandardNormal.sample(&mut rng); v as f32 * 0.1 })
        .collect();
    (w1, w2)
}

/// Build score tables for all (adjacency_pair, color_a, color_b) combos.
/// Returns flat array of shape (n_adj, nc, nc).
pub fn build_score_table(
    embs: &[f32],  // (h*w, nc, dim)
    dim: usize,
    adj_pairs: &[[usize; 4]],
    w1: &[f32],
    w2: &[f32],
    jtm: &[f32],
    n_trans: usize,
    nc: usize,
    _h: usize,
    w: usize,
) -> Vec<f32> {
    let n_adj = adj_pairs.len();
    let mut table = vec![0.0f32; n_adj * nc * nc];

    // Parallel over adjacency pairs
    table
        .par_chunks_mut(nc * nc)
        .enumerate()
        .for_each(|(ai, chunk)| {
            let [r1, c1, r2, c2] = adj_pairs[ai];
            let idx_a = r1 * w + c1;
            let idx_b = r2 * w + c2;

            for ca in 0..nc {
                let ea = &embs[(idx_a * nc + ca) * dim..(idx_a * nc + ca) * dim + dim];
                for cb in 0..nc {
                    let eb = &embs[(idx_b * nc + cb) * dim..(idx_b * nc + cb) * dim + dim];

                    let sc = if let Some(line) = plucker::make_line_f32(ea, eb, dim, w1, w2) {
                        plucker::score_line(&line, jtm, n_trans)
                    } else {
                        0.0
                    };
                    chunk[ca * nc + cb] = sc;
                }
            }
        });

    table
}

/// Score a candidate grid given its per-cell color indices and score tables.
pub fn score_candidate(
    cand: &[usize],
    adj_pairs: &[[usize; 4]],
    score_tables: &[&[f32]], // one table per embedding, each (n_adj, nc, nc)
    nc: usize,
    w: usize,
) -> f32 {
    let n_adj = adj_pairs.len();
    let mut total = 0.0f32;
    for table in score_tables {
        for (ai, &[r1, c1, r2, c2]) in adj_pairs.iter().enumerate() {
            let ca = cand[r1 * w + c1];
            let cb = cand[r2 * w + c2];
            total += table[ai * nc * nc + ca * nc + cb];
        }
    }
    total
}

/// Flat index to per-cell color indices (mixed-radix decomposition).
pub fn flat_to_indices(mut idx: u64, nc: usize, hw: usize, out: &mut [usize]) {
    let nc = nc as u64;
    for i in (0..hw).rev() {
        out[i] = (idx % nc) as usize;
        idx /= nc;
    }
}

/// Result of solving a task.
#[derive(Debug, Clone)]
pub struct SolveResult {
    pub rank: u64,
    pub solved: bool,
    pub n_candidates: u64,
    pub setup_secs: f64,
    pub score_secs: f64,
    pub total_trans: usize,
}

/// Shared helper: compute training lines and transversals for one embedding.
struct EmbeddingTransversals {
    w1: Vec<f32>,
    w2: Vec<f32>,
    jtm: Vec<f32>,   // cleaned, (6 x n_trans) row-major
    n_trans: usize,
}

fn compute_embedding_transversals(
    task: &ArcTask, ei: usize, seed: u64, random_nullspace: bool,
) -> Option<EmbeddingTransversals> {
    let dim = EMB_DIMS[ei];
    let proj_seed = seed.wrapping_mul(31).wrapping_add(ei as u64 * 7919);
    let (w1, w2) = gen_projections(proj_seed, dim);

    let mut all_lines: Vec<[f64; 6]> = Vec::new();
    for pair in &task.train {
        let ph = pair.input.h;
        let pw = pair.input.w;
        let pinp = &pair.input.data;
        let pout = &pair.output.data;
        let mut emb_a = vec![0.0f32; dim];
        let mut emb_b = vec![0.0f32; dim];
        for r in 0..ph {
            for c in 0..pw {
                for d in 0..2 {
                    let r2 = r + if d == 1 { 1 } else { 0 };
                    let c2 = c + if d == 0 { 1 } else { 0 };
                    if r2 >= ph || c2 >= pw { continue; }
                    embeddings::compute_embedding(ei, &mut emb_a, r, c, pinp[r*pw+c], pout[r*pw+c], pinp, pout, ph, pw);
                    embeddings::compute_embedding(ei, &mut emb_b, r2, c2, pinp[r2*pw+c2], pout[r2*pw+c2], pinp, pout, ph, pw);
                    if let Some(line) = plucker::make_line_f64(&emb_a, &emb_b, dim, &w1, &w2) {
                        all_lines.push(line);
                    }
                }
            }
        }
    }

    let mut trans_all: Vec<[f32; 6]> = Vec::new();
    for (p_idx, _) in task.train.iter().enumerate() {
        let trans_seed = seed.wrapping_add(42 + p_idx as u64).wrapping_mul(997).wrapping_add(ei as u64);
        let mut rng = ChaCha8Rng::seed_from_u64(trans_seed);
        let trans = if random_nullspace {
            compute_random_nullspace_transversals(&all_lines, N_TRANS_PER_PAIR, &mut rng)
        } else {
            plucker::compute_transversals(&all_lines, N_TRANS_PER_PAIR, &mut rng)
        };
        trans_all.extend_from_slice(&trans);
    }

    if trans_all.is_empty() { return None; }

    let jtm = plucker::build_jtm(&trans_all);
    let n_trans = trans_all.len();
    let valid = filter_valid_jtm_cols(&jtm, n_trans);
    if valid.is_empty() { return None; }
    let jtm_clean = compact_jtm(&jtm, n_trans, &valid);
    let valid_n = valid.len();

    Some(EmbeddingTransversals { w1, w2, jtm: jtm_clean, n_trans: valid_n })
}

/// Full M0-rust solver with histogram-aware scoring for hist_color.
pub fn solve_m0_rust(task: &ArcTask, seed: u64) -> SolveResult {
    solve_full_pipeline(task, seed, false)
}

/// M1: Same as M0-rust but with random null-space vectors instead of Plücker quadratic.
pub fn solve_m1_full(task: &ArcTask, seed: u64) -> SolveResult {
    solve_full_pipeline(task, seed, true)
}

fn solve_full_pipeline(task: &ArcTask, seed: u64, random_nullspace: bool) -> SolveResult {
    let t0 = std::time::Instant::now();

    let h = task.test_input.h;
    let w = task.test_input.w;
    let hw = h * w;
    let test_inp = &task.test_input.data;

    let used = task::used_colors(task);
    let nc = used.len();
    let adj = task::adjacency_pairs(h, w);

    let mut c2i = vec![None; task::N_COLORS];
    for (i, &c) in used.iter().enumerate() {
        c2i[c as usize] = Some(i);
    }

    // Input histogram (raw counts, not normalized)
    let mut inp_hist = [0.0f32; 10];
    for &v in test_inp {
        if (v as usize) < 10 { inp_hist[v as usize] += 1.0; }
    }

    let mut raw_tables: Vec<Vec<f32>> = Vec::new();
    let mut hist_emb_data: Vec<EmbeddingTransversals> = Vec::new();
    let mut total_trans = 0usize;

    for ei in 0..N_EMBEDDINGS {
        let is_hist = ei == 0; // hist_color
        let Some(et) = compute_embedding_transversals(task, ei, seed, random_nullspace) else {
            continue;
        };
        total_trans += et.n_trans;

        if is_hist {
            hist_emb_data.push(et);
        } else {
            let dim = EMB_DIMS[ei];
            let cell_embs = embeddings::precompute_cell_embeddings(ei, test_inp, test_inp, &used, h, w);
            let table = build_score_table(&cell_embs, dim, &adj, &et.w1, &et.w2, &et.jtm, et.n_trans, nc, h, w);
            raw_tables.push(table);
        }
    }

    // Histogram-aware scoring for hist_color
    let all_hists = gen_all_histograms(hw, nc, MAX_HIST_TABLES + 1);
    let n_hists = all_hists.len();
    let use_hist_tables = !hist_emb_data.is_empty() && n_hists <= MAX_HIST_TABLES;

    let hist_tables: Vec<Vec<f32>> = if use_hist_tables {
        eprintln!("    Building {} histogram tables...", n_hists);
        all_hists.par_iter().map(|hist| {
            let mut combined = vec![0.0f32; adj.len() * nc * nc];
            for et in &hist_emb_data {
                let t = build_hist_score_table(&adj, &used, nc, h, w, test_inp, &inp_hist, hist, &et.w1, &et.w2, &et.jtm, et.n_trans);
                for (i, v) in t.iter().enumerate() { combined[i] += v; }
            }
            combined
        }).collect()
    } else if !hist_emb_data.is_empty() {
        // Fallback: treat hist_color as non-histogram
        eprintln!("    {} histograms -- using placeholder for hist_color", n_hists);
        for et in &hist_emb_data {
            let dim = EMB_DIMS[0];
            let cell_embs = embeddings::precompute_cell_embeddings(0, test_inp, test_inp, &used, h, w);
            let table = build_score_table(&cell_embs, dim, &adj, &et.w1, &et.w2, &et.jtm, et.n_trans, nc, h, w);
            raw_tables.push(table);
        }
        Vec::new()
    } else {
        Vec::new()
    };

    let setup_secs = t0.elapsed().as_secs_f64();
    let t1 = std::time::Instant::now();

    // Correct candidate
    let correct_cand: Option<Vec<usize>> = task.test_output.data.iter()
        .map(|&c| c2i.get(c as usize).copied().flatten()).collect();
    let correct_cand = match correct_cand {
        Some(c) => c,
        None => return SolveResult { rank: u64::MAX, solved: false, n_candidates: 0, setup_secs, score_secs: 0.0, total_trans },
    };

    let table_refs: Vec<&[f32]> = raw_tables.iter().map(|t| t.as_slice()).collect();
    let n_total = (nc as u64).checked_pow(hw as u32).unwrap_or(u64::MAX);

    // Score correct answer
    let mut correct_score = score_candidate(&correct_cand, &adj, &table_refs, nc, w);
    if use_hist_tables {
        correct_score += score_candidate_hist(&correct_cand, &adj, nc, w, &all_hists, &hist_tables);
    }

    let (rank, solved) = if n_total <= 200_000_000 {
        let better: u64 = (0..n_total).into_par_iter().map(|idx| {
            let mut cand = vec![0usize; hw];
            flat_to_indices(idx, nc, hw, &mut cand);
            let mut sc = score_candidate(&cand, &adj, &table_refs, nc, w);
            if use_hist_tables {
                sc += score_candidate_hist(&cand, &adj, nc, w, &all_hists, &hist_tables);
            }
            if sc < correct_score { 1u64 } else { 0u64 }
        }).sum();
        (better + 1, better == 0)
    } else {
        let n_samples = 10_000_000u64;
        let better: u64 = (0..n_samples).into_par_iter().map(|si| {
            let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(si));
            let cand: Vec<usize> = (0..hw).map(|_| rand::Rng::gen_range(&mut rng, 0..nc)).collect();
            let mut sc = score_candidate(&cand, &adj, &table_refs, nc, w);
            if use_hist_tables {
                sc += score_candidate_hist(&cand, &adj, nc, w, &all_hists, &hist_tables);
            }
            if sc < correct_score { 1u64 } else { 0u64 }
        }).sum();
        let est_rank = if better == 0 { 1 } else { (better as f64 / n_samples as f64 * n_total as f64) as u64 };
        (est_rank.max(1), better == 0)
    };

    let score_secs = t1.elapsed().as_secs_f64();
    SolveResult { rank, solved, n_candidates: n_total.min(2_000_000_000), setup_secs, score_secs, total_trans }
}

/// M1: No Plucker constraint — random null-space vector instead of quadratic solve.
pub fn solve_m1(task: &ArcTask, seed: u64) -> SolveResult {
    solve_with_transversal_variant(task, seed, TransversalVariant::RandomNullSpace)
}

/// M2: Score against training lines directly, no transversal extraction.
pub fn solve_m2(task: &ArcTask, seed: u64) -> SolveResult {
    solve_with_reference_lines(task, seed, true) // use J6
}

/// M3: Random projection + plain dot product (no J6, no exterior product).
pub fn solve_m3(task: &ArcTask, seed: u64) -> SolveResult {
    solve_m3_impl(task, seed)
}

/// M4: Cosine similarity on raw embeddings (no projection).
pub fn solve_m4(task: &ArcTask, seed: u64) -> SolveResult {
    solve_m4_impl(task, seed)
}

/// M5: One-hot pairwise scoring.
pub fn solve_m5(task: &ArcTask, seed: u64) -> SolveResult {
    solve_m5_impl(task, seed)
}

/// M6: Color transition frequency counting.
pub fn solve_m6(task: &ArcTask, _seed: u64) -> SolveResult {
    solve_m6_impl(task)
}

/// M7: Per-cell independent mode.
pub fn solve_m7(task: &ArcTask) -> SolveResult {
    let t0 = std::time::Instant::now();
    let h = task.test_input.h;
    let w = task.test_input.w;
    let hw = h * w;

    // For each cell position, count (input_color -> output_color) frequencies
    let mut prediction = vec![0i32; hw];
    for i in 0..hw {
        let in_c = task.test_input.data[i];
        let mut counts = [0u32; 10];
        for pair in &task.train {
            for j in 0..pair.input.data.len() {
                if pair.input.data[j] == in_c {
                    let out_c = pair.output.data[j] as usize;
                    if out_c < 10 {
                        counts[out_c] += 1;
                    }
                }
            }
        }
        prediction[i] = counts
            .iter()
            .enumerate()
            .max_by_key(|(_, &c)| c)
            .map(|(idx, _)| idx as i32)
            .unwrap_or(in_c);
    }

    let solved = prediction == task.test_output.data;
    let secs = t0.elapsed().as_secs_f64();

    SolveResult {
        rank: if solved { 1 } else { u64::MAX },
        solved,
        n_candidates: 1,
        setup_secs: secs,
        score_secs: 0.0,
        total_trans: 0,
    }
}

// --- Internal implementation for M1 ---

enum TransversalVariant {
    Plucker,       // standard solve_p3
    RandomNullSpace, // random combination from null space
}

fn solve_with_transversal_variant(task: &ArcTask, seed: u64, variant: TransversalVariant) -> SolveResult {
    let t0 = std::time::Instant::now();

    let h = task.test_input.h;
    let w = task.test_input.w;
    let hw = h * w;
    let test_inp = &task.test_input.data;
    let used = task::used_colors(task);
    let nc = used.len();
    let adj = task::adjacency_pairs(h, w);

    let mut c2i = vec![None; task::N_COLORS];
    for (i, &c) in used.iter().enumerate() {
        c2i[c as usize] = Some(i);
    }

    let mut all_tables: Vec<Vec<f32>> = Vec::new();
    let mut total_trans = 0usize;

    for ei in 0..N_EMBEDDINGS {
        let dim = EMB_DIMS[ei];
        let proj_seed = seed.wrapping_mul(31).wrapping_add(ei as u64 * 7919);
        let (w1, w2) = gen_projections(proj_seed, dim);

        let mut all_lines: Vec<[f64; 6]> = Vec::new();
        for pair in &task.train {
            let ph = pair.input.h;
            let pw = pair.input.w;
            let pinp = &pair.input.data;
            let pout = &pair.output.data;
            let mut emb_a = vec![0.0f32; dim];
            let mut emb_b = vec![0.0f32; dim];
            for r in 0..ph {
                for c in 0..pw {
                    for d in 0..2 {
                        let r2 = r + if d == 1 { 1 } else { 0 };
                        let c2 = c + if d == 0 { 1 } else { 0 };
                        if r2 >= ph || c2 >= pw { continue; }
                        embeddings::compute_embedding(ei, &mut emb_a, r, c, pinp[r*pw+c], pout[r*pw+c], pinp, pout, ph, pw);
                        embeddings::compute_embedding(ei, &mut emb_b, r2, c2, pinp[r2*pw+c2], pout[r2*pw+c2], pinp, pout, ph, pw);
                        if let Some(line) = plucker::make_line_f64(&emb_a, &emb_b, dim, &w1, &w2) {
                            all_lines.push(line);
                        }
                    }
                }
            }
        }

        // Compute transversals with variant
        let mut trans_all: Vec<[f32; 6]> = Vec::new();
        for (p_idx, _) in task.train.iter().enumerate() {
            let trans_seed = seed.wrapping_add(42 + p_idx as u64).wrapping_mul(997).wrapping_add(ei as u64);
            let mut rng = ChaCha8Rng::seed_from_u64(trans_seed);
            let trans = match variant {
                TransversalVariant::Plucker => plucker::compute_transversals(&all_lines, N_TRANS_PER_PAIR, &mut rng),
                TransversalVariant::RandomNullSpace => compute_random_nullspace_transversals(&all_lines, N_TRANS_PER_PAIR, &mut rng),
            };
            trans_all.extend_from_slice(&trans);
        }

        total_trans += trans_all.len();
        if trans_all.is_empty() { continue; }

        let jtm = plucker::build_jtm(&trans_all);
        let n_trans = trans_all.len();
        let valid_n = filter_valid_jtm_cols(&jtm, n_trans);
        let jtm_clean = compact_jtm(&jtm, n_trans, &valid_n);
        if valid_n.is_empty() { continue; }

        let cell_embs = embeddings::precompute_cell_embeddings(ei, test_inp, test_inp, &used, h, w);
        let table = build_score_table(&cell_embs, dim, &adj, &w1, &w2, &jtm_clean, valid_n.len(), nc, h, w);
        all_tables.push(table);
    }

    let setup_secs = t0.elapsed().as_secs_f64();
    score_and_rank(task, &used, &adj, &all_tables, nc, h, w, hw, seed, setup_secs, total_trans)
}

/// M1 helper: random null-space vector instead of quadratic solve
fn compute_random_nullspace_transversals(
    lines: &[[f64; 6]],
    max_trans: usize,
    rng: &mut impl rand::Rng,
) -> Vec<[f32; 6]> {
    use rand::seq::SliceRandom;

    if lines.len() < 4 {
        return Vec::new();
    }

    let mut result = Vec::new();
    let max_attempts = max_trans * 10;
    let mut indices: Vec<usize> = (0..lines.len()).collect();

    for _ in 0..max_attempts {
        if result.len() >= max_trans { break; }

        indices.shuffle(rng);
        let chosen: Vec<usize> = indices[..4].to_vec();

        let mut a_data = vec![0.0f64; 24];
        for (i, &idx) in chosen.iter().enumerate() {
            let hd = plucker::hodge_dual(&lines[idx]);
            for j in 0..6 { a_data[i * 6 + j] = hd[j]; }
        }

        let a_mat = faer::mat::from_row_major_slice::<f64, usize, usize>(&a_data, 4, 6);
        let svd = a_mat.svd();
        let v = svd.v();
        if v.nrows() < 6 || v.ncols() < 6 { continue; }

        let mut v1 = [0.0f64; 6];
        let mut v2 = [0.0f64; 6];
        for j in 0..6 {
            v1[j] = v[(j, 5)]; // column 5 = smallest SV
            v2[j] = v[(j, 4)]; // column 4 = 2nd smallest
        }

        // Instead of solving quadratic, pick random t
        let t: f64 = rand::Rng::gen_range(rng, -10.0..10.0);
        let mut tt = [0.0f64; 6];
        for k in 0..6 { tt[k] = t * v1[k] + v2[k]; }
        let n = (tt.iter().map(|x| x*x).sum::<f64>()).sqrt();
        if n > 1e-10 {
            let mut f = [0.0f32; 6];
            let mut ok = true;
            for k in 0..6 {
                let v = (tt[k] / n) as f32;
                if !v.is_finite() { ok = false; break; }
                f[k] = v;
            }
            if ok { result.push(f); }
        }
    }
    result
}

// --- M2: Score against training reference lines ---

fn solve_with_reference_lines(task: &ArcTask, seed: u64, use_j6: bool) -> SolveResult {
    let t0 = std::time::Instant::now();

    let h = task.test_input.h;
    let w = task.test_input.w;
    let hw = h * w;
    let test_inp = &task.test_input.data;
    let used = task::used_colors(task);
    let nc = used.len();
    let adj = task::adjacency_pairs(h, w);

    let mut c2i = vec![None; task::N_COLORS];
    for (i, &c) in used.iter().enumerate() {
        c2i[c as usize] = Some(i);
    }

    let mut all_tables: Vec<Vec<f32>> = Vec::new();
    let mut total_trans = 0usize;

    for ei in 0..N_EMBEDDINGS {
        let dim = EMB_DIMS[ei];
        let proj_seed = seed.wrapping_mul(31).wrapping_add(ei as u64 * 7919);
        let (w1, w2) = gen_projections(proj_seed, dim);

        // Collect training lines as reference vectors (f32)
        let mut ref_lines: Vec<[f32; 6]> = Vec::new();
        for pair in &task.train {
            let ph = pair.input.h;
            let pw = pair.input.w;
            let pinp = &pair.input.data;
            let pout = &pair.output.data;
            let mut emb_a = vec![0.0f32; dim];
            let mut emb_b = vec![0.0f32; dim];
            for r in 0..ph {
                for c in 0..pw {
                    for d in 0..2 {
                        let r2 = r + if d == 1 { 1 } else { 0 };
                        let c2 = c + if d == 0 { 1 } else { 0 };
                        if r2 >= ph || c2 >= pw { continue; }
                        embeddings::compute_embedding(ei, &mut emb_a, r, c, pinp[r*pw+c], pout[r*pw+c], pinp, pout, ph, pw);
                        embeddings::compute_embedding(ei, &mut emb_b, r2, c2, pinp[r2*pw+c2], pout[r2*pw+c2], pinp, pout, ph, pw);
                        if let Some(line) = plucker::make_line_f32(&emb_a, &emb_b, dim, &w1, &w2) {
                            ref_lines.push(line);
                        }
                    }
                }
            }
        }

        total_trans += ref_lines.len();
        if ref_lines.is_empty() { continue; }

        // Use ref_lines as "transversals" — build JTm with or without J6
        let jtm = if use_j6 {
            plucker::build_jtm(&ref_lines)
        } else {
            // Plain: jtm[i][t] = ref_lines[t][i] (no J6 transform)
            let n = ref_lines.len();
            let mut jtm = vec![0.0f32; 6 * n];
            for i in 0..6 {
                for (t, rl) in ref_lines.iter().enumerate() {
                    jtm[i * n + t] = rl[i];
                }
            }
            jtm
        };

        let n_ref = ref_lines.len();
        let cell_embs = embeddings::precompute_cell_embeddings(ei, test_inp, test_inp, &used, h, w);
        let table = build_score_table(&cell_embs, dim, &adj, &w1, &w2, &jtm, n_ref, nc, h, w);
        all_tables.push(table);
    }

    let setup_secs = t0.elapsed().as_secs_f64();
    score_and_rank(task, &used, &adj, &all_tables, nc, h, w, hw, seed, setup_secs, total_trans)
}

// --- M3: Random projection + plain dot product ---

fn solve_m3_impl(task: &ArcTask, seed: u64) -> SolveResult {
    let t0 = std::time::Instant::now();

    let h = task.test_input.h;
    let w = task.test_input.w;
    let hw = h * w;
    let test_inp = &task.test_input.data;
    let used = task::used_colors(task);
    let nc = used.len();
    let adj = task::adjacency_pairs(h, w);

    let mut all_tables: Vec<Vec<f32>> = Vec::new();
    let mut total_refs = 0usize;

    for ei in 0..N_EMBEDDINGS {
        let dim = EMB_DIMS[ei];
        let cd = 2 * dim;
        // Single random matrix W ∈ R^{6 x 2d}
        let proj_seed = seed.wrapping_mul(31).wrapping_add(ei as u64 * 7919);
        let mut rng = ChaCha8Rng::seed_from_u64(proj_seed);
        let w_mat: Vec<f32> = (0..6 * cd)
            .map(|_| { let v: f64 = StandardNormal.sample(&mut rng); v as f32 * 0.1 })
            .collect();

        // Collect training reference 6-vectors (plain projection, no exterior product)
        let mut ref_vecs: Vec<[f32; 6]> = Vec::new();
        for pair in &task.train {
            let ph = pair.input.h;
            let pw = pair.input.w;
            let pinp = &pair.input.data;
            let pout = &pair.output.data;
            let mut emb_a = vec![0.0f32; dim];
            let mut emb_b = vec![0.0f32; dim];
            for r in 0..ph {
                for c in 0..pw {
                    for d in 0..2 {
                        let r2 = r + if d == 1 { 1 } else { 0 };
                        let c2 = c + if d == 0 { 1 } else { 0 };
                        if r2 >= ph || c2 >= pw { continue; }
                        embeddings::compute_embedding(ei, &mut emb_a, r, c, pinp[r*pw+c], pout[r*pw+c], pinp, pout, ph, pw);
                        embeddings::compute_embedding(ei, &mut emb_b, r2, c2, pinp[r2*pw+c2], pout[r2*pw+c2], pinp, pout, ph, pw);

                        // Project: v = W @ [ea; eb], normalize
                        if let Some(v) = project_and_normalize(&emb_a, &emb_b, dim, &w_mat) {
                            ref_vecs.push(v);
                        }
                    }
                }
            }
        }

        total_refs += ref_vecs.len();
        if ref_vecs.is_empty() { continue; }

        // Build score table: for each (adj, ca, cb), project test embeddings, dot with all refs
        let n_ref = ref_vecs.len();
        let n_adj = adj.len();
        let cell_embs = embeddings::precompute_cell_embeddings(ei, test_inp, test_inp, &used, h, w);

        let mut table = vec![0.0f32; n_adj * nc * nc];
        table.par_chunks_mut(nc * nc).enumerate().for_each(|(ai, chunk)| {
            let [r1, c1, r2, c2] = adj[ai];
            let idx_a = r1 * w + c1;
            let idx_b = r2 * w + c2;
            for ca in 0..nc {
                let ea = &cell_embs[(idx_a * nc + ca) * dim..(idx_a * nc + ca) * dim + dim];
                for cb in 0..nc {
                    let eb = &cell_embs[(idx_b * nc + cb) * dim..(idx_b * nc + cb) * dim + dim];
                    if let Some(v) = project_and_normalize(ea, eb, dim, &w_mat) {
                        let mut sc = 0.0f32;
                        for rv in &ref_vecs {
                            let dot: f32 = (0..6).map(|k| v[k] * rv[k]).sum();
                            sc += (dot.abs() + 1e-10).ln();
                        }
                        chunk[ca * nc + cb] = sc;
                    }
                }
            }
        });

        all_tables.push(table);
    }

    let setup_secs = t0.elapsed().as_secs_f64();
    score_and_rank(task, &used, &adj, &all_tables, nc, h, w, hw, seed, setup_secs, total_refs)
}

fn project_and_normalize(ea: &[f32], eb: &[f32], dim: usize, w_mat: &[f32]) -> Option<[f32; 6]> {
    let cd = 2 * dim;
    let mut combined = vec![0.0f32; cd];
    combined[..dim].copy_from_slice(&ea[..dim]);
    combined[dim..cd].copy_from_slice(&eb[..dim]);

    let mut v = [0.0f32; 6];
    for i in 0..6 {
        let mut s = 0.0f32;
        for j in 0..cd {
            s += w_mat[i * cd + j] * combined[j];
        }
        v[i] = s;
    }
    let norm2: f32 = v.iter().map(|x| x * x).sum();
    let norm = norm2.sqrt();
    if norm <= 1e-10 { return None; }
    let inv = 1.0 / norm;
    for k in 0..6 { v[k] *= inv; }
    Some(v)
}

// --- M4: Cosine similarity on raw embeddings ---

fn solve_m4_impl(task: &ArcTask, seed: u64) -> SolveResult {
    let t0 = std::time::Instant::now();

    let h = task.test_input.h;
    let w = task.test_input.w;
    let hw = h * w;
    let test_inp = &task.test_input.data;
    let used = task::used_colors(task);
    let nc = used.len();
    let adj = task::adjacency_pairs(h, w);

    let mut all_tables: Vec<Vec<f32>> = Vec::new();
    let mut total_refs = 0usize;

    for ei in 0..N_EMBEDDINGS {
        let dim = EMB_DIMS[ei];
        let cd = 2 * dim;

        // Collect training reference vectors (concatenated embeddings, normalized)
        let mut ref_vecs: Vec<Vec<f32>> = Vec::new();
        for pair in &task.train {
            let ph = pair.input.h;
            let pw = pair.input.w;
            let pinp = &pair.input.data;
            let pout = &pair.output.data;
            let mut emb_a = vec![0.0f32; dim];
            let mut emb_b = vec![0.0f32; dim];
            for r in 0..ph {
                for c in 0..pw {
                    for d in 0..2 {
                        let r2 = r + if d == 1 { 1 } else { 0 };
                        let c2 = c + if d == 0 { 1 } else { 0 };
                        if r2 >= ph || c2 >= pw { continue; }
                        embeddings::compute_embedding(ei, &mut emb_a, r, c, pinp[r*pw+c], pout[r*pw+c], pinp, pout, ph, pw);
                        embeddings::compute_embedding(ei, &mut emb_b, r2, c2, pinp[r2*pw+c2], pout[r2*pw+c2], pinp, pout, ph, pw);

                        let mut combined = vec![0.0f32; cd];
                        combined[..dim].copy_from_slice(&emb_a[..dim]);
                        combined[dim..cd].copy_from_slice(&emb_b[..dim]);
                        let norm: f32 = combined.iter().map(|x| x * x).sum::<f32>().sqrt();
                        if norm > 1e-10 {
                            for v in &mut combined { *v /= norm; }
                            ref_vecs.push(combined);
                        }
                    }
                }
            }
        }

        total_refs += ref_vecs.len();
        if ref_vecs.is_empty() { continue; }

        // Build score table using cosine similarity
        let n_adj = adj.len();
        let cell_embs = embeddings::precompute_cell_embeddings(ei, test_inp, test_inp, &used, h, w);

        let mut table = vec![0.0f32; n_adj * nc * nc];
        table.par_chunks_mut(nc * nc).enumerate().for_each(|(ai, chunk)| {
            let [r1, c1, r2, c2] = adj[ai];
            let idx_a = r1 * w + c1;
            let idx_b = r2 * w + c2;
            for ca in 0..nc {
                let ea = &cell_embs[(idx_a * nc + ca) * dim..(idx_a * nc + ca) * dim + dim];
                for cb in 0..nc {
                    let eb = &cell_embs[(idx_b * nc + cb) * dim..(idx_b * nc + cb) * dim + dim];

                    let mut combined = vec![0.0f32; cd];
                    combined[..dim].copy_from_slice(&ea[..dim]);
                    combined[dim..cd].copy_from_slice(&eb[..dim]);
                    let norm: f32 = combined.iter().map(|x| x * x).sum::<f32>().sqrt();
                    if norm <= 1e-10 { continue; }
                    let inv = 1.0 / norm;

                    let mut sc = 0.0f32;
                    for rv in &ref_vecs {
                        let dot: f32 = combined.iter().zip(rv.iter()).map(|(a, b)| a * b * inv).sum();
                        sc += dot; // sum of cosine similarities
                    }
                    chunk[ca * nc + cb] = -sc; // negate: higher cosine = better → lower = better
                }
            }
        });

        all_tables.push(table);
    }

    let setup_secs = t0.elapsed().as_secs_f64();
    score_and_rank(task, &used, &adj, &all_tables, nc, h, w, hw, seed, setup_secs, total_refs)
}

// --- M5: One-hot pairwise scoring ---

fn solve_m5_impl(task: &ArcTask, seed: u64) -> SolveResult {
    let t0 = std::time::Instant::now();

    let h = task.test_input.h;
    let w = task.test_input.w;
    let hw = h * w;
    let test_inp = &task.test_input.data;
    let used = task::used_colors(task);
    let nc = used.len();
    let adj = task::adjacency_pairs(h, w);

    // 40-dim one-hot: (in_a[10], out_a[10], in_b[10], out_b[10])
    let oh_dim = 40;

    // Collect training reference vectors
    let mut ref_vecs: Vec<[f32; 40]> = Vec::new();
    for pair in &task.train {
        let ph = pair.input.h;
        let pw = pair.input.w;
        let pinp = &pair.input.data;
        let pout = &pair.output.data;
        for r in 0..ph {
            for c in 0..pw {
                for d in 0..2 {
                    let r2 = r + if d == 1 { 1 } else { 0 };
                    let c2 = c + if d == 0 { 1 } else { 0 };
                    if r2 >= ph || c2 >= pw { continue; }
                    let mut v = [0.0f32; 40];
                    let in_a = pinp[r * pw + c] as usize;
                    let out_a = pout[r * pw + c] as usize;
                    let in_b = pinp[r2 * pw + c2] as usize;
                    let out_b = pout[r2 * pw + c2] as usize;
                    if in_a < 10 { v[in_a] = 1.0; }
                    if out_a < 10 { v[10 + out_a] = 1.0; }
                    if in_b < 10 { v[20 + in_b] = 1.0; }
                    if out_b < 10 { v[30 + out_b] = 1.0; }
                    // Normalize (always has exactly 4 ones, norm = 2)
                    let norm = 2.0f32;
                    for x in &mut v { *x /= norm; }
                    ref_vecs.push(v);
                }
            }
        }
    }

    if ref_vecs.is_empty() {
        return SolveResult {
            rank: u64::MAX, solved: false, n_candidates: 0,
            setup_secs: t0.elapsed().as_secs_f64(), score_secs: 0.0, total_trans: 0,
        };
    }

    // Build score table
    let n_adj = adj.len();
    let mut table = vec![0.0f32; n_adj * nc * nc];
    table.par_chunks_mut(nc * nc).enumerate().for_each(|(ai, chunk)| {
        let [r1, c1, r2, c2] = adj[ai];
        let in_a = test_inp[r1 * w + c1] as usize;
        let in_b = test_inp[r2 * w + c2] as usize;
        for ca in 0..nc {
            let out_a = used[ca] as usize;
            for cb in 0..nc {
                let out_b = used[cb] as usize;
                let mut v = [0.0f32; 40];
                if in_a < 10 { v[in_a] = 1.0; }
                if out_a < 10 { v[10 + out_a] = 1.0; }
                if in_b < 10 { v[20 + in_b] = 1.0; }
                if out_b < 10 { v[30 + out_b] = 1.0; }
                let norm = 2.0f32;
                // Cosine similarity with each ref
                let mut sc = 0.0f32;
                for rv in &ref_vecs {
                    let dot: f32 = v.iter().zip(rv.iter()).map(|(a, b)| a * b / norm).sum();
                    sc += dot;
                }
                chunk[ca * nc + cb] = -sc; // negate: higher cosine = better → lower = better
            }
        }
    });

    let all_tables = vec![table];
    let setup_secs = t0.elapsed().as_secs_f64();
    score_and_rank(task, &used, &adj, &all_tables, nc, h, w, hw, seed, setup_secs, ref_vecs.len())
}

// --- M6: Color transition frequency ---

fn solve_m6_impl(task: &ArcTask) -> SolveResult {
    let t0 = std::time::Instant::now();

    let h = task.test_input.h;
    let w = task.test_input.w;
    let hw = h * w;
    let test_inp = &task.test_input.data;
    let used = task::used_colors(task);
    let nc = used.len();
    let adj = task::adjacency_pairs(h, w);

    let mut c2i = vec![None; task::N_COLORS];
    for (i, &c) in used.iter().enumerate() {
        c2i[c as usize] = Some(i);
    }

    // Count (in_a, out_a, in_b, out_b) 4-tuple frequencies
    // Key: (in_a_idx, out_a_idx, in_b_idx, out_b_idx)
    let mut counts = std::collections::HashMap::<(usize, usize, usize, usize), u32>::new();
    for pair in &task.train {
        let ph = pair.input.h;
        let pw = pair.input.w;
        let pinp = &pair.input.data;
        let pout = &pair.output.data;
        for r in 0..ph {
            for c in 0..pw {
                for d in 0..2 {
                    let r2 = r + if d == 1 { 1 } else { 0 };
                    let c2 = c + if d == 0 { 1 } else { 0 };
                    if r2 >= ph || c2 >= pw { continue; }
                    let in_a = c2i.get(pinp[r*pw+c] as usize).copied().flatten();
                    let out_a = c2i.get(pout[r*pw+c] as usize).copied().flatten();
                    let in_b = c2i.get(pinp[r2*pw+c2] as usize).copied().flatten();
                    let out_b = c2i.get(pout[r2*pw+c2] as usize).copied().flatten();
                    if let (Some(ia), Some(oa), Some(ib), Some(ob)) = (in_a, out_a, in_b, out_b) {
                        *counts.entry((ia, oa, ib, ob)).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    // Build score table
    let n_adj = adj.len();
    let eps = 1e-10f32;
    let mut table = vec![0.0f32; n_adj * nc * nc];
    for (ai, &[r1, c1, r2, c2]) in adj.iter().enumerate() {
        let in_a = c2i.get(test_inp[r1 * w + c1] as usize).copied().flatten().unwrap_or(0);
        let in_b = c2i.get(test_inp[r2 * w + c2] as usize).copied().flatten().unwrap_or(0);
        for ca in 0..nc {
            for cb in 0..nc {
                let cnt = counts.get(&(in_a, ca, in_b, cb)).copied().unwrap_or(0);
                table[ai * nc * nc + ca * nc + cb] = -(cnt as f32 + eps).ln();
            }
        }
    }

    let all_tables = vec![table];
    let setup_secs = t0.elapsed().as_secs_f64();
    // M6 is deterministic — seed doesn't matter for scoring
    score_and_rank(task, &used, &adj, &all_tables, nc, h, w, hw, 0, setup_secs, 0)
}

// --- Common scoring/ranking ---

fn score_and_rank(
    task: &ArcTask,
    used: &[i32],
    adj: &[[usize; 4]],
    all_tables: &[Vec<f32>],
    nc: usize, h: usize, w: usize, hw: usize,
    seed: u64,
    setup_secs: f64,
    total_trans: usize,
) -> SolveResult {
    let t1 = std::time::Instant::now();

    let mut c2i = vec![None; task::N_COLORS];
    for (i, &c) in used.iter().enumerate() {
        c2i[c as usize] = Some(i);
    }

    let correct_cand: Option<Vec<usize>> = task
        .test_output.data.iter()
        .map(|&c| c2i.get(c as usize).copied().flatten())
        .collect();

    let correct_cand = match correct_cand {
        Some(c) => c,
        None => {
            return SolveResult {
                rank: u64::MAX, solved: false, n_candidates: 0,
                setup_secs, score_secs: 0.0, total_trans,
            };
        }
    };

    let table_refs: Vec<&[f32]> = all_tables.iter().map(|t| t.as_slice()).collect();
    let n_total = (nc as u64).checked_pow(hw as u32).unwrap_or(u64::MAX);
    let correct_score = score_candidate(&correct_cand, adj, &table_refs, nc, w);

    // For M5/M6: higher score = better (cosine sim / log counts)
    // For M0-M3: lower score = better (log of near-zero incidence)
    // We use the same convention as upstream: count how many score LOWER than correct
    // This works correctly because the score tables are built consistently within each method

    let (rank, solved) = if n_total <= 200_000_000 {
        let better: u64 = (0..n_total)
            .into_par_iter()
            .map(|idx| {
                let mut cand = vec![0usize; hw];
                flat_to_indices(idx, nc, hw, &mut cand);
                let sc = score_candidate(&cand, adj, &table_refs, nc, w);
                if sc < correct_score { 1u64 } else { 0u64 }
            })
            .sum();
        (better + 1, better == 0)
    } else {
        let n_samples = 10_000_000u64;
        let better: u64 = (0..n_samples)
            .into_par_iter()
            .map(|si| {
                let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(si).wrapping_mul(6971));
                let cand: Vec<usize> = (0..hw)
                    .map(|_| rand::Rng::gen_range(&mut rng, 0..nc))
                    .collect();
                let sc = score_candidate(&cand, adj, &table_refs, nc, w);
                if sc < correct_score { 1u64 } else { 0u64 }
            })
            .sum();
        let est_rank = if better == 0 { 1 } else { (better as f64 / n_samples as f64 * n_total as f64) as u64 };
        (est_rank.max(1), better == 0)
    };

    let score_secs = t1.elapsed().as_secs_f64();
    SolveResult {
        rank, solved, n_candidates: n_total.min(2_000_000_000),
        setup_secs, score_secs, total_trans,
    }
}

// --- Helpers ---

fn filter_valid_jtm_cols(jtm: &[f32], n_trans: usize) -> Vec<usize> {
    (0..n_trans)
        .filter(|&t| (0..6).all(|k| jtm[k * n_trans + t].is_finite()))
        .collect()
}

fn compact_jtm(jtm: &[f32], n_trans: usize, valid: &[usize]) -> Vec<f32> {
    let vn = valid.len();
    if vn == n_trans {
        return jtm.to_vec();
    }
    let mut new = vec![0.0f32; 6 * vn];
    for (new_t, &old_t) in valid.iter().enumerate() {
        for k in 0..6 {
            new[k * vn + new_t] = jtm[k * n_trans + old_t];
        }
    }
    new
}
