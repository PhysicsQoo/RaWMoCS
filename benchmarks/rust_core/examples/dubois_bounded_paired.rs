//! Source-pinned certified Dubois20 native UNSAT-vs-UNKNOWN research.
//! No unbounded solve: each independent process receives a fixed node cap.
//! SATLIB contains the Boolean CNF; rational literal weights are synthetic.

use rawmocs_benchmarks::dubois_xor_certificate::verify_dubois20_unsat;
use rawmocs_benchmarks::satlib_provenance::fixture;
use rawmocs_core::{
    solve_with_canonical_mul_research, solve_with_limits, CanonicalMulResearchError, Gaussian,
    Limits, PreparedCanonicalProblem, Report, SolveError,
};
use std::process::Command;
use std::time::Instant;

const NODES: [u64; 3] = [1, 32, 256];
const CACHE_CAPS: [usize; 2] = [0, 10_000];
const MODES: [&str; 3] = ["baseline", "gated", "prepared"];

fn memory_kib(name: &str) -> Result<u64, String> {
    let source = std::fs::read_to_string("/proc/self/status")
        .map_err(|e| format!("Linux memory evidence unavailable: {e}"))?;
    let line = source
        .lines()
        .find(|line| line.starts_with(name))
        .ok_or_else(|| format!("missing {name}"))?;
    let tokens: Vec<_> = line.split_whitespace().collect();
    if tokens.len() != 3 || tokens[0] != name || tokens[2] != "kB" {
        return Err(format!("invalid Linux memory record: {line}"));
    }
    tokens[1].parse::<u64>().map_err(|e| e.to_string())
}

fn verdict(result: Result<Report, SolveError>) -> Result<&'static str, String> {
    match result {
        Err(SolveError::NodeBudgetExceeded) => Ok("UNKNOWN_NODE_BUDGET"),
        Err(SolveError::DepthBudgetExceeded) => Ok("UNKNOWN_DEPTH_BUDGET"),
        Ok(report) => {
            if report.satisfiable || report.count != Gaussian::zero() {
                return Err("independently proven Dubois20 UNSAT was incorrectly reported".into());
            }
            Ok("PROVEN_UNSAT")
        }
    }
}

fn worker(mode: &str, nodes: u64, cap: usize) -> Result<(), String> {
    let certificate = verify_dubois20_unsat().map_err(|e| format!("source proof: {e}"))?;
    if certificate.variables != 60
        || certificate.xor_equations != 40
        || certificate.rhs_xor != 1
    {
        return Err("source certificate no longer proves original UNSAT".into());
    }

    let original = fixture("satlib_dubois20_unverified");
    let limits = Limits {
        nodes,
        depth: 128,
        cache_entries: cap,
    };
    let rss_before = memory_kib("VmRSS:")?;
    let started = Instant::now();
    let mut setup_ns = 0_u128;
    let mut retained = 0_usize;
    let mut prepared = None;
    let result = match mode {
        "baseline" => solve_with_limits(&original, limits),
        "gated" => {
            solve_with_canonical_mul_research(&original, limits).map_err(|e| match e {
                CanonicalMulResearchError::Solve(error) => error,
                CanonicalMulResearchError::ZeroDenominator { .. } => {
                    panic!("source-pinned synthetic weights have valid denominators")
                }
            })
        }
        "prepared" => {
            let setup_started = Instant::now();
            let value = PreparedCanonicalProblem::prepare(&original)
                .map_err(|e| format!("unexpected research input error: {e:?}"))?;
            setup_ns = setup_started.elapsed().as_nanos();
            retained = value.estimated_retained_payload_bytes();
            let result = value.solve_with_limits(limits);
            prepared = Some(value);
            result
        }
        _ => return Err("unrecognized fixed research mode".into()),
    };
    let elapsed_ns = started.elapsed().as_nanos();
    let outcome = verdict(result)?;
    let rss_live = memory_kib("VmRSS:")?;
    let high_water = memory_kib("VmHWM:")?;
    std::hint::black_box(&prepared);
    println!(
        "RESULT\t{mode}\t{nodes}\t{cap}\t{outcome}\t{elapsed_ns}\t{setup_ns}\t{rss_before}\t{rss_live}\t{high_water}\t{retained}"
    );
    Ok(())
}

