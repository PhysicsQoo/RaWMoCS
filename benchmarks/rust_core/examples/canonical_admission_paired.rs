//! QOO-216: predeclared *exploratory* static admission, never production.
//! Native wall-time includes the policy check and the candidate's weight
//! normalization. Same-host trials are fresh processes; no hard RSS cap.
use num_rational::BigRational;
use rawmocs_benchmarks::cache_reuse::{independent_exact_wmc, repeated_residuals};
use rawmocs_benchmarks::expensive_reuse::{guarded_cycle_residual, independent_cycle_wmc};
use rawmocs_benchmarks::scalability::{expected, fixture as medium, Family};
use rawmocs_core::{
    solve_with_canonical_mul_research, solve_with_limits, Gaussian, Limits, Problem, Report,
    Weights,
};
use std::process::Command;
use std::time::Instant;

const CASES: [&str; 15] = [
    "cycle10",
    "repeat8",
    "chain16",
    "pairs16",
    "cancel1",
    "unsat1",
    "noncanonical1",
    "cycle14_holdout",
    "repeat12_holdout",
    "cycle6_holdout",
    "repeat2_holdout",
    "units16_holdout",
    "contradict16_holdout",
    "identity16_holdout",
    "sparse16_holdout",
];

fn ratio(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}

fn gaussian_integer(n: i64) -> Gaussian {
    Gaussian::integers(n, 0)
}

fn dense_weights(n: usize) -> Vec<Weights> {
    (1..=n)
        .map(|i| {
            let k = i as i64;
            Weights::new(
                Gaussian::new(ratio(k % 7 + 1, 11), ratio(k % 5 + 1, 13)),
                Gaussian::new(ratio(k % 9 + 1, 17), ratio(-k % 5 - 1, 19)),
            )
        })
        .collect()
}

fn equality_chain(n: usize) -> Vec<Vec<i32>> {
    let mut clauses = Vec::new();
    for i in 1..n as i32 {
        clauses.push(vec![-i, i + 1]);
        clauses.push(vec![i, -(i + 1)]);
    }
    clauses
}

fn fixture(name: &str) -> Result<(Problem, bool, Gaussian), String> {
    let output = match name {
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
        "cancel1" => (
            Problem::new(
                1,
                vec![],
                vec![Weights::new(gaussian_integer(1), gaussian_integer(-1))],
            )
            .expect("fixed cancellation"),
            true,
            Gaussian::zero(),
        ),
        "unsat1" => (
            Problem::new(
                1,
                vec![vec![1], vec![-1]],
                vec![Weights::new(Gaussian::one(), Gaussian::one())],
            )
            .expect("fixed UNSAT"),
            false,
            Gaussian::zero(),
        ),
        "noncanonical1" => (
            Problem::new(
                1,
                vec![],
                vec![Weights::new(
                    Gaussian::new(
                        BigRational::new_raw(2.into(), 4.into()),
                        BigRational::new_raw((-3).into(), 6.into()),
                    ),
                    Gaussian::new(
                        BigRational::new_raw(0.into(), 7.into()),
                        BigRational::new_raw(1.into(), (-2).into()),
                    ),
                )],
            )
            .expect("valid denominators"),
            true,
            Gaussian::new(ratio(1, 2), gaussian_integer(-1).real),
        ),
        "cycle14_holdout" => (guarded_cycle_residual(14), true, independent_cycle_wmc(14)),
        "repeat12_holdout" => (repeated_residuals(12), true, independent_exact_wmc(12)),
        "cycle6_holdout" => (guarded_cycle_residual(6), true, independent_cycle_wmc(6)),
        "repeat2_holdout" => (repeated_residuals(2), true, independent_exact_wmc(2)),
        "units16_holdout" => {
            let clauses = (1..=16_i32).map(|i| vec![i]).collect();
            let weights = (0..16)
                .map(|_| Weights::new(Gaussian::one(), Gaussian::zero()))
                .collect();
            (
                Problem::new(16, clauses, weights).expect("forced SAT"),
                true,
                Gaussian::one(),
            )
        }
        "contradict16_holdout" => {
            // No unit clauses, but four clauses over x1,x2 are contradictory.
            // Padding ensures variables and clause count look expensive.
            let mut clauses = vec![vec![1, 2], vec![1, -2], vec![-1, 2], vec![-1, -2]];
            for i in 3..16_i32 {
                clauses.push(vec![i, i + 1]);
            }
            (
                Problem::new(16, clauses, dense_weights(16)).expect("contradiction"),
                false,
                Gaussian::zero(),
            )
        }
        "identity16_holdout" => {
            let weights = (0..16)
                .map(|_| Weights::new(Gaussian::one(), Gaussian::one()))
                .collect();
            (
                Problem::new(16, equality_chain(16), weights).expect("identity chain"),
                true,
                gaussian_integer(2),
            )
        }
        "sparse16_holdout" => {
            let weights = (0..16)
                .map(|_| Weights::new(Gaussian::one(), Gaussian::zero()))
                .collect();
            (
                Problem::new(16, equality_chain(16), weights).expect("sparse chain"),
                true,
                Gaussian::one(),
            )
        }
        _ => return Err(format!("undeclared fixture: {name}")),
    };
    Ok(output)
}

