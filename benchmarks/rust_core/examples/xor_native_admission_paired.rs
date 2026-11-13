//! QOO-224 research: guarded native baseline/strict/relaxed XOR admission.
//! Independent exact oracles are evaluated outside the recorded solve timer.
//! The timer includes XOR extraction, witness replay and fallback DPLL;
//! it excludes external process startup and source-fixture construction.
//! RLIMIT_AS limits virtual address space, not resident memory or cgroup OOM.

use rawmocs_benchmarks::dubois_xor_certificate::verify_dubois20_unsat;
use rawmocs_benchmarks::satlib_provenance::{fixture, independent_full_wmc};
use rawmocs_benchmarks::xor_research::{refute, refute_relaxed, verify};
use rawmocs_core::{solve_with_limits, Gaussian, Limits, Problem, SolveError, Weights};
use std::process::Command;
use std::time::Instant;

const CASES: [&str; 8] = [
    "satlib_uf20_01",
    "satlib_uf20_02",
    "dubois_orig",
    "dubois_dup",
    "xor_opposite",
    "xor_opposite_dup",
    "xor_consistent_sat",
    "xor_nonxor_unsat",
];
const CAPS: [usize; 2] = [0, 10_000];
const MODES: [&str; 3] = ["baseline", "strict", "relaxed"];

fn parity_clause(mask: u8) -> Vec<i32> {
    (0..3)
        .map(|bit| {
            let variable = bit + 1;
            if mask & (1 << bit) != 0 {
                -variable
            } else {
                variable
            }
        })
        .collect()
}

fn synthetic_problem(name: &str) -> Problem {
    let all: Vec<Vec<i32>> = (0u8..8).map(parity_clause).collect();
    let mut clauses = match name {
        "xor_opposite" | "xor_opposite_dup" => all,
        "xor_consistent_sat" | "xor_nonxor_unsat" => (0u8..8)
            .filter(|mask| mask.count_ones() % 2 == 0)
            .map(parity_clause)
            .collect(),
        _ => panic!("unknown synthetic fixture"),
    };
    let variables = if name == "xor_nonxor_unsat" { 4 } else { 3 };
    if name == "xor_opposite_dup" {
        clauses.push(clauses[0].clone());
    }
    if name == "xor_consistent_sat" {
        clauses.push(clauses[0].clone());
    }
    if name == "xor_nonxor_unsat" {
        clauses.push(vec![4]);
        clauses.push(vec![-4]);
    }
    let weights = (0..variables)
        .map(|i| {
            let k = i as i64 + 1;
            Weights::new(
                Gaussian::integers(k, if i % 2 == 0 { 1 } else { -1 }),
                Gaussian::integers(-k, 2),
            )
        })
        .collect();
    Problem::new(variables, clauses, weights).expect("predeclared exact CNF")
}

fn problem(name: &str) -> Problem {
    match name {
        "satlib_uf20_01" | "satlib_uf20_02" => fixture(name),
        "dubois_orig" => fixture("satlib_dubois20_unverified"),
        "dubois_dup" => {
            let source = fixture("satlib_dubois20_unverified");
            let mut clauses = source.clauses().to_vec();
            clauses.push(clauses[0].clone());
            Problem::new(source.variables(), clauses, source.weights().to_vec())
                .expect("original CNF plus repeated implied clause")
        }
        _ => synthetic_problem(name),
    }
}

fn schoolbook_mul(a: &Gaussian, b: &Gaussian) -> Gaussian {
    Gaussian::new(
        &a.real * &b.real - &a.imag * &b.imag,
        &a.real * &b.imag + &a.imag * &b.real,
    )
}

fn synthetic_oracle(input: &Problem) -> (bool, Gaussian) {
    let n = input.variables();
    assert!(n <= 4);
    let mut sat = false;
    let mut count = Gaussian::zero();
    for mask in 0usize..(1usize << n) {
        let holds = input.clauses().iter().all(|clause| {
            clause.iter().any(|&literal| {
                let truth = (mask & (1 << (literal.unsigned_abs() as usize - 1))) != 0;
                if literal > 0 { truth } else { !truth }
            })
        });
        if holds {
            sat = true;
            let mut weight = Gaussian::one();
            for (i, choices) in input.weights().iter().enumerate() {
                let literal_weight = if mask & (1 << i) != 0 {
                    &choices.positive
                } else {
                    &choices.negative
                };
                weight = schoolbook_mul(&weight, literal_weight);
            }
            count = Gaussian::new(&count.real + &weight.real, &count.imag + &weight.imag);
        }
    }
    (sat, count)
}

