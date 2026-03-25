mod task;
mod embeddings;
mod plucker;
mod scoring;

use clap::Parser;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(name = "ablations", about = "ARC solver ablation study")]
struct Cli {
    /// Method to run (m0rust, m1, m2, m3, m4, m5, m6, m7, all)
    #[arg(short, long, default_value = "all")]
    method: String,

    /// Random seed or seed range (e.g., "1000" or "1000-1009")
    #[arg(short, long, default_value = "1000")]
    seeds: String,

    /// Run on all tasks in directory
    #[arg(long)]
    all: Option<PathBuf>,

    /// Single task file(s)
    #[arg(long)]
    task: Vec<PathBuf>,

    /// Timeout per task in seconds
    #[arg(long, default_value = "30")]
    timeout: u64,

    /// Output TSV file
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn parse_seeds(s: &str) -> Vec<u64> {
    if let Some((a, b)) = s.split_once('-') {
        let start: u64 = a.parse().expect("invalid seed start");
        let end: u64 = b.parse().expect("invalid seed end");
        (start..=end).collect()
    } else {
        vec![s.parse().expect("invalid seed")]
    }
}

fn methods_from_str(s: &str) -> Vec<String> {
    if s == "all" {
        vec![
            "m0rust".into(), "m1".into(), "m2".into(), "m3".into(),
            "m4".into(), "m5".into(), "m6".into(), "m7".into(),
        ]
    } else {
        s.split(',').map(|s| s.trim().to_lowercase()).collect()
    }
}

fn solve_with_method(method: &str, task: &task::ArcTask, seed: u64) -> scoring::SolveResult {
    match method {
        "m0rust" => scoring::solve_m0_rust(task, seed),
        "m1" => scoring::solve_m1_full(task, seed),
        "m2" => scoring::solve_m2(task, seed),
        "m3" => scoring::solve_m3(task, seed),
        "m4" => scoring::solve_m4(task, seed),
        "m5" => scoring::solve_m5(task, seed),
        "m6" => scoring::solve_m6(task, seed),
        "m7" => scoring::solve_m7(task),
        _ => panic!("Unknown method: {method}"),
    }
}

fn is_deterministic(method: &str) -> bool {
    matches!(method, "m6" | "m7")
}

fn main() {
    let cli = Cli::parse();
    let seeds = parse_seeds(&cli.seeds);
    let methods = methods_from_str(&cli.method);
    let timeout = Duration::from_secs(cli.timeout);

    // Collect task files
    let task_paths: Vec<PathBuf> = if let Some(dir) = &cli.all {
        task::list_tasks(dir)
    } else if !cli.task.is_empty() {
        cli.task.clone()
    } else {
        eprintln!("Specify --all <dir> or --task <file>");
        std::process::exit(1);
    };

    // Load and filter tasks
    let mut tasks: Vec<task::ArcTask> = Vec::new();
    for path in &task_paths {
        match task::load_task(path) {
            Ok(t) if t.is_same_size() && t.train.len() >= 2 => tasks.push(t),
            Ok(_) => {} // not same-size or too few training pairs
            Err(e) => eprintln!("Skip {}: {e}", path.display()),
        }
    }

    eprintln!("Loaded {} same-size tasks", tasks.len());

    // TSV header
    let mut tsv_lines: Vec<String> = vec![
        "method\tseed\tn_tasks\tn_solved\tn_timeout\tsolve_rate\twall_seconds".into()
    ];

    for method in &methods {
        let effective_seeds = if is_deterministic(method) {
            vec![seeds[0]] // run once
        } else {
            seeds.clone()
        };

        for &seed in &effective_seeds {
            let wall_start = Instant::now();
            let mut n_solved = 0u32;
            let mut n_timeout = 0u32;

            for (ti, task) in tasks.iter().enumerate() {
                let t0 = Instant::now();

                // Run solver with hard timeout via thread
                let method_str = method.clone();
                let task_clone = task.clone();
                let timeout_dur = timeout;
                let handle = std::thread::spawn(move || {
                    solve_with_method(&method_str, &task_clone, seed)
                });

                let (result, timed_out) = match handle.join() {
                    Ok(r) => {
                        let elapsed = t0.elapsed();
                        (Some(r), elapsed > timeout)
                    }
                    Err(_) => (None, true),
                };

                if timed_out {
                    n_timeout += 1;
                }

                if let Some(ref r) = result {
                    if r.solved && !timed_out {
                        n_solved += 1;
                    }
                }

                // Per-task output to stderr for progress
                let elapsed = t0.elapsed();
                if let Some(ref r) = result {
                    if r.solved && !timed_out {
                        eprintln!(
                            "  [{}/{}] {} {} seed={}: SOLVED rank 1/{} ({:.1}s)",
                            ti + 1, tasks.len(), method, task.name, seed,
                            r.n_candidates, elapsed.as_secs_f64()
                        );
                    } else {
                        eprintln!(
                            "  [{}/{}] {} {} seed={}: rank {}/{} ({:.1}s){}",
                            ti + 1, tasks.len(), method, task.name, seed,
                            r.rank, r.n_candidates, elapsed.as_secs_f64(),
                            if timed_out { " TIMEOUT" } else { "" }
                        );
                    }
                } else {
                    eprintln!(
                        "  [{}/{}] {} {} seed={}: TIMEOUT ({:.1}s)",
                        ti + 1, tasks.len(), method, task.name, seed,
                        elapsed.as_secs_f64()
                    );
                }
            }

            let wall_secs = wall_start.elapsed().as_secs_f64();
            let n_tasks = tasks.len() as u32;
            let solve_rate = if n_tasks > 0 {
                n_solved as f64 / n_tasks as f64
            } else {
                0.0
            };

            let line = format!(
                "{}\t{}\t{}\t{}\t{}\t{:.4}\t{:.1}",
                method, seed, n_tasks, n_solved, n_timeout, solve_rate, wall_secs
            );
            println!("{line}");
            tsv_lines.push(line);

            eprintln!(
                "=== {method} seed={seed}: {n_solved}/{n_tasks} solved, {n_timeout} timeout, {wall_secs:.1}s ==="
            );
        }
    }

    // Write TSV output
    if let Some(out_path) = &cli.output {
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(out_path, tsv_lines.join("\n") + "\n").expect("write TSV");
        eprintln!("Results written to {}", out_path.display());
    }
}
