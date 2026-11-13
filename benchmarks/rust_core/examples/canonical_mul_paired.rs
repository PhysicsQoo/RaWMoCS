//! QOO-214: fixed, same-Linux-runner isolated-process wall-time/RSS cohort.
//! Not an exhaustive benchmark or a hard OS memory limit. All candidate input
//! normalization is measured, and each worker independently checks exact WMC.
use num_rational::BigRational;
use rawmocs_benchmarks::cache_reuse::{independent_exact_wmc, repeated_residuals};
use rawmocs_benchmarks::expensive_reuse::{guarded_cycle_residual, independent_cycle_wmc};
use rawmocs_benchmarks::scalability::{expected, fixture as medium, Family};
use rawmocs_core::{
    solve_with_canonical_mul_research, solve_with_limits, Gaussian, Limits, Problem, Weights,
};
use std::process::Command;
use std::time::Instant;

const CASES: [&str; 7] = [
    "cycle10",
    "repeat8",
    "chain16",
    "pairs16",
    "cancel",
    "unsat",
    "noncanonical",
];

fn raw(n: i64, d: i64) -> BigRational {
    BigRational::new_raw(n.into(), d.into())
}

fn fixture(name: &str) -> Result<(Problem, bool, Gaussian), String> {
    let result = match name {
        "cycle10" => (guarded_cycle_residual(10), true, independent_cycle_wmc(10)),
        "repeat8" => (repeated_residuals(8), true, independent_exact_wmc(8)),
        "chain16" => (
            medium(Family::EqualityChain, 16),
            true,
            expected(Family::EqualityChain, 16),
        ),
        "pairs16" => (
            medium(Family::IndependentPairs, 16),
            true,
            expected(Family::IndependentPairs, 16),
        ),
        "cancel" => (
            Problem::new(
                1,
                vec![],
                vec![Weights::new(Gaussian::integers(1, 0), Gaussian::integers(-1, 0))],
            )
            .expect("fixed cancellation"),
            true,
            Gaussian::zero(),
        ),
        "unsat" => (
            Problem::new(
                1,
                vec![vec![1], vec![-1]],
                vec![Weights::new(Gaussian::one(), Gaussian::one())],
            )
            .expect("fixed UNSAT"),
            false,
            Gaussian::zero(),
        ),
        "noncanonical" => (
            Problem::new(
                1,
                vec![],
                vec![Weights::new(
                    Gaussian::new(raw(2, 4), raw(-3, 6)),
                    Gaussian::new(raw(0, 7), raw(1, -2)),
                )],
            )
            .expect("fixed valid-denominator weights"),
            true,
            Gaussian::new(
                BigRational::new(1.into(), 2.into()),
                BigRational::from_integer((-1).into()),
            ),
        ),
        _ => return Err(format!("unexpected fixed case {name}")),
    };
    Ok(result)
}

fn canonical(g: &Gaussian) -> Gaussian {
    let normalize = |r: &BigRational| BigRational::new(r.numer().clone(), r.denom().clone());
    Gaussian::new(normalize(&g.real), normalize(&g.imag))
}

fn vmhwm_kib() -> Result<u64, String> {
    let status = std::fs::read_to_string("/proc/self/status")
        .map_err(|e| format!("VmHWM unavailable: {e}"))?;
    let line = status.lines().find(|s| s.starts_with("VmHWM:"))
        .ok_or("VmHWM missing")?;
    let parts: Vec<_> = line.split_whitespace().collect();
    if parts.len() != 3 || parts[2] != "kB" {
        return Err(format!("unexpected VmHWM record: {line}"));
    }
    parts[1].parse::<u64>().map_err(|e| e.to_string())
}