fn expected(name: &str, input: &Problem) -> Result<(bool, Gaussian), String> {
    match name {
        "satlib_uf20_01" | "satlib_uf20_02" => Ok(independent_full_wmc(name, input)),
        "dubois_orig" | "dubois_dup" => {
            let proof = verify_dubois20_unsat().map_err(|e| format!("source proof: {e}"))?;
            if proof.variables != 60 || proof.xor_equations != 40 || proof.rhs_xor != 1 {
                return Err("source proof shape changed".into());
            }
            if name == "dubois_dup" {
                let original = fixture("satlib_dubois20_unverified");
                if input.clauses().len() != original.clauses().len() + 1
                    || input.clauses()[..original.clauses().len()] != *original.clauses()
                    || input.clauses().last() != original.clauses().first()
                {
                    return Err("duplicate source is not original plus clause".into());
                }
            }
            Ok((false, Gaussian::zero()))
        }
        _ => Ok(synthetic_oracle(input)),
    }
}

fn peak_kib() -> Result<u64, String> {
    let text = std::fs::read_to_string("/proc/self/status").map_err(|e| e.to_string())?;
    let line = text.lines().find(|x| x.starts_with("VmHWM:")).ok_or("no VmHWM")?;
    let fields: Vec<_> = line.split_whitespace().collect();
    if fields.len() != 3 || fields[2] != "kB" {
        return Err("invalid VmHWM schema".into());
    }
    fields[1].parse().map_err(|e| format!("VmHWM: {e}"))
}

fn native_worker(name: &str, cache: usize, mode: &str) -> Result<(), String> {
    if !CASES.contains(&name) || !CAPS.contains(&cache) || !MODES.contains(&mode) {
        return Err("undeclared source/cache/mode".into());
    }
    let input = problem(name);
    let (sat, exact) = expected(name, &input)?;
    let limits = Limits {
        nodes: if name.starts_with("dubois") { 256 } else { 10_000 },
        depth: 128,
        cache_entries: cache,
    };
    let start = Instant::now();
    let mut proof_rows = 0usize;
    let mut fallback = true;
    let witness = match mode {
        "strict" => refute(input.variables(), input.clauses()),
        "relaxed" => refute_relaxed(input.variables(), input.clauses()),
        _ => None,
    };
    let verdict = if let Some(proof) = witness {
        if !verify(input.variables(), input.clauses(), &proof) || sat {
            return Err("false or unreplayable UNSAT certificate".into());
        }
        proof_rows = proof.equations.len();
        fallback = false;
        "PROVEN_UNSAT"
    } else {
        match solve_with_limits(&input, limits) {
            Ok(report) => {
                if report.satisfiable != sat || report.count != exact {
                    return Err(format!("independent exact oracle mismatch {name}/{mode}"));
                }
                if sat { "PROVEN_SAT_EXACT_WMC" } else { "PROVEN_UNSAT" }
            }
            Err(SolveError::NodeBudgetExceeded) => "UNKNOWN_NODE_BUDGET",
            Err(SolveError::DepthBudgetExceeded) => "UNKNOWN_DEPTH_BUDGET",
        }
    };
    let wall_ns = start.elapsed().as_nanos();
    if sat && verdict != "PROVEN_SAT_EXACT_WMC" {
        return Err(format!("SAT oracle not completed: {name}/{mode}/{verdict}"));
    }
    if !sat && name.starts_with("xor_") && verdict != "PROVEN_UNSAT" {
        return Err(format!("small UNSAT oracle not completed: {name}/{mode}/{verdict}"));
    }
    if proof_rows == 0 && !fallback {
        return Err("unobserved nonempty proof".into());
    }
    println!(
        "RESULT\t{name}\t{cache}\t{mode}\t{verdict}\t{wall_ns}\t{}\t{proof_rows}\t{fallback}",
        peak_kib()?
    );
    Ok(())
}