fn parent() -> Result<(), String> {
    let binary = std::env::current_exe().map_err(|e| e.to_string())?;
    for nodes in NODES {
        for cap in CACHE_CAPS {
            let mut by_mode = Vec::new();
            let mut consistent_outcome = None;
            for round in 0..3_usize {
                let order = match round {
                    0 => MODES,
                    1 => ["prepared", "baseline", "gated"],
                    _ => ["gated", "prepared", "baseline"],
                };
                for mode in order {
                    let output = Command::new(&binary)
                        .args(["--worker", mode, &nodes.to_string(), &cap.to_string()])
                        .output()
                        .map_err(|e| format!("isolated child launch failed: {e}"))?;
                    if !output.status.success() {
                        return Err(format!(
                            "worker {mode}/{nodes}/{cap} failed: {}",
                            String::from_utf8_lossy(&output.stderr)
                        ));
                    }
                    let line = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
                    let fields: Vec<_> = line.trim().split('\t').collect();
                    if fields.len() != 11
                        || fields[0] != "RESULT"
                        || fields[1] != mode
                        || fields[2] != nodes.to_string()
                        || fields[3] != cap.to_string()
                    {
                        return Err(format!("unexpected worker output: {line}"));
                    }
                    let classification = fields[4];
                    if let Some(expected) = &consistent_outcome {
                        if expected != classification {
                            return Err(format!(
                                "cross-mode/round SAT-vs-UNKNOWN mismatch nodes={nodes} cap={cap}"
                            ));
                        }
                    } else {
                        consistent_outcome = Some(classification.to_owned());
                    }
                    let wall_ns = fields[5].parse::<u128>().map_err(|e| e.to_string())?;
                    let setup_ns = fields[6].parse::<u128>().map_err(|e| e.to_string())?;
                    let live_rss = fields[8].parse::<u64>().map_err(|e| e.to_string())?;
                    let peak_rss = fields[9].parse::<u64>().map_err(|e| e.to_string())?;
                    let payload = fields[10].parse::<usize>().map_err(|e| e.to_string())?;
                    println!(
                        "PAIRED\t{nodes}\t{cap}\t{round}\t{mode}\t{classification}\t{wall_ns}\t{setup_ns}\t{live_rss}\t{peak_rss}\t{payload}"
                    );
                    by_mode.push((mode.to_owned(), wall_ns));
                }
            }
            let median = |mode: &str| -> u128 {
                let mut runs: Vec<u128> = by_mode
                    .iter()
                    .filter(|run| run.0 == mode)
                    .map(|run| run.1)
                    .collect();
                runs.sort_unstable();
                runs[1]
            };
            println!(
                "SUMMARY\t{nodes}\t{cap}\t{}\tbaseline_ns={}\tgated_ns={}\tprepared_ns={}",
                consistent_outcome.as_deref().unwrap_or("UNKNOWN_NOT_RUN"),
                median("baseline"),
                median("gated"),
                median("prepared")
            );
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let result = if args.len() == 1 {
        parent()
    } else if args.len() == 5 && args[1] == "--worker" {
        match (args[3].parse::<u64>(), args[4].parse::<usize>()) {
            (Ok(nodes), Ok(cap)) if NODES.contains(&nodes) && CACHE_CAPS.contains(&cap) => {
                worker(&args[2], nodes, cap)
            }
            _ => Err("worker requires predeclared budget and cache cap".into()),
        }
    } else {
        Err("invalid arguments".into())
    };
    if let Err(error) = result {
        eprintln!("FAIL: {error}");
        std::process::exit(1);
    }
}
