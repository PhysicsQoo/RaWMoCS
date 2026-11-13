//! QOO-222: source-locked Linux isolated-process XOR refutation study.
//! Each worker has RLIMIT_AS (virtual address space), an external deadline,
//! and a bounded default DPLL fallback. This is NOT hard RSS/cgroup enforcement.

use rawmocs_benchmarks::dubois_xor_certificate::verify_dubois20_unsat;
use rawmocs_benchmarks::satlib_provenance::{fixture, independent_full_wmc};
use rawmocs_benchmarks::xor_research::{refute_problem, verify};
use rawmocs_core::{solve_with_limits, Gaussian, Limits, Problem, SolveError};
use std::process::Command;
use std::time::Instant;

const CASES: [&str; 3] = [
    "satlib_uf20_01",
    "satlib_uf20_02",
    "satlib_dubois20_unverified",
];
const CAPS: [usize; 2] = [0, 10_000];
const MODES: [&str; 2] = ["baseline", "xor_guard"];

fn peak_kib() -> Result<u64, String> {
    let contents = std::fs::read_to_string("/proc/self/status").map_err(|e| e.to_string())?;
    let line = contents
        .lines()
        .find(|line| line.starts_with("VmHWM:"))
        .ok_or("Linux VmHWM unavailable")?;
    let fields: Vec<_> = line.split_whitespace().collect();
    if fields.len() != 3 || fields[2] != "kB" {
        return Err(format!("unexpected VmHWM record: {line}"));
    }
    fields[1].parse::<u64>().map_err(|e| e.to_string())
}

fn expected(name: &str, input: &Problem) -> Result<(bool, Gaussian), String> {
    if name == "satlib_dubois20_unverified" {
        let proof = verify_dubois20_unsat().map_err(|e| format!("independent source: {e}"))?;
        if proof.variables != 60 || proof.xor_equations != 40 || proof.rhs_xor != 1 {
            return Err("independent source proof shape changed".into());
        }
        Ok((false, Gaussian::zero()))
    } else {
        Ok(independent_full_wmc(name, input))
    }
}

fn solve_bounded(input: &Problem, cap: usize, name: &str) -> Result<&'static str, String> {
    let nodes = if name == "satlib_dubois20_unverified" {
        256
    } else {
        10_000
    };
    let limits = Limits {
        nodes,
        depth: 128,
        cache_entries: cap,
    };
    let (sat, count) = expected(name, input)?;
    match solve_with_limits(input, limits) {
        Err(SolveError::NodeBudgetExceeded) => Ok("UNKNOWN_NODE_BUDGET"),
        Err(SolveError::DepthBudgetExceeded) => Ok("UNKNOWN_DEPTH_BUDGET"),
        Ok(report) => {
            if report.satisfiable != sat || report.count != count {
                return Err(format!("default DPLL exact SAT/WMC violated: {name}"));
            }
            if sat {
                Ok("PROVEN_SAT_EXACT_WMC")
            } else {
                Ok("PROVEN_UNSAT")
            }
        }
    }
}

fn worker(name: &str, cap: usize, mode: &str) -> Result<(), String> {
    if !CASES.contains(&name) || !CAPS.contains(&cap) || !MODES.contains(&mode) {
        return Err("only source-declared cases, cache caps and modes".into());
    }
    let problem = fixture(name);
    let (sat, _) = expected(name, &problem)?;
    let started = Instant::now();
    let mut witness_size = 0usize;
    let verdict = if mode == "xor_guard" {
        if let Some(proof) = refute_problem(&problem) {
            if !verify(problem.variables(), problem.clauses(), &proof) {
                return Err("unverifiable refutation must fail closed".into());
            }
            if sat {
                return Err("FALSE_UNSAT: certified SAT source labeled UNSAT".into());
            }
            witness_size = proof.equations.len();
            "PROVEN_UNSAT"
        } else {
            solve_bounded(&problem, cap, name)?
        }
    } else {
        solve_bounded(&problem, cap, name)?
    };
    let wall = started.elapsed().as_nanos();
    let peak = peak_kib()?;
    println!("RESULT\t{name}\t{cap}\t{mode}\t{verdict}\t{wall}\t{peak}\t{witness_size}");
    Ok(())
}

