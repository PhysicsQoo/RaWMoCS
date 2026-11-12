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

// Linux prlimit applies RLIMIT_AS to the worker process. It bounds its
// virtual address space, NOT its resident memory (RSS). Only this research
// executable uses these guards; the library solve() API remains unchanged.
enum GuardedExit {
    Success,
    ResourceFailure(std::process::ExitStatus),
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
            Ok(Some(status)) => return Ok(GuardedExit::ResourceFailure(status)),
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
        GuardedExit::ResourceFailure(status) => Err(format!(
            "{name}/{n}/{cap}: RESOURCE_FAILURE (worker exit {status}); no certified count"
        )),
        GuardedExit::Timeout => Err(format!(
            "{name}/{n}/{cap}: TIMEOUT (worker killed); no certified count"
        )),
    }
}

fn guard_negative_controls(exe: &std::path::Path) -> Result<(), String> {
    // Under RLIMIT_AS=256 MiB the attempt to reserve 1 GiB must fail
    // without allocating that amount of physical memory. Only the worker's
    // explicit allocation-rejection exit code is accepted as evidence.
    match launch_guarded(exe, &["--stress-memory"], 256, Duration::from_secs(3))? {
        GuardedExit::ResourceFailure(status) if status.code() == Some(42) => {
            eprintln!("GUARD_MEMORY_DENIED=PASS STATUS=RESOURCE_FAILURE");
        }
        GuardedExit::ResourceFailure(status) => {
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
        GuardedExit::ResourceFailure(status) => Err(format!(
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

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    match args.as_slice() {
        [_] => suite(),
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
        [_, flag, name, n, cap] if flag == "--worker" => {
            let variables = n.parse::<usize>().map_err(|e| format!("variables: {e}"))?;
            let cache_cap = cap
                .parse::<usize>()
                .map_err(|e| format!("cache cap: {e}"))?;
            worker(name, variables, cache_cap)
        }
        _ => Err("usage: cache_profile [--worker chain|pairs|cache n cache_cap] [--stress-memory|--stress-sleep]".into()),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("CACHE_PROFILE_FAIL: {error}");
        std::process::exit(1);
    }
}
