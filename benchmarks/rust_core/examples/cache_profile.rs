//! Experimental Linux process-isolated cache RSS and elapsed-time cohort.
//! Each cache cap and CNF shape runs in a fresh process; no hard RSS cap is claimed.
use rawmocs_benchmarks::scalability::{expected, fixture, Family};
use rawmocs_benchmarks::{fixture as synthetic_fixture, Workload};
use rawmocs_core::{
    solve_with_cache_payload_budget, solve_with_limits, CachePayloadBudget, Limits,
};
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

// Linux prlimit applies RLIMIT_AS to the worker process. It bounds its
// virtual address space, NOT its resident memory (RSS). Only this research
// executable uses these guards; the library solve() API remains unchanged.
enum GuardedExit {
    Success,
    Failed(std::process::ExitStatus),
    Timeout,
}

fn launch_guarded(
    exe: &std::path::Path,
    args: &[&str],
    address_space_mib: u64,
    deadline: Duration,
) -> Result<GuardedExit, String> {
    let limit_bytes = address_space_mib
        .checked_mul(1024 * 1024)
        .ok_or_else(|| "virtual address-space limit overflow".to_string())?;
    let mut child = Command::new("prlimit")
        .arg(format!("--as={limit_bytes}"))
        .arg("--")
        .arg(exe)
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Linux prlimit/worker launch failure: {e}"))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(GuardedExit::Success),
            Ok(Some(status)) => return Ok(GuardedExit::Failed(status)),
            Ok(None) if started.elapsed() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(GuardedExit::Timeout);
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("guarded child wait failure: {e}"));
            }
        }
    }
}

fn launch(exe: &std::path::Path, name: &str, n: usize, cap: usize) -> Result<(), String> {
    let n_arg = n.to_string();
    let cap_arg = cap.to_string();
    match launch_guarded(
        exe,
        &["--worker", name, &n_arg, &cap_arg],
        512,
        Duration::from_secs(5),
    )? {
        GuardedExit::Success => Ok(()),
        GuardedExit::Failed(status) => Err(format!(
            "{name}/{n}/{cap}: WORKER_FAILURE (worker exit {status}; reason not inferred); no certified count"
        )),
        GuardedExit::Timeout => Err(format!(
            "{name}/{n}/{cap}: TIMEOUT (worker killed); no certified count"
        )),
    }
}

fn guard_negative_controls(exe: &std::path::Path) -> Result<(), String> {
    // Under RLIMIT_AS=256 MiB the attempt to reserve 1 GiB must fail
    // without allocating that amount of physical memory. Only this worker's
    // explicit allocation-rejection exit 42 proves the negative control;
    // arbitrary nonzero solver exits remain unclassified WORKER_FAILURE.
    match launch_guarded(exe, &["--stress-memory"], 256, Duration::from_secs(3))? {
        GuardedExit::Failed(status) if status.code() == Some(42) => {
            eprintln!("GUARD_MEMORY_DENIED=PASS STATUS=RESOURCE_FAILURE");
        }
        GuardedExit::Failed(status) => {
            return Err(format!("resource control: unexpected exit {status}"));
        }
        GuardedExit::Success => {
            return Err("resource control: allocation unexpectedly admitted".into());
        }
        GuardedExit::Timeout => {
            return Err("resource control: allocation test timed out".into());
        }
    }
    // The parent must actively terminate a sleeping worker and must never
    // treat a deadline breach as an exact WMC / UNSAT result.
    match launch_guarded(exe, &["--stress-sleep"], 256, Duration::from_millis(200))? {
        GuardedExit::Timeout => {
            eprintln!("GUARD_TIMEOUT=PASS STATUS=TIMEOUT");
            Ok(())
        }
        GuardedExit::Success => Err("timeout control unexpectedly succeeded".into()),
        GuardedExit::Failed(status) => Err(format!(
            "timeout control: worker exited prematurely {status}"
        )),
    }
}

fn suite() -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("RSS profile is supported on Linux only".into());
    }
    let exe = std::env::current_exe().map_err(|e| format!("current executable: {e}"))?;
    guard_negative_controls(&exe)?;
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

fn quota_policy(name: &str) -> Result<Option<CachePayloadBudget>, String> {
    match name {
        "legacy" => Ok(None),
        "tight" => Ok(Some(CachePayloadBudget {
            scope_items: 2,
            clause_items: 2,
            literal_items: 2,
            value_bits: 32,
        })),
        "roomy" => Ok(Some(CachePayloadBudget {
            scope_items: 1_024,
            clause_items: 1_024,
            literal_items: 2_048,
            value_bits: 16_384,
        })),
        _ => Err(format!("unknown source-pinned quota policy: {name}")),
    }
}