/// A deliberately simple, predeclared, allocation-free threshold. Does NOT
/// inspect the solver's SAT outcome or previous timings. A hostile test family
/// is specifically designed to trigger a false positive.
fn admit(problem: &Problem) -> bool {
    problem.variables() >= 8
        && problem.clauses().len() >= problem.variables() / 2
        && problem.clauses().iter().all(|c| c.len() >= 2)
        && problem.weights().iter().any(|w| {
            w.positive.imag.numer().sign() != num_bigint::Sign::NoSign
                && w.negative.imag.numer().sign() != num_bigint::Sign::NoSign
        })
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
        .find(|l| l.starts_with("VmHWM:"))
        .ok_or("VmHWM unavailable")?;
    let fields: Vec<_> = line.split_whitespace().collect();
    if fields.len() != 3 || fields[2] != "kB" {
        return Err(format!("invalid VmHWM: {line}"));
    }
    fields[1].parse::<u64>().map_err(|e| e.to_string())
}

fn run_mode(problem: &Problem, limits: Limits, mode: &str) -> Result<(Report, bool), String> {
    match mode {
        "baseline" => solve_with_limits(problem, limits)
            .map(|r| (r, false))
            .map_err(|e| format!("{e}")),
        "gated" => solve_with_canonical_mul_research(problem, limits)
            .map(|r| (r, true))
            .map_err(|e| format!("{e:?}")),
        "selective" => {
            if admit(problem) {
                solve_with_canonical_mul_research(problem, limits)
                    .map(|r| (r, true))
                    .map_err(|e| format!("{e:?}"))
            } else {
                solve_with_limits(problem, limits)
                    .map(|r| (r, false))
                    .map_err(|e| format!("{e}"))
            }
        }
        _ => Err("unknown mode".into()),
    }
}

fn worker(name: &str, mode: &str, cap: usize) -> Result<(), String> {
    const REPEATS: usize = 12;
    let (p, sat, wmc) = fixture(name)?;
    let limits = Limits {
        cache_entries: cap,
        ..Limits::default()
    };
    let start = Instant::now();
    let mut last = None;
    for _ in 0..REPEATS {
        last = Some(run_mode(&p, limits, mode)?);
    }
    let elapsed = start.elapsed().as_nanos();
    let (report, chosen) = last.ok_or("empty cohort")?;
    if report.satisfiable != sat || canonical(&report.count) != wmc {
        return Err(format!(
            "independent exact witness mismatch: {name}/{mode}/{cap}"
        ));
    }
    println!(
        "RESULT\t{name}\t{mode}\t{cap}\t{elapsed}\t{}\t{chosen}\t{}\t{:?}\t{}\t{}\t{}",
        vmhwm_kib()?,
        report.satisfiable,
        canonical(&report.count),
        report.stats.nodes,
        report.stats.cache_hits,
        report.stats.decompositions,
    );
    Ok(())
}

fn parent() -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    for name in CASES {
        let (problem, _, _) = fixture(name)?;
        let planned = admit(&problem);
        println!("ADMISSION\t{name}\t{planned}");
        for cap in [0_usize, 10_000_usize] {
            let mut times = Vec::new();
            for round in 0..3_usize {
                let modes = match round {
                    1 => ["selective", "baseline", "gated"],
                    2 => ["gated", "selective", "baseline"],
                    _ => ["baseline", "gated", "selective"],
                };
                let mut round_rows = Vec::new();
                for mode in modes {
                    let output = Command::new(&executable)
                        .args(["--worker", name, mode, &cap.to_string()])
                        .output()
                        .map_err(|e| e.to_string())?;
                    if !output.status.success() {
                        return Err(format!(
                            "worker failed {name}/{mode}/{cap}: {}",
                            String::from_utf8_lossy(&output.stderr)
                        ));
                    }
                    let line = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
                    let fields: Vec<_> = line.trim().split('\t').collect();
                    if fields.len() != 12
                        || fields[0] != "RESULT"
                        || fields[1] != name
                        || fields[2] != mode
                        || fields[3] != cap.to_string()
                    {
                        return Err(format!("invalid worker record {line}"));
                    }
                    let ns = fields[4].parse::<u128>().map_err(|e| e.to_string())?;
                    let rss = fields[5].parse::<u64>().map_err(|e| e.to_string())?;
                    let selected = fields[6].parse::<bool>().map_err(|e| e.to_string())?;
                    if selected != (mode == "gated" || mode == "selective" && planned) {
                        return Err(format!("strategy admission mismatch {name}/{mode}"));
                    }
                    round_rows.push((mode, ns, rss, fields[7..].join("\t")));
                }
                if round_rows.iter().any(|row| row.3 != round_rows[0].3) {
                    return Err(format!("SAT/exact count/stats mismatch {name}/{cap}"));
                }
                for (mode, ns, rss, _) in round_rows {
                    println!("PAIRED\t{name}\t{cap}\t{round}\t{mode}\t{ns}\t{rss}");
                    times.push((mode, ns));
                }
            }
            let median = |mode: &str| {
                let mut ns: Vec<_> = times
                    .iter()
                    .filter(|row| row.0 == mode)
                    .map(|row| row.1)
                    .collect();
                ns.sort_unstable();
                ns[1]
            };
            let base = median("baseline");
            let gated = median("gated");
            let selective = median("selective");
            println!("SUMMARY\t{name}\t{cap}\tadmit={planned}\tbase_ns={base}\tgated_ns={gated}\tselective_ns={selective}\tbase_over_selective={:.5}",
                base as f64 / selective as f64);
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let result = if args.len() == 5 && args[1] == "--worker" {
        match args[4].parse::<usize>() {
            Ok(cap) if cap == 0 || cap == 10_000 => worker(&args[2], &args[3], cap),
            _ => Err("invalid cache cap".into()),
        }
    } else if args.len() == 1 {
        parent()
    } else {
        Err("invalid arguments".into())
    };
    if let Err(e) = result {
        eprintln!("FAIL: {e}");
        std::process::exit(1);
    }
}
