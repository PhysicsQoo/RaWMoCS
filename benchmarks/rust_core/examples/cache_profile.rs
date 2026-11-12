//! Experimental Linux process-isolated cache RSS and elapsed-time cohort.
//! Each cache cap and CNF shape runs in a fresh process; no hard RSS cap is claimed.
use rawmocs_benchmarks::scalability::{expected, fixture, Family};
use rawmocs_benchmarks::{fixture as synthetic_fixture, Workload};
use rawmocs_core::{solve_with_limits, Limits};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn peak_rss_kib() -> Result<u64, String> {
    let status = std::fs::read_to_string("/proc/self/status")
        .map_err(|e| format!("Linux VmHWM unavailable: {e}"))?;
    let value = status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .ok_or_else(|| "Linux /proc/self/status has no VmHWM".to_string())?;
    let mut tokens = value.split_whitespace();
    let amount = tokens
        .next()
        .ok_or_else(|| "missing VmHWM count".to_string())?
        .parse::<u64>()
        .map_err(|e| format!("malformed VmHWM: {e}"))?;
    if tokens.next() != Some("kB") {
        return Err("unexpected VmHWM unit; expected kB".into());
    }
    Ok(amount)
}

fn family(name: &str) -> Result<Family, String> {
    match name {
        "chain" => Ok(Family::EqualityChain),
        "pairs" => Ok(Family::IndependentPairs),
        _ => Err(format!("unknown fixed family: {name}")),
    }
}

fn worker(name: &str, n: usize, cap: usize) -> Result<(), String> {
    if ![0, 1, 8, 10_000].contains(&cap) {
        return Err("profile accepts only the predeclared cache caps".into());
    }
    let (input, oracle) = if name == "cache" && n == 3 {
        // Independently simplify the cache-hit fixture:
        // (x|y)&(!x|y)&(x|z)&(!x|z) forces y and z true, x free.
        let input = synthetic_fixture(Workload::Cache);
        let w = input.weights();
        let oracle = w[0]
            .positive
            .add(&w[0].negative)
            .mul(&w[1].positive)
            .mul(&w[2].positive);
        (input, oracle)
    } else if [16, 32, 64].contains(&n) {
        let family = family(name)?;
        (fixture(family, n), expected(family, n))
    } else {
        return Err("profile accepts only the declared CNF shapes and widths".into());
    };
    let started = Instant::now();
    let result = solve_with_limits(
        &input,
        Limits {
            cache_entries: cap,
            ..Limits::default()
        },
    )
    .map_err(|e| format!("{name}/{n}/{cap}: solver failed closed: {e}"))?;
    let wall_ns = started.elapsed().as_nanos();
    if !result.satisfiable || result.count != oracle {
        return Err(format!(
            "{name}/{n}/{cap}: exact analytical Full WMC or SAT mismatch"
        ));
    }
    let hwm_kib = peak_rss_kib()?;
    let s = &result.stats;
    if name == "cache" && cap == 10_000 && s.cache_hits == 0 {
        return Err("expected source-locked repeated residual cache hits".into());
    }
    println!(
        "{name},{n},{cap},{wall_ns},{hwm_kib},{},{},{},{},{},{},{},{},{}",
        s.nodes,
        s.cache_entries,
        s.cache_hits,
        s.decompositions,
        s.cache_scope_items,
        s.cache_clause_items,
        s.cache_literal_items,
        s.cache_value_bits,
        s.max_cached_value_bits
    );
    Ok(())
}

fn launch(exe: &std::path::Path, name: &str, n: usize, cap: usize) -> Result<(), String> {
    let n_arg = n.to_string();
    let cap_arg = cap.to_string();
    let mut child = Command::new(exe)
        .args(["--worker", name, &n_arg, &cap_arg])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("could not start isolated worker: {e}"))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Err(format!("{name}/{n}/{cap}: child failed with {status}"));
                }
                return Ok(());
            }
            Ok(None) if started.elapsed() > Duration::from_secs(5) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{name}/{n}/{cap}: bounded profiling worker timeout"));
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{name}/{n}/{cap}: wait failed: {e}"));
            }
        }
    }
}

fn suite() -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("RSS profile is supported on Linux only".into());
    }
    let exe = std::env::current_exe().map_err(|e| format!("current executable: {e}"))?;
    println!(
        "family,variables,cache_cap,solve_wall_ns,process_peak_rss_kib,nodes,cache_entries,cache_hits,decompositions,key_scope_items,key_clauses,key_literals,value_bits,max_value_bits"
    );
    for name in ["chain", "pairs"] {
        for n in [16, 32, 64] {
            for cap in [0, 1, 8, 10_000] {
                launch(&exe, name, n, cap)?;
            }
        }
    }
    // Essential positive control: the medium chain/pair families have no hits.
    for cap in [0, 1, 8, 10_000] {
        launch(&exe, "cache", 3, cap)?;
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    match args.as_slice() {
        [_] => suite(),
        [_, flag, name, n, cap] if flag == "--worker" => {
            let variables = n.parse::<usize>().map_err(|e| format!("variables: {e}"))?;
            let cache_cap = cap
                .parse::<usize>()
                .map_err(|e| format!("cache cap: {e}"))?;
            worker(name, variables, cache_cap)
        }
        _ => Err("usage: cache_profile [--worker chain|pairs|cache n cache_cap]".into()),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("CACHE_PROFILE_FAIL: {error}");
        std::process::exit(1);
    }
}