// Research-only, source-locked quota-vs-cache comparison. The existing
// 28-record cache-cap suite and its CSV format are intentionally unchanged.
fn quota_worker(name: &str, n: usize, policy: &str) -> Result<(), String> {
    let budget = quota_policy(policy)?;
    let (problem, oracle) = if name == "cache" && n == 3 {
        let input = synthetic_fixture(Workload::Cache);
        // Derivation: y and z are forced true, x is free. The expected
        // Gaussian-rational count does not come from a solver invocation.
        let w = input.weights();
        let expected = w[0]
            .positive
            .add(&w[0].negative)
            .mul(&w[1].positive)
            .mul(&w[2].positive);
        (input, expected)
    } else if [16, 32, 64].contains(&n) {
        let shape = family(name)?;
        (fixture(shape, n), expected(shape, n))
    } else {
        return Err("quota profile accepts only source-locked families and widths".into());
    };

    let limits = Limits {
        cache_entries: 10_000,
        ..Limits::default()
    };
    let started = Instant::now();
    let report = match budget {
        None => solve_with_limits(&problem, limits),
        Some(budget) => solve_with_cache_payload_budget(&problem, limits, budget),
    }
    .map_err(|e| format!("{name}/{n}/{policy}: solver failed closed: {e}"))?;
    let elapsed_ns = started.elapsed().as_nanos();
    if !report.satisfiable || report.count != oracle {
        return Err(format!(
            "{name}/{n}/{policy}: exact analytical WMC or SAT mismatch"
        ));
    }
    let stats = &report.stats;
    if let Some(b) = budget {
        if stats.cache_scope_items > b.scope_items
            || stats.cache_clause_items > b.clause_items
            || stats.cache_literal_items > b.literal_items
            || stats.cache_value_bits > b.value_bits
        {
            return Err(format!(
                "{name}/{n}/{policy}: retained logical quota exceeded"
            ));
        }
        if policy == "tight" && stats.cache_payload_rejections == 0 {
            return Err(format!(
                "{name}/{n}/{policy}: tight quota did not reject an insertion"
            ));
        }
        if policy == "roomy" && stats.cache_payload_rejections != 0 {
            return Err(format!(
                "{name}/{n}/{policy}: roomy quota unexpectedly rejected an insertion"
            ));
        }
    } else if stats.cache_payload_rejections != 0 {
        return Err("legacy policy unexpectedly counted quota rejection".into());
    }
    if name == "cache" && policy != "tight" && stats.cache_hits == 0 {
        return Err(format!(
            "{policy}: cache-hit positive control was not exercised"
        ));
    }
    let rss = peak_rss_kib()?;
    println!(
        "{name},{n},{policy},10000,{elapsed_ns},{rss},{},{},{},{},{},{},{},{},{},{}",
        stats.nodes,
        stats.cache_entries,
        stats.cache_hits,
        stats.decompositions,
        stats.cache_scope_items,
        stats.cache_clause_items,
        stats.cache_literal_items,
        stats.cache_value_bits,
        stats.max_cached_value_bits,
        stats.cache_payload_rejections
    );
    Ok(())
}

fn launch_quota(exe: &std::path::Path, name: &str, n: usize, policy: &str) -> Result<(), String> {
    let width = n.to_string();
    match launch_guarded(
        exe,
        &["--quota-worker", name, &width, policy],
        512,
        Duration::from_secs(5),
    )? {
        GuardedExit::Success => Ok(()),
        GuardedExit::Failed(status) => Err(format!(
            "{name}/{n}/{policy}: WORKER_FAILURE ({status}); no certified WMC result"
        )),
        GuardedExit::Timeout => Err(format!(
            "{name}/{n}/{policy}: TIMEOUT (terminated); no certified WMC result"
        )),
    }
}

fn quota_suite() -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("isolated quota RSS profile supports Linux only".into());
    }
    let exe = std::env::current_exe().map_err(|e| format!("executable path: {e}"))?;
    // Standalone quota runs must independently qualify both OS research
    // negative controls, not rely on the legacy suite running first.
    guard_negative_controls(&exe)?;
    println!(
        "family,variables,policy,cache_cap,solve_wall_ns,process_peak_rss_kib,nodes,cache_entries,cache_hits,decompositions,key_scope_items,key_clauses,key_literals,value_bits,max_value_bits,cache_payload_rejections"
    );
    for name in ["chain", "pairs"] {
        for n in [16, 32, 64] {
            for policy in ["legacy", "tight", "roomy"] {
                launch_quota(&exe, name, n, policy)?;
            }
        }
    }
    for policy in ["legacy", "tight", "roomy"] {
        launch_quota(&exe, "cache", 3, policy)?;
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    match args.as_slice() {
        [_] => suite(),
        [_, flag] if flag == "--quota-suite" => quota_suite(),
        [_, flag] if flag == "--stress-memory" => {
            let mut bytes: Vec<u8> = Vec::new();
            if bytes.try_reserve_exact(1024 * 1024 * 1024).is_err() {
                eprintln!("GUARD_CHILD_ALLOCATION_DENIED");
                std::process::exit(42);
            }
            Err("1 GiB virtual allocation unexpectedly admitted".into())
        }
        [_, flag] if flag == "--stress-sleep" => {
            thread::sleep(Duration::from_secs(5));
            Err("sleep control unexpectedly reached its end".into())
        }
        [_, flag, name, n, policy] if flag == "--quota-worker" => {
            let variables = n.parse::<usize>().map_err(|e| format!("variables: {e}"))?;
            quota_worker(name, variables, policy)
        }
        [_, flag, name, n, cap] if flag == "--worker" => {
            let variables = n.parse::<usize>().map_err(|e| format!("variables: {e}"))?;
            let cache_cap = cap
                .parse::<usize>()
                .map_err(|e| format!("cache cap: {e}"))?;
            worker(name, variables, cache_cap)
        }
        _ => Err("usage: cache_profile [--quota-suite|--quota-worker chain|pairs|cache n legacy|tight|roomy] [--worker chain|pairs|cache n cache_cap] [--stress-memory|--stress-sleep]".into()),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("CACHE_PROFILE_FAIL: {error}");
        std::process::exit(1);
    }
}
