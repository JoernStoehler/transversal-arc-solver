use crate::task::{Grid, N_COLORS};

pub const N_EMBEDDINGS: usize = 8;
pub const EMB_DIMS: [usize; N_EMBEDDINGS] = [30, 20, 22, 42, 44, 42, 24, 26];
pub const EMB_NAMES: [&str; N_EMBEDDINGS] = [
    "hist_color", "color_only", "pos_color", "all",
    "row_feat", "col_feat", "color_count", "diagonal",
];

fn one_hot(out: &mut [f32], idx: i32) {
    for v in out[..N_COLORS].iter_mut() {
        *v = 0.0;
    }
    if idx >= 0 && (idx as usize) < N_COLORS {
        out[idx as usize] = 1.0;
    }
}

fn count_val(arr: &[i32], v: i32) -> usize {
    arr.iter().filter(|&&x| x == v).count()
}

fn count_unique(arr: &[i32]) -> usize {
    let mut seen = [false; N_COLORS];
    for &v in arr {
        if (v as usize) < N_COLORS {
            seen[v as usize] = true;
        }
    }
    seen.iter().filter(|&&s| s).count()
}

/// Compute embedding for a cell at (r, c) with given input/output colors.
/// `inp` and `out_grid` are the full grid data (row-major).
/// Returns the dimension written.
pub fn compute_embedding(
    emb_type: usize,
    out: &mut [f32],
    r: usize,
    c: usize,
    in_c: i32,
    out_c: i32,
    inp: &[i32],
    out_grid: &[i32],
    h: usize,
    w: usize,
) -> usize {
    match emb_type {
        0 => emb_hist_color(out, r, c, in_c, out_c, inp, out_grid, h, w),
        1 => emb_color_only(out, in_c, out_c),
        2 => emb_pos_color(out, r, c, in_c, out_c, h, w),
        3 => emb_all(out, r, c, in_c, out_c, inp, out_grid, h, w),
        4 => emb_row_features(out, r, c, in_c, out_c, inp, out_grid, h, w),
        5 => emb_col_features(out, r, c, in_c, out_c, inp, out_grid, h, w),
        6 => emb_color_count(out, r, c, in_c, out_c, inp, out_grid, h, w),
        7 => emb_diagonal(out, r, c, in_c, out_c, h, w),
        _ => panic!("invalid embedding type"),
    }
}

fn emb_hist_color(
    out: &mut [f32], _r: usize, _c: usize, in_c: i32, out_c: i32,
    inp: &[i32], out_grid: &[i32], _h: usize, _w: usize,
) -> usize {
    let sz = inp.len();
    let inv_sz = 1.0 / (sz.max(1) as f32);
    one_hot(out, in_c);
    one_hot(&mut out[10..], out_c);
    for i in 0..N_COLORS {
        let cnt_out = count_val(out_grid, i as i32);
        let cnt_inp = count_val(inp, i as i32);
        out[20 + i] = (cnt_out as f32 - cnt_inp as f32) * inv_sz;
    }
    30
}

fn emb_color_only(out: &mut [f32], in_c: i32, out_c: i32) -> usize {
    one_hot(out, in_c);
    one_hot(&mut out[10..], out_c);
    20
}

fn emb_pos_color(
    out: &mut [f32], r: usize, c: usize, in_c: i32, out_c: i32,
    h: usize, w: usize,
) -> usize {
    out[0] = if h > 1 { r as f32 / (h - 1) as f32 } else { 0.0 };
    out[1] = if w > 1 { c as f32 / (w - 1) as f32 } else { 0.0 };
    one_hot(&mut out[2..], in_c);
    one_hot(&mut out[12..], out_c);
    22
}

fn emb_all(
    out: &mut [f32], r: usize, c: usize, in_c: i32, out_c: i32,
    inp: &[i32], out_grid: &[i32], h: usize, w: usize,
) -> usize {
    let sz = inp.len();
    let inv_sz = 1.0 / (sz.max(1) as f32);
    out[0] = if h > 1 { r as f32 / (h - 1) as f32 } else { 0.0 };
    out[1] = if w > 1 { c as f32 / (w - 1) as f32 } else { 0.0 };
    one_hot(&mut out[2..], in_c);
    one_hot(&mut out[12..], out_c);
    for i in 0..N_COLORS {
        out[22 + i] = count_val(inp, i as i32) as f32 * inv_sz;
    }
    for i in 0..N_COLORS {
        out[32 + i] = count_val(out_grid, i as i32) as f32 * inv_sz;
    }
    42
}