fn worker(name: &str, mode: &str, cap: usize) -> Result<(), String> {
    const REPEATS: usize = 12;
    let (problem, expected_sat, expected_count) = fixture(name)?;
    let limits = Limits { cache_entries: cap, ..Limits::default() };
    let start = Instant::now();
    let mut last = None;
    for _ in 0..REPEATS {
        let report = match mode {
            "baseline" => solve_with_limits(&problem, limits).map_err(|e| format!("{e}"))?,
            "gated" => solve_with_canonical_mul_research(&problem, limits)
                .map_err(|e| format!("{e:?}"))?,
            _ => return Err("invalid fixed strategy".into()),
        };
        last = Some(report);
    }
    let nanos = start.elapsed().as_nanos();
    let report = last.ok_or("zero repeats")?;
    if report.satisfiable != expected_sat || canonical(&report.count) != expected_count {
        return Err(format!("independent exact witness mismatch for {name}/{mode}/{cap}"));
    }
    let rss = vmhwm_kib()?;
    println!(
        "RESULT\t{name}\t{mode}\t{cap}\t{nanos}\t{rss}\t{}\t{:?}\t{}\t{}\t{}",
        report.satisfiable,
        canonical(&report.count),
        report.stats.nodes,
        report.stats.cache_hits,
        report.stats.decompositions,
    );
    Ok(())
}

fn run_parent() -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    for name in CASES {
        for cap in [0_usize, 10_000_usize] {
            let mut trials = Vec::new();
            for round in 0..3_usize {
                let order = if round % 2 == 0 {
                    ["baseline", "gated"]
                } else {
                    ["gated", "baseline"]
                };
                let mut observed = Vec::new();
                for mode in order {
                    let output = Command::new(&executable)
                        .args(["--worker", name, mode, &cap.to_string()])
                        .output()
                        .map_err(|e| format!("worker launch failed: {e}"))?;
                    if !output.status.success() {
                        return Err(format!(
                            "worker failed {name}/{mode}/{cap}: {}",
                            String::from_utf8_lossy(&output.stderr)
                        ));
                    }
                    let text = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
                    let values: Vec<_> = text.trim().split('\t').collect();
                    if values.len() != 11 || values[0] != "RESULT"
                        || values[1] != name || values[2] != mode
                        || values[3] != cap.to_string()
                    {
                        return Err(format!("malformed worker record: {text}"));
                    }
                    let duration = values[4].parse::<u128>().map_err(|e| e.to_string())?;
                    let vmhwm = values[5].parse::<u64>().map_err(|e| e.to_string())?;
                    observed.push((mode.to_owned(), duration, vmhwm, values[6..].join("\t")));
                }
                if observed[0].3 != observed[1].3 {
                    return Err(format!("SAT/count/stats parity failed for {name}/{cap}"));
                }
                for (mode, duration, rss, _) in &observed {
                    println!("PAIRED\t{name}\t{cap}\t{round}\t{mode}\t{duration}\t{rss}");
                }
                trials.extend(observed.into_iter().map(|(mode, duration, rss, _)| (mode, duration, rss)));
            }
            let mut base: Vec<_> = trials.iter().filter(|x| x.0 == "baseline").map(|x| x.1).collect();
            let mut gated: Vec<_> = trials.iter().filter(|x| x.0 == "gated").map(|x| x.1).collect();
            base.sort_unstable();
            gated.sort_unstable();
            let ratio = base[1] as f64 / gated[1] as f64;
            println!("SUMMARY\t{name}\t{cap}\tmedian_base_ns={}\tmedian_gated_ns={}\tbase_over_gated={ratio:.5}", base[1], gated[1]);
        }
    }
    Ok(())
}

fn main() {
    let result = {
        let args: Vec<_> = std::env::args().collect();
        if args.len() == 5 && args[1] == "--worker" {
            match args[4].parse::<usize>() {
                Ok(cap) if cap == 0 || cap == 10_000 => worker(&args[2], &args[3], cap),
                _ => Err("worker requires fixed cache cap".into()),
            }
        } else if args.len() == 1 {
            run_parent()
        } else {
            Err("invalid arguments".into())
        }
    };
    if let Err(err) = result {
        eprintln!("FAIL: {err}");
        std::process::exit(1);
    }
}