fn run() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    for name in CASES {
        for cap in CAPS {
            let mut grouped = Vec::new();
            for round in 0..3usize {
                let order = if round % 2 == 0 {
                    MODES
                } else {
                    ["xor_guard", "baseline"]
                };
                for mode in order {
                    // These *independent* Linux process guards are a deadline
                    // and virtual address-space limit, NOT a cgroup RSS cap.
                    let output = Command::new("timeout")
                        .args(["-k", "1s", "6s", "prlimit", "--as=536870912", "--"])
                        .arg(&exe)
                        .args(["--worker", name, &cap.to_string(), mode])
                        .output()
                        .map_err(|e| format!("guarded worker launch: {e}"))?;
                    if !output.status.success() {
                        return Err(format!(
                            "guarded worker failed or timed out {name}/{cap}/{mode}: exit={}, stderr={}",
                            output.status,
                            String::from_utf8_lossy(&output.stderr)
                        ));
                    }
                    let s = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
                    let f: Vec<_> = s.trim().split('\t').collect();
                    if f.len() != 8
                        || f[0] != "RESULT"
                        || f[1] != name
                        || f[2] != cap.to_string()
                        || f[3] != mode
                    {
                        return Err(format!("invalid worker evidence: {s}"));
                    }
                    let verdict = f[4].to_owned();
                    let ns = f[5].parse::<u128>().map_err(|e| e.to_string())?;
                    let peak = f[6].parse::<u64>().map_err(|e| e.to_string())?;
                    let witness = f[7].parse::<usize>().map_err(|e| e.to_string())?;
                    if name == "satlib_dubois20_unverified" {
                        if mode == "xor_guard" && (verdict != "PROVEN_UNSAT" || witness == 0) {
                            return Err("certified Dubois20 XOR guard did not prove UNSAT".into());
                        }
                        if mode == "baseline"
                            && ![
                                "UNKNOWN_NODE_BUDGET",
                                "UNKNOWN_DEPTH_BUDGET",
                                "PROVEN_UNSAT",
                            ]
                            .contains(&verdict.as_str())
                        {
                            return Err("baseline UNSAT-or-UNKNOWN contract violated".into());
                        }
                    } else if verdict != "PROVEN_SAT_EXACT_WMC" || witness != 0 {
                        return Err("SATLIB exact witness or no-false-XOR guarantee failed".into());
                    }
                    println!("PAIRED\t{name}\t{cap}\t{round}\t{mode}\t{verdict}\t{ns}\t{peak}\t{witness}");
                    grouped.push((mode, ns, verdict));
                }
            }
            let median = |mode: &str| {
                let mut numbers: Vec<_> = grouped
                    .iter()
                    .filter(|item| item.0 == mode)
                    .map(|item| item.1)
                    .collect();
                numbers.sort_unstable();
                numbers[1]
            };
            let baseline = median("baseline");
            let guarded = median("xor_guard");
            let b_verdict = grouped
                .iter()
                .find(|entry| entry.0 == "baseline")
                .ok_or("missing baseline verdict")?
                .2
                .as_str();
            let g_verdict = grouped
                .iter()
                .find(|entry| entry.0 == "xor_guard")
                .ok_or("missing XOR verdict")?
                .2
                .as_str();
            if grouped.iter().any(|entry| {
                entry.2
                    != if entry.0 == "baseline" {
                        b_verdict
                    } else {
                        g_verdict
                    }
            }) {
                return Err("inconsistent repeated verdict for the same mode".into());
            }
            // UNKNOWN versus PROVEN_UNSAT are different completed tasks:
            // do not publish a meaningless "500x speedup" quotient.
            let ratio = if b_verdict == g_verdict {
                format!("{:.4}", baseline as f64 / guarded as f64)
            } else {
                "NA_DIFFERENT_VERDICT".to_owned()
            };
            println!(
                "SUMMARY\t{name}\t{cap}\tbaseline_verdict={b_verdict}\txor_verdict={g_verdict}\tbaseline_ns={baseline}\txor_guard_ns={guarded}\tbaseline_over_guard={ratio}"
            );
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let result = if args.len() == 1 {
        run()
    } else if args.len() == 5 && args[1] == "--worker" {
        match args[3].parse::<usize>() {
            Ok(cap) => worker(&args[2], cap, &args[4]),
            Err(e) => Err(format!("invalid worker cap: {e}")),
        }
    } else {
        Err("unsupported worker arguments".into())
    };
    if let Err(error) = result {
        eprintln!("FAIL: {error}");
        std::process::exit(1);
    }
}