fn run() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    for name in CASES {
        for cap in CAPS {
            let mut measures: Vec<(&str, String, u128)> = Vec::new();
            for round in 0..3 {
                for step in 0..3 {
                    let mode = MODES[(round + step) % 3];
                    let output = Command::new("timeout")
                        .args(["-k", "1s", "6s", "prlimit", "--as=536870912", "--"])
                        .arg(&exe)
                        .args(["--worker", name, &cap.to_string(), mode])
                        .output()
                        .map_err(|e| format!("spawn: {e}"))?;
                    if !output.status.success() {
                        return Err(format!(
                            "bounded worker failed {name}/{cap}/{mode}: exit={} stderr={}",
                            output.status,
                            String::from_utf8_lossy(&output.stderr)
                        ));
                    }
                    let out = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
                    let f: Vec<_> = out.trim().split('\t').collect();
                    if f.len() != 9 || f[0] != "RESULT" || f[1] != name
                        || f[2] != cap.to_string() || f[3] != mode
                    {
                        return Err(format!("invalid worker line: {out}"));
                    }
                    let verdict = f[4].to_string();
                    let nanos = f[5].parse::<u128>().map_err(|e| e.to_string())?;
                    let peak = f[6].parse::<u64>().map_err(|e| e.to_string())?;
                    let proof = f[7].parse::<usize>().map_err(|e| e.to_string())?;
                    let fallback = f[8].parse::<bool>().map_err(|e| e.to_string())?;
                    if nanos == 0 || peak == 0 || (proof > 0) == fallback
                        || (mode == "baseline" && proof != 0)
                    {
                        return Err("inconsistent timing/memory/proof provenance".into());
                    }
                    let independent = expected(name, &problem(name))?;
                    if independent.0 && verdict != "PROVEN_SAT_EXACT_WMC" {
                        return Err("SAT misclassified".into());
                    }
                    if name == "dubois_orig" && mode != "baseline" && (verdict != "PROVEN_UNSAT" || proof == 0) {
                        return Err("original Dubois proof missing".into());
                    }
                    if name == "dubois_dup" && mode == "relaxed" && (verdict != "PROVEN_UNSAT" || proof == 0) {
                        return Err("duplicate Dubois relaxed proof missing".into());
                    }
                    println!(
                        "PAIRED\t{name}\t{cap}\t{round}\t{mode}\t{verdict}\t{nanos}\t{peak}\t{proof}\t{fallback}"
                    );
                    measures.push((mode, verdict, nanos));
                }
            }
            let median = |mode: &str| -> Result<(u128, &str), String> {
                let mut rows: Vec<_> = measures.iter().filter(|x| x.0 == mode).collect();
                if rows.len() != 3 || rows.iter().any(|x| x.1 != rows[0].1) {
                    return Err(format!("inconsistent repeated verdict {name}/{cap}/{mode}"));
                }
                rows.sort_by_key(|x| x.2);
                Ok((rows[1].2, rows[1].1.as_str()))
            };
            let (base_ns, base_v) = median("baseline")?;
            let (strict_ns, strict_v) = median("strict")?;
            let (relaxed_ns, relaxed_v) = median("relaxed")?;
            let ratio = |other_ns: u128, other_v: &str| -> String {
                if base_v != other_v || base_v.starts_with("UNKNOWN") {
                    "NA_DIFFERENT_OR_INCOMPLETE_VERDICT".to_string()
                } else {
                    format!("{:.4}", base_ns as f64 / other_ns as f64)
                }
            };
            println!(
                "SUMMARY\t{name}\t{cap}\tbaseline={base_v}:{base_ns}\tstrict={strict_v}:{strict_ns}\trelaxed={relaxed_v}:{relaxed_ns}\tbase_over_strict={}\tbase_over_relaxed={}",
                ratio(strict_ns, strict_v),
                ratio(relaxed_ns, relaxed_v)
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
            Ok(cap) => native_worker(&args[2], cap, &args[4]),
            Err(e) => Err(format!("bad cache cap: {e}")),
        }
    } else {
        Err("unsupported arguments".into())
    };
    if let Err(err) = result {
        eprintln!("FAIL: {err}");
        std::process::exit(1);
    }
}
