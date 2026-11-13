//! QOO-219: native source-pinned SATLIB Full WMC prepared-weight lifetime study.
//!
//! This is a separate process-isolated research executable. The CNF bytes are
//! original SATLIB examples, but complex weights are synthetic research data.
//! No default solver, cache policy or QuPRS/GPMC component is modified.

use rawmocs_benchmarks::satlib_provenance::{fixture, independent_full_wmc};
use rawmocs_core::{
    solve_with_canonical_mul_research, solve_with_limits, Gaussian, Limits,
    PreparedCanonicalProblem,
};
use std::process::Command;
use std::time::Instant;

const CASES: [&str; 2] = ["satlib_uf20_01", "satlib_uf20_02"];
const REPEATS: [usize; 4] = [1, 4, 16, 64];

fn memory_kib(field: &str) -> Result<u64, String> {
    let status = std::fs::read_to_string("/proc/self/status")
        .map_err(|e| format!("Linux memory evidence unavailable: {e}"))?;
    let line = status
        .lines()
        .find(|line| line.starts_with(field))
        .ok_or_else(|| format!("missing {field}"))?;
    let tokens: Vec<_> = line.split_whitespace().collect();
    if tokens.len() != 3 || tokens[0] != field || tokens[2] != "kB" {
        return Err(format!("unexpected /proc/self/status record: {line}"));
    }
    tokens[1].parse::<u64>().map_err(|e| e.to_string())
}

fn canonical_fields(result: &Gaussian) -> bool {
    use num_rational::BigRational;
    for ratio in [&result.real, &result.imag] {
        let c = BigRational::new(ratio.numer().clone(), ratio.denom().clone());
        if ratio.numer() != c.numer() || ratio.denom() != c.denom() {
            return false;
        }
    }
    true
}

fn worker(name: &str, mode: &str, cap: usize, repeats: usize) -> Result<(), String> {
    let problem = fixture(name);
    // Independent exhaustively checked SAT truth table and schoolbook exact
    // WMC are prepared before timing. They are identical across all modes.
    let (expected_sat, expected_count) = independent_full_wmc(name, &problem);
    let limits = Limits {
        cache_entries: cap,
        ..Limits::default()
    };
    let rss_before = memory_kib("VmRSS:")?;
    let start = Instant::now();
    let mut setup_ns = 0_u128;
    let mut retained = 0_usize;
    let mut prepared = None;
    let mut last = None;
    match mode {
        "baseline" => {
            for _ in 0..repeats {
                last = Some(solve_with_limits(&problem, limits).map_err(|e| e.to_string())?);
            }
        }
        "gated" => {
            for _ in 0..repeats {
                last = Some(
                    solve_with_canonical_mul_research(&problem, limits)
                        .map_err(|e| format!("{e:?}"))?,
                );
            }
        }
        "prepared" => {
            let setup = Instant::now();
            let value =
                PreparedCanonicalProblem::prepare(&problem).map_err(|e| format!("{e:?}"))?;
            setup_ns = setup.elapsed().as_nanos();
            retained = value.estimated_retained_payload_bytes();
            for _ in 0..repeats {
                last = Some(value.solve_with_limits(limits).map_err(|e| e.to_string())?);
            }
            prepared = Some(value);
        }
        _ => return Err("unknown fixed experiment mode".into()),
    }
    let elapsed_ns = start.elapsed().as_nanos();
    let report = last.ok_or("no solves executed")?;
    if report.satisfiable != expected_sat || report.count != expected_count {
        return Err(format!(
            "exact SAT/WMC disagreement: {name}/{mode}/{cap}/{repeats}"
        ));
    }
    if mode != "baseline" && !canonical_fields(&report.count) {
        return Err(format!(
            "noncanonical output fields: {name}/{mode}/{cap}/{repeats}"
        ));
    }
    // The prepared object is STILL ALIVE while recording live RSS and VmHWM.
    let rss_live = memory_kib("VmRSS:")?;
    let peak = memory_kib("VmHWM:")?;
    std::hint::black_box(&prepared);
    println!(
        "RESULT\t{name}\t{mode}\t{cap}\t{repeats}\t{elapsed_ns}\t{setup_ns}\t{peak}\t{rss_before}\t{rss_live}\t{retained}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        report.satisfiable,
        report.count.real.numer(),
        report.count.real.denom(),
        report.count.imag.numer(),
        report.count.imag.denom(),
        report.stats.nodes,
        report.stats.cache_hits,
        report.stats.decompositions,
    );
    Ok(())
}

