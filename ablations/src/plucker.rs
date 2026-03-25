/// Plucker index pairs: (0,1), (0,2), (0,3), (1,2), (1,3), (2,3)
const PL_I: [usize; 6] = [0, 0, 0, 1, 1, 2];
const PL_J: [usize; 6] = [1, 2, 3, 2, 3, 3];

/// Index aliases for the Plucker relation
const PR_AB: usize = 0; // (0,1)
const PR_AC: usize = 1; // (0,2)
const PR_AD: usize = 2; // (0,3)
const PR_BC: usize = 3; // (1,2)
const PR_BD: usize = 4; // (1,3)
const PR_CD: usize = 5; // (2,3)

/// J6 matrix (Hodge star on Plucker coordinates)
pub const J6: [[f64; 6]; 6] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 1.0],
    [0.0, 0.0, 0.0, 0.0, -1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
    [0.0, -1.0, 0.0, 0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
];

fn vec_norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// Plucker relation: p01*p23 - p02*p13 + p03*p12
pub fn plucker_relation(p: &[f64; 6]) -> f64 {
    p[PR_AB] * p[PR_CD] - p[PR_AC] * p[PR_BD] + p[PR_AD] * p[PR_BC]
}

/// Hodge dual: [p5, -p4, p3, p2, -p1, p0]
pub fn hodge_dual(p: &[f64; 6]) -> [f64; 6] {
    [p[5], -p[4], p[3], p[2], -p[1], p[0]]
}

/// Plucker inner product: p . J6 . q
pub fn plucker_inner(p: &[f64; 6], q: &[f64; 6]) -> f64 {
    p[0] * q[5] - p[1] * q[4] + p[2] * q[3] + p[3] * q[2] - p[4] * q[1] + p[5] * q[0]
}

/// Compute Plucker line from two R4 points. Returns None if degenerate.
pub fn line_from_points(a: &[f64; 4], b: &[f64; 4]) -> Option<[f64; 6]> {
    let mut out = [0.0f64; 6];
    for k in 0..6 {
        out[k] = a[PL_I[k]] * b[PL_J[k]] - a[PL_J[k]] * b[PL_I[k]];
    }
    let n = vec_norm(&out);
    if n < 1e-12 {
        return None;
    }
    for k in 0..6 {
        out[k] /= n;
    }
    Some(out)
}

/// Compute Plucker line from embedding pair via projection + exterior product.
/// Uses f32 accumulation to match upstream C code.
pub fn make_line_f32(
    ea: &[f32],
    eb: &[f32],
    dim: usize,
    w1: &[f32], // 4 x 2*dim
    w2: &[f32], // 4 x 2*dim
) -> Option<[f32; 6]> {
    let cd = 2 * dim;
    // Combined = [ea; eb]
    let mut combined = vec![0.0f32; cd];
    combined[..dim].copy_from_slice(&ea[..dim]);
    combined[dim..cd].copy_from_slice(&eb[..dim]);

    // f32 matmul
    let mut p1 = [0.0f32; 4];
    let mut p2 = [0.0f32; 4];
    for i in 0..4 {
        let mut s1 = 0.0f32;
        let mut s2 = 0.0f32;
        for j in 0..cd {
            s1 += w1[i * cd + j] * combined[j];
            s2 += w2[i * cd + j] * combined[j];
        }
        p1[i] = s1;
        p2[i] = s2;
    }

    // Exterior product in f32
    let mut l = [0.0f32; 6];
    for k in 0..6 {
        l[k] = p1[PL_I[k]] * p2[PL_J[k]] - p1[PL_J[k]] * p2[PL_I[k]];
    }

    let norm2: f32 = l.iter().map(|x| x * x).sum();
    let norm = norm2.sqrt();
    if norm <= 1e-10 {
        return None;
    }
    let inv = 1.0 / norm;
    for k in 0..6 {
        l[k] *= inv;
    }
    Some(l)
}

/// Same as make_line_f32 but returns f64 (for transversal computation via SVD).
pub fn make_line_f64(
    ea: &[f32],
    eb: &[f32],
    dim: usize,
    w1: &[f32],
    w2: &[f32],
) -> Option<[f64; 6]> {
    make_line_f32(ea, eb, dim, w1, w2).map(|l| {
        let mut out = [0.0f64; 6];
        for k in 0..6 {
            out[k] = l[k] as f64;
        }
        out
    })
}

/// Solve for transversals given two null-space vectors v1, v2.
/// Returns up to 2 normalized Plucker lines satisfying the Plucker relation.
pub fn solve_p3(v1: &[f64; 6], v2: &[f64; 6]) -> Vec<[f64; 6]> {
    let tol = 1e-10;
    let alpha = plucker_relation(v1);
    let gamma = plucker_relation(v2);
    let beta = (v1[PR_AB] * v2[PR_CD] + v2[PR_AB] * v1[PR_CD])
        - (v1[PR_AC] * v2[PR_BD] + v2[PR_AC] * v1[PR_BD])
        + (v1[PR_AD] * v2[PR_BC] + v2[PR_AD] * v1[PR_BC]);

    let mut solutions = Vec::new();

    let try_sol = |is_v1_only: bool, t_val: f64| -> Option<[f64; 6]> {
        let mut tt = [0.0f64; 6];
        if is_v1_only {
            tt.copy_from_slice(v1);
        } else {
            for k in 0..6 {
                tt[k] = t_val * v1[k] + v2[k];
            }
        }
        let n = vec_norm(&tt);
        if n > tol {
            for k in 0..6 {
                tt[k] /= n;
            }
            Some(tt)
        } else {
            None
        }
    };

    if alpha.abs() < tol {
        if beta.abs() > tol {
            if let Some(s) = try_sol(false, -gamma / beta) { solutions.push(s); }
        }
        if let Some(s) = try_sol(true, 0.0) { solutions.push(s); }
    } else {
        let disc = beta * beta - 4.0 * alpha * gamma;
        if disc >= 0.0 {
            let sq = disc.abs().sqrt();
            if let Some(s) = try_sol(false, (-beta + sq) / (2.0 * alpha)) { solutions.push(s); }
            if solutions.len() < 2 {
                if let Some(s) = try_sol(false, (-beta - sq) / (2.0 * alpha)) { solutions.push(s); }
            }
        }
    }

    // Sort by residual
    solutions.sort_by(|a, b| {
        plucker_relation(a)
            .abs()
            .partial_cmp(&plucker_relation(b).abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    solutions
}

/// Compute transversals from a set of Plucker lines by random 4-line sampling.
/// Uses faer for SVD of 4x6 constraint matrices.
pub fn compute_transversals(
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
        if result.len() >= max_trans {
            break;
        }

        // Choose 4 random lines
        indices.shuffle(rng);
        let chosen: Vec<usize> = indices[..4].to_vec();

        // Build 4x6 constraint matrix (rows = hodge_dual of each line)
        let mut a_data = vec![0.0f64; 24];
        for (i, &idx) in chosen.iter().enumerate() {
            let hd = hodge_dual(&lines[idx]);
            for j in 0..6 {
                a_data[i * 6 + j] = hd[j];
            }
        }

        // Full SVD to get all 6 right singular vectors (need null space)
        let a_mat = faer::mat::from_row_major_slice::<f64, usize, usize>(&a_data, 4, 6);
        let svd = a_mat.svd();
        let v = svd.v();

        // v is 6×6, columns correspond to singular values in descending order
        // Null space = last two columns of V (smallest singular values)
        if v.nrows() < 6 || v.ncols() < 6 {
            continue;
        }

        let mut v1 = [0.0f64; 6];
        let mut v2 = [0.0f64; 6];
        for j in 0..6 {
            v1[j] = v[(j, 5)]; // column 5 = smallest singular value
            v2[j] = v[(j, 4)]; // column 4 = 2nd smallest
        }

        let sols = solve_p3(&v1, &v2);
        for sol in sols {
            if result.len() >= max_trans {
                break;
            }
            let n = vec_norm(&sol);
            let res = plucker_relation(&sol).abs();
            if n > 1e-10 && res < 1e-6 {
                let mut f = [0.0f32; 6];
                let mut ok = true;
                for k in 0..6 {
                    let v = (sol[k] / n) as f32;
                    if !v.is_finite() {
                        ok = false;
                        break;
                    }
                    f[k] = v;
                }
                if ok {
                    result.push(f);
                }
            }
        }
    }
    result
}

/// Build JTm = J6 @ transversals.T, shape (6, n_trans), stored row-major.
pub fn build_jtm(transversals: &[[f32; 6]]) -> Vec<f32> {
    let n_trans = transversals.len();
    let mut jtm = vec![0.0f32; 6 * n_trans];
    for i in 0..6 {
        for (t, trans) in transversals.iter().enumerate() {
            let mut v = 0.0f64;
            for j in 0..6 {
                v += J6[i][j] * trans[j] as f64;
            }
            jtm[i * n_trans + t] = v as f32;
        }
    }
    jtm
}

/// Score a single Plucker line against JTm.
pub fn score_line(l: &[f32; 6], jtm: &[f32], n_trans: usize) -> f32 {
    let mut score = 0.0f32;
    for t in 0..n_trans {
        let mut dot = 0.0f32;
        for k in 0..6 {
            dot += l[k] * jtm[k * n_trans + t];
        }
        dot = dot.clamp(-1e10, 1e10);
        let mut v = (dot.abs() + 1e-10).ln();
        if !v.is_finite() {
            if v.is_nan() {
                v = 0.0;
            } else if v > 0.0 {
                v = 0.0;
            } else {
                v = -100.0;
            }
        }
        score += v;
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plucker_relation_valid_line() {
        // Two points in R^4
        let a = [1.0, 0.0, 0.0, 0.0];
        let b = [0.0, 1.0, 0.0, 0.0];
        let line = line_from_points(&a, &b).unwrap();
        let pr = plucker_relation(&line);
        assert!(pr.abs() < 1e-10, "Plucker relation should be ~0, got {pr}");
    }

    #[test]
    fn test_hodge_dual_involution() {
        let p = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let hd = hodge_dual(&p);
        let hd2 = hodge_dual(&hd);
        for k in 0..6 {
            assert!(
                (hd2[k] - p[k]).abs() < 1e-12,
                "Hodge dual should be an involution"
            );
        }
    }

    #[test]
    fn test_incidence_coplanar_lines() {
        // Two lines in the same plane should be incident
        let a1 = [1.0, 0.0, 0.0, 0.0];
        let b1 = [0.0, 1.0, 0.0, 0.0];
        let a2 = [1.0, 0.0, 0.0, 0.0];
        let b2 = [0.0, 0.0, 1.0, 0.0];
        let l1 = line_from_points(&a1, &b1).unwrap();
        let l2 = line_from_points(&a2, &b2).unwrap();
        let inner = plucker_inner(&l1, &l2);
        assert!(inner.abs() < 1e-10, "Coplanar lines should have plucker_inner ~0, got {inner}");
    }
}