fn emb_row_features(
    out: &mut [f32], r: usize, _c: usize, in_c: i32, out_c: i32,
    inp: &[i32], out_grid: &[i32], _h: usize, w: usize,
) -> usize {
    one_hot(out, in_c);
    one_hot(&mut out[10..], out_c);
    let in_row = &inp[r * w..(r + 1) * w];
    let out_row = &out_grid[r * w..(r + 1) * w];
    let inv_w = if w > 0 { 1.0 / w as f32 } else { 0.0 };
    for i in 0..N_COLORS {
        out[20 + i] = count_val(in_row, i as i32) as f32 * inv_w;
    }
    let in_unique = count_unique(in_row);
    out[30] = if in_unique == 1 { 1.0 } else { 0.0 };
    out[31] = if w > 0 { in_unique as f32 / w as f32 } else { 0.0 };
    for i in 0..N_COLORS {
        out[32 + i] = count_val(out_row, i as i32) as f32 * inv_w;
    }
    let out_unique = count_unique(out_row);
    out[42] = if out_unique == 1 { 1.0 } else { 0.0 };
    out[43] = if w > 0 { out_unique as f32 / w as f32 } else { 0.0 };
    44
}

fn emb_col_features(
    out: &mut [f32], _r: usize, c: usize, in_c: i32, out_c: i32,
    inp: &[i32], out_grid: &[i32], h: usize, w: usize,
) -> usize {
    one_hot(out, in_c);
    one_hot(&mut out[10..], out_c);
    let inv_h = if h > 0 { 1.0 / h as f32 } else { 0.0 };
    let in_col: Vec<i32> = (0..h).map(|i| inp[i * w + c]).collect();
    for i in 0..N_COLORS {
        out[20 + i] = count_val(&in_col, i as i32) as f32 * inv_h;
    }
    out[30] = if count_unique(&in_col) == 1 { 1.0 } else { 0.0 };
    let out_col: Vec<i32> = (0..h).map(|i| out_grid[i * w + c]).collect();
    for i in 0..N_COLORS {
        out[31 + i] = count_val(&out_col, i as i32) as f32 * inv_h;
    }
    out[41] = if count_unique(&out_col) == 1 { 1.0 } else { 0.0 };
    42
}

fn emb_color_count(
    out: &mut [f32], _r: usize, _c: usize, in_c: i32, out_c: i32,
    inp: &[i32], out_grid: &[i32], _h: usize, _w: usize,
) -> usize {
    let sz = inp.len();
    let inv_sz = 1.0 / (sz.max(1) as f32);
    one_hot(out, in_c);
    one_hot(&mut out[10..], out_c);
    out[20] = count_val(inp, in_c) as f32 * inv_sz;
    out[21] = count_val(out_grid, out_c) as f32 * inv_sz;
    let mut in_mode = 0i32;
    let mut out_mode = 0i32;
    let mut in_max = 0;
    let mut out_max = 0;
    for i in 0..N_COLORS as i32 {
        let ci = count_val(inp, i);
        let co = count_val(out_grid, i);
        if ci > in_max {
            in_max = ci;
            in_mode = i;
        }
        if co > out_max {
            out_max = co;
            out_mode = i;
        }
    }
    out[22] = if in_c == in_mode { 1.0 } else { 0.0 };
    out[23] = if out_c == out_mode { 1.0 } else { 0.0 };
    24
}

fn emb_diagonal(
    out: &mut [f32], r: usize, c: usize, in_c: i32, out_c: i32,
    h: usize, w: usize,
) -> usize {
    one_hot(out, in_c);
    one_hot(&mut out[10..], out_c);
    out[20] = if h > 1 { r as f32 / (h - 1) as f32 } else { 0.0 };
    out[21] = if w > 1 { c as f32 / (w - 1) as f32 } else { 0.0 };
    let max_hw = h.max(w);
    let min_hw = h.min(w);
    out[22] = (r as f32 - c as f32 + max_hw as f32) / (2.0 * max_hw as f32);
    let denom = (h + w) as f32 - 2.0 + 1e-6;
    out[23] = (r + c) as f32 / denom;
    out[24] = if r == c { 1.0 } else { 0.0 };
    out[25] = if r + c == min_hw - 1 { 1.0 } else { 0.0 };
    26
}

/// Precompute embeddings for all (position, candidate_color) combos.
/// Returns a flat array of shape (h*w, nc, dim).
pub fn precompute_cell_embeddings(
    emb_type: usize,
    inp: &[i32],
    out_grid: &[i32],
    used_colors: &[i32],
    h: usize,
    w: usize,
) -> Vec<f32> {
    let dim = EMB_DIMS[emb_type];
    let nc = used_colors.len();
    let mut result = vec![0.0f32; h * w * nc * dim];

    for r in 0..h {
        for c in 0..w {
            let pos = r * w + c;
            let in_c = inp[pos];
            for (ci, &out_c) in used_colors.iter().enumerate() {
                let offset = (pos * nc + ci) * dim;
                compute_embedding(
                    emb_type,
                    &mut result[offset..offset + dim],
                    r, c, in_c, out_c, inp, out_grid, h, w,
                );
            }
        }
    }
    result
}