fn parent() -> Result<(), String> {
    let binary = std::env::current_exe().map_err(|e| e.to_string())?;
    for name in CASES {
        for cap in [0_usize, 10_000_usize] {
            for n in REPEATS {
                let mut records = Vec::new();
                for round in 0..3 {
                    let modes = match round {
                        1 => ["prepared", "gated", "baseline"],
                        2 => ["gated", "baseline", "prepared"],
                        _ => ["baseline", "prepared", "gated"],
                    };
                    let mut within_round = Vec::new();
                    for mode in modes {
                        let result = Command::new(&binary)
                            .args(["--worker", name, mode, &cap.to_string(), &n.to_string()])
                            .output()
                            .map_err(|e| format!("process launch failure: {e}"))?;
                        if !result.status.success() {
                            return Err(format!(
                                "{name}/{mode}/{cap}/{n} child failure: {}",
                                String::from_utf8_lossy(&result.stderr)
                            ));
                        }
                        let output = String::from_utf8(result.stdout).map_err(|e| e.to_string())?;
                        let fields: Vec<_> = output.trim().split('\t').collect();
                        if fields.len() != 19
                            || fields[0] != "RESULT"
                            || fields[1] != name
                            || fields[2] != mode
                            || fields[3] != cap.to_string()
                            || fields[4] != n.to_string()
                        {
                            return Err(format!("bad worker record: {output}"));
                        }
                        let elapsed = fields[5].parse::<u128>().map_err(|e| e.to_string())?;
                        let setup = fields[6].parse::<u128>().map_err(|e| e.to_string())?;
                        let peak = fields[7].parse::<u64>().map_err(|e| e.to_string())?;
                        let before = fields[8].parse::<u64>().map_err(|e| e.to_string())?;
                        let live = fields[9].parse::<u64>().map_err(|e| e.to_string())?;
                        let retained = fields[10].parse::<usize>().map_err(|e| e.to_string())?;
                        let witness = fields[11..].join("\t");
                        within_round
                            .push((mode, witness, elapsed, setup, peak, before, live, retained));
                    }
                    if within_round
                        .iter()
                        .any(|record| record.1 != within_round[0].1)
                    {
                        return Err(format!(
                            "SAT/exact field/stats parity failed {name}/{cap}/{n}"
                        ));
                    }
                    for (mode, _, elapsed, setup, peak, before, live, retained) in &within_round {
                        println!(
                            "PAIRED\t{name}\t{cap}\t{n}\t{round}\t{mode}\t{elapsed}\t{setup}\t{peak}\t{before}\t{live}\t{retained}"
                        );
                    }
                    records.extend(within_round.into_iter().map(|x| (x.0, x.2)));
                }
                let median = |mode: &str| {
                    let mut values: Vec<_> = records
                        .iter()
                        .filter(|x| x.0 == mode)
                        .map(|x| x.1)
                        .collect();
                    values.sort_unstable();
                    values[1]
                };
                let b = median("baseline");
                let g = median("gated");
                let p = median("prepared");
                println!(
                    "SUMMARY\t{name}\t{cap}\t{n}\tbaseline_ns={b}\tprepared_ns={p}\tgated_ns={g}\tbaseline_over_prepared={:.5}",
                    b as f64 / p as f64
                );
            }
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = if args.len() == 1 {
        parent()
    } else if args.len() == 6 && args[1] == "--worker" {
        match (args[4].parse::<usize>(), args[5].parse::<usize>()) {
            (Ok(cap), Ok(n)) if [0, 10_000].contains(&cap) && REPEATS.contains(&n) => {
                worker(&args[2], &args[3], cap, n)
            }
            _ => Err("worker requires predeclared cache caps and repetitions".into()),
        }
    } else {
        Err("unexpected command-line arguments".into())
    };
    if let Err(e) = result {
        eprintln!("FAIL: {e}");
        std::process::exit(1);
    }
}
