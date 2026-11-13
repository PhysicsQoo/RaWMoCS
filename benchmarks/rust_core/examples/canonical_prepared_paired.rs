//! QOO-218: prepared-once immutable canonical weights amortization cohort.
//! Not an exhaustive benchmark or a hard OS memory limit. All candidate input
//! normalization is measured, and each worker independently checks exact WMC.
//! Baseline/normalized-only/gated comparisons are NOT pure GCD attribution.
use num_rational::BigRational;
use rawmocs_benchmarks::cache_reuse::{independent_exact_wmc, repeated_residuals};
use rawmocs_benchmarks::expensive_reuse::{guarded_cycle_residual, independent_cycle_wmc};
use rawmocs_benchmarks::scalability::{expected, fixture as medium, Family};
use rawmocs_core::{
    solve_with_canonical_mul_research, solve_with_limits, PreparedCanonicalProblem, Gaussian,
    Limits, Problem, Weights,
};
use std::process::Command;
use std::time::Instant;

const CASES: [&str; 13] = [
    "cycle10",
    "repeat8",
    "chain16",
    "pairs16",
    "cancel",
    "unsat",
    "noncanonical",
    // New source-pinned 24/20-variable holdouts were not used in QOO-216.
    "chain24_holdout",
    "pairs24_holdout",
    "forced20_holdout",
    "contradict20_holdout",
    // Newly predeclared unseen holdouts, not used by QOO-217.
    "chain32_holdout",
    "contradict32_holdout",
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
                vec![Weights::new(
                    Gaussian::integers(1, 0),
                    Gaussian::integers(-1, 0),
                )],
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
        "chain24_holdout" => (
            medium(Family::EqualityChain, 24),
            true,
            expected(Family::EqualityChain, 24),
        ),
        "pairs24_holdout" => (
            medium(Family::IndependentPairs, 24),
            true,
            expected(Family::IndependentPairs, 24),
        ),
        "forced20_holdout" => (
            Problem::new(
                20,
                (1_i32..=20).map(|lit| vec![lit]).collect(),
                (0..20)
                    .map(|_| Weights::new(Gaussian::one(), Gaussian::zero()))
                    .collect(),
            )
            .expect("forced all-true SAT formula"),
            true,
            Gaussian::one(),
        ),
        "contradict20_holdout" => (
            Problem::new(
                20,
                vec![vec![1], vec![-1]],
                medium(Family::EqualityChain, 20).weights().to_vec(),
            )
            .expect("unit-contradictory CNF with expensive weights"),
            false,
            Gaussian::zero(),
        ),
        "chain32_holdout" => (
            medium(Family::EqualityChain, 32),
            true,
            expected(Family::EqualityChain, 32),
        ),
        "contradict32_holdout" => (
            Problem::new(
                32,
                vec![vec![1], vec![-1]],
                medium(Family::EqualityChain, 32).weights().to_vec(),
            ).expect("unit-contradictory 32-variable CNF"),
            false,
            Gaussian::zero(),
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
    let line = status
        .lines()
        .find(|s| s.starts_with("VmHWM:"))
        .ok_or("VmHWM missing")?;
    let parts: Vec<_> = line.split_whitespace().collect();
    if parts.len() != 3 || parts[2] != "kB" {
        return Err(format!("unexpected VmHWM record: {line}"));
    }
    parts[1].parse::<u64>().map_err(|e| e.to_string())
}

fn worker(name: &str, mode: &str, cap: usize, repeats: usize) -> Result<(), String> {
    let (problem, expected_sat, expected_count) = fixture(name)?;
    let limits = Limits {
        cache_entries: cap,
        ..Limits::default()
    };
    let start = Instant::now();
    let mut last = None;
    let mut setup_ns = 0_u128;
    match mode {
        "prepared" => {
            let setup_start = Instant::now();
            let prepared = PreparedCanonicalProblem::prepare(&problem)
                .map_err(|e| format!("{e:?}"))?;
            setup_ns = setup_start.elapsed().as_nanos();
            for _ in 0..repeats {
                last = Some(prepared.solve_with_limits(limits).map_err(|e| format!("{e}"))?);
            }
        }
        "baseline" => {
            for _ in 0..repeats {
                last = Some(solve_with_limits(&problem, limits).map_err(|e| format!("{e}"))?);
            }
        }
        "gated" => {
            for _ in 0..repeats {
                last = Some(solve_with_canonical_mul_research(&problem, limits)
                    .map_err(|e| format!("{e:?}"))?);
            }
        }
        _ => return Err("invalid fixed mode".into()),
    }
    let nanos = start.elapsed().as_nanos();
    let report = last.ok_or("zero repeats")?;
    if report.satisfiable != expected_sat || canonical(&report.count) != expected_count {
        return Err(format!("independent exact witness mismatch {name}/{mode}/{cap}/{repeats}"));
    }
    let rss = vmhwm_kib()?;
    println!(
        "RESULT\t{name}\t{mode}\t{cap}\t{repeats}\t{nanos}\t{rss}\t{}\t{:?}\t{}\t{}\t{}\t{setup_ns}",
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
            for repeats in [1_usize, 2_usize, 12_usize, 64_usize] {
                let mut trials = Vec::new();
                for round in 0..3_usize {
                    let order = match round {
                        1 => ["prepared", "gated", "baseline"],
                        2 => ["gated", "baseline", "prepared"],
                        _ => ["baseline", "prepared", "gated"],
                    };
                    let mut observed = Vec::new();
                    for mode in order {
                        let output = Command::new(&executable)
                            .args([
                                "--worker",
                                name,
                                mode,
                                &cap.to_string(),
                                &repeats.to_string(),
                            ])
                            .output()
                            .map_err(|e| format!("worker launch failed: {e}"))?;
                        if !output.status.success() {
                            return Err(format!(
                                "worker failed {name}/{mode}/{cap}/{repeats}: {}",
                                String::from_utf8_lossy(&output.stderr)
                            ));
                        }
                        let text = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
                        let values: Vec<_> = text.trim().split('\t').collect();
                        if values.len() != 13
                            || values[0] != "RESULT"
                            || values[1] != name
                            || values[2] != mode
                            || values[3] != cap.to_string()
                            || values[4] != repeats.to_string()
                        {
                            return Err(format!("malformed worker record: {text}"));
                        }
                        let duration = values[5].parse::<u128>().map_err(|e| e.to_string())?;
                        let vmhwm = values[6].parse::<u64>().map_err(|e| e.to_string())?;
                        let setup = values[12].parse::<u128>().map_err(|e| e.to_string())?;
                        println!("SETUP\t{name}\t{cap}\t{repeats}\t{round}\t{mode}\t{setup}");
                        observed.push((mode.to_owned(), duration, vmhwm, values[7..12].join("\t")));
                    }
                    if observed.iter().any(|row| row.3 != observed[0].3) {
                        return Err(format!(
                            "SAT/count/stats parity failed {name}/{cap}/{repeats}"
                        ));
                    }
                    for (mode, duration, rss, _) in &observed {
                        println!(
                            "PAIRED\t{name}\t{cap}\t{repeats}\t{round}\t{mode}\t{duration}\t{rss}"
                        );
                    }
                    trials.extend(
                        observed
                            .into_iter()
                            .map(|(mode, duration, _, _)| (mode, duration)),
                    );
                }
                let median = |mode: &str| {
                    let mut values: Vec<_> = trials
                        .iter()
                        .filter(|row| row.0 == mode)
                        .map(|row| row.1)
                        .collect();
                    values.sort_unstable();
                    values[1]
                };
                let baseline = median("baseline");
                let prepared = median("prepared");
                let gated = median("gated");
                println!(
                    "SUMMARY\t{name}\t{cap}\t{repeats}\tbaseline_ns={baseline}\tprepared_ns={prepared}\tgated_ns={gated}\tbaseline_over_prepared={:.5}\tbaseline_over_gated={:.5}",
                    baseline as f64 / prepared as f64,
                    baseline as f64 / gated as f64,
                );
            }
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let result = if args.len() == 6 && args[1] == "--worker" {
        match (args[4].parse::<usize>(), args[5].parse::<usize>()) {
            (Ok(cap), Ok(repeats))
                if (cap == 0 || cap == 10_000) && [1, 2, 12, 64].contains(&repeats) =>
            {
                worker(&args[2], &args[3], cap, repeats)
            }
            _ => Err("worker requires fixed cache cap and repeats".into()),
        }
    } else if args.len() == 1 {
        run_parent()
    } else {
        Err("invalid arguments".into())
    };
    if let Err(err) = result {
        eprintln!("FAIL: {err}");
        std::process::exit(1);
    }
}
