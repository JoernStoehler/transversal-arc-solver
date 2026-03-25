use serde::Deserialize;
use std::path::Path;

pub const N_COLORS: usize = 10;
pub const MAX_GRID_DIM: usize = 30;

#[derive(Debug, Clone, Deserialize)]
pub struct RawTask {
    pub train: Vec<RawPair>,
    pub test: Vec<RawPair>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawPair {
    pub input: Vec<Vec<i32>>,
    pub output: Vec<Vec<i32>>,
}

#[derive(Debug, Clone)]
pub struct Grid {
    pub h: usize,
    pub w: usize,
    pub data: Vec<i32>, // row-major
}

#[derive(Debug, Clone)]
pub struct Pair {
    pub input: Grid,
    pub output: Grid,
}

#[derive(Debug, Clone)]
pub struct ArcTask {
    pub name: String,
    pub train: Vec<Pair>,
    pub test_input: Grid,
    pub test_output: Grid,
}

impl Grid {
    pub fn from_raw(raw: &[Vec<i32>]) -> Self {
        let h = raw.len();
        let w = if h > 0 { raw[0].len() } else { 0 };
        let mut data = Vec::with_capacity(h * w);
        for row in raw {
            data.extend_from_slice(row);
        }
        Grid { h, w, data }
    }

    pub fn get(&self, r: usize, c: usize) -> i32 {
        self.data[r * self.w + c]
    }
}

impl ArcTask {
    pub fn is_same_size(&self) -> bool {
        for p in &self.train {
            if p.input.h != p.output.h || p.input.w != p.output.w {
                return false;
            }
        }
        self.test_input.h == self.test_output.h && self.test_input.w == self.test_output.w
    }
}

pub fn load_task(path: &Path) -> Result<ArcTask, String> {
    let data = std::fs::read_to_string(path).map_err(|e| format!("read: {e}"))?;
    let raw: RawTask = serde_json::from_str(&data).map_err(|e| format!("json: {e}"))?;

    if raw.train.len() < 2 {
        return Err("fewer than 2 training pairs".into());
    }
    if raw.test.is_empty() {
        return Err("no test pairs".into());
    }

    let train: Vec<Pair> = raw
        .train
        .iter()
        .map(|p| Pair {
            input: Grid::from_raw(&p.input),
            output: Grid::from_raw(&p.output),
        })
        .collect();

    let test_input = Grid::from_raw(&raw.test[0].input);
    let test_output = Grid::from_raw(&raw.test[0].output);

    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    Ok(ArcTask {
        name,
        train,
        test_input,
        test_output,
    })
}

/// List all .json task files in a directory, filtered to same-size tasks with >=2 training pairs.
pub fn list_tasks(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .expect("cannot read directory")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "json").unwrap_or(false))
        .collect();
    paths.sort();
    paths
}

/// Determine used colors from training pairs + test input.
pub fn used_colors(task: &ArcTask) -> Vec<i32> {
    let mut seen = [false; N_COLORS];
    for p in &task.train {
        for &v in &p.input.data {
            if (v as usize) < N_COLORS {
                seen[v as usize] = true;
            }
        }
        for &v in &p.output.data {
            if (v as usize) < N_COLORS {
                seen[v as usize] = true;
            }
        }
    }
    for &v in &task.test_input.data {
        if (v as usize) < N_COLORS {
            seen[v as usize] = true;
        }
    }
    (0..N_COLORS as i32).filter(|&i| seen[i as usize]).collect()
}

/// Build adjacency pairs for an HxW grid. Each pair is (r1, c1, r2, c2).
/// Right neighbor first, then down neighbor (matching C code order).
pub fn adjacency_pairs(h: usize, w: usize) -> Vec<[usize; 4]> {
    let mut pairs = Vec::new();
    for r in 0..h {
        for c in 0..w {
            if c + 1 < w {
                pairs.push([r, c, r, c + 1]);
            }
            if r + 1 < h {
                pairs.push([r, c, r + 1, c]);
            }
        }
    }
    pairs
}

/// Map color value -> index in used_colors list. Returns None if color not found.
pub fn color_to_idx(used: &[i32], color: i32) -> Option<usize> {
    used.iter().position(|&c| c == color)
}
