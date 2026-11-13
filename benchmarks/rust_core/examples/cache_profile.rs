//! Experimental Linux process-isolated cache RSS and elapsed-time cohort.
//! Each cache cap and CNF shape runs in a fresh process; no hard RSS cap is claimed.
#[cfg(feature = "cache-admission-research")]
use rawmocs_benchmarks::admission_holdout::{
    guarded_graph, independent_exact_wmc as graph_oracle, Graph,
};
#[cfg(feature = "branching-research")]
use rawmocs_benchmarks::branching_holdout::{fixture as branching_fixture, BranchingFamily};
use rawmocs_benchmarks::cache_reuse::{independent_exact_wmc, repeated_residuals};
use rawmocs_benchmarks::expensive_reuse::{guarded_cycle_residual, independent_cycle_wmc};
use rawmocs_benchmarks::scalability::{expected, fixture, Family};
#[cfg(feature = "cache-admission-research")]
use rawmocs_benchmarks::work_proxy_holdout::{independent_wmc, selector_free_cnf, Holdout};
use rawmocs_benchmarks::{fixture as synthetic_fixture, Workload};
use rawmocs_core::{
    solve_with_cache_payload_budget, solve_with_limits, CachePayloadBudget, Limits,
};
#[cfg(feature = "cache-admission-research")]
use rawmocs_core::{solve_with_min_cache_clauses, solve_with_observed_work_ratio};
#[cfg(feature = "branching-research")]
use rawmocs_core::{solve_with_pivot_policy, PivotPolicy};
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

fn repeated_worker(gadgets: usize, policy: &str, trial: usize) -> Result<(), String> {
    if ![4, 8, 16].contains(&gadgets) || trial >= 3 {
        return Err("repeated-residual cohort only accepts 4/8/16 gadgets and trials 0..3".into());
    }
    let input = repeated_residuals(gadgets);
    let oracle = independent_exact_wmc(gadgets);
    let cache_cap = if policy == "off" { 0 } else { 10_000 };
    let budget = if policy == "off" {
        None
    } else {
        quota_policy(policy)?
    };
    let limits = Limits {
        cache_entries: cache_cap,
        ..Limits::default()
    };
    let started = Instant::now();
    let result = match budget {
        Some(b) => solve_with_cache_payload_budget(&input, limits, b),
        None => solve_with_limits(&input, limits),
    }
    .map_err(|e| format!("repeated/{gadgets}/{trial}/{policy}: fail-closed {e}"))?;
    let elapsed_ns = started.elapsed().as_nanos();
    if !result.satisfiable || result.count != oracle {
        return Err(format!(
            "repeated/{gadgets}/{trial}/{policy}: exact oracle or SAT mismatch"
        ));
    }
    let s = &result.stats;
    if let Some(b) = budget {
        if s.cache_scope_items > b.scope_items
            || s.cache_clause_items > b.clause_items
            || s.cache_literal_items > b.literal_items
            || s.cache_value_bits > b.value_bits
        {
            return Err("retained payload exceeded admission quota".into());
        }
    }
    match policy {
        "off" if s.cache_hits != 0 || s.cache_entries != 0 || s.cache_payload_rejections != 0 => {
            return Err("disabled cache unexpectedly retained or reused residual".into());
        }
        "tight" if s.cache_hits == 0 || s.cache_payload_rejections == 0 => {
            return Err("tight cache policy did not exercise hits and rejects".into());
        }
        "legacy" | "roomy" if s.cache_hits < gadgets as u64 || s.cache_payload_rejections != 0 => {
            return Err("unbounded cache policy lost expected reuse or rejected insertion".into());
        }
        _ => {}
    }
    let peak_rss = peak_rss_kib()?;
    println!(
        "{gadgets},{},{trial},{policy},{cache_cap},{elapsed_ns},{peak_rss},{},{},{},{},{},{},{},{},{},{}",
        gadgets * 3,
        s.nodes,
        s.cache_entries,
        s.cache_hits,
        s.decompositions,
        s.cache_scope_items,
        s.cache_clause_items,
        s.cache_literal_items,
        s.cache_value_bits,
        s.max_cached_value_bits,
        s.cache_payload_rejections,
    );
    Ok(())
}

fn launch_repeated(
    exe: &std::path::Path,
    gadgets: usize,
    trial: usize,
    policy: &str,
) -> Result<(), String> {
    let g_arg = gadgets.to_string();
    let trial_arg = trial.to_string();
    match launch_guarded(
        exe,
        &["--repeat-worker", &g_arg, policy, &trial_arg],
        512,
        Duration::from_secs(5),
    )? {
        GuardedExit::Success => Ok(()),
        GuardedExit::Failed(status) => Err(format!(
            "repeat/{gadgets}/{trial}/{policy}: WORKER_FAILURE ({status}); no certified count"
        )),
        GuardedExit::Timeout => Err(format!(
            "repeat/{gadgets}/{trial}/{policy}: TIMEOUT; no certified count"
        )),
    }
}

fn repeated_suite() -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("repeated-residual VmHWM research requires Linux".into());
    }
    let exe = std::env::current_exe().map_err(|e| format!("executable path: {e}"))?;
    guard_negative_controls(&exe)?;
    println!(
        "gadgets,variables,trial,policy,cache_cap,solve_wall_ns,process_peak_rss_kib,nodes,cache_entries,cache_hits,decompositions,key_scope_items,key_clauses,key_literals,value_bits,max_value_bits,cache_payload_rejections"
    );
    // Rotate deterministic policy order to reduce monotonic runner drift.
    // Policy order is declared before measurement, and each call uses a
    // separate OS process under the existing RLIMIT_AS and parent deadline.
    const POLICIES: [&str; 4] = ["off", "legacy", "tight", "roomy"];
    for gadgets in [4, 8, 16] {
        for trial in 0..3 {
            for position in 0..POLICIES.len() {
                let policy = POLICIES[(trial + position) % POLICIES.len()];
                launch_repeated(&exe, gadgets, trial, policy)?;
            }
        }
    }
    Ok(())
}

// A separate source-locked high-cost connected residual cohort.
// Avoid collapsing this study into the 3-variable repeated-gadget CSV.
fn cycle_worker(vertices: usize, policy: &str, trial: usize) -> Result<(), String> {
    if ![6, 10, 14].contains(&vertices) || trial >= 3 {
        return Err("cycle cohort only accepts 6/10/14 vertices and trials 0..3".into());
    }
    let cap = match policy {
        "off" => 0,
        "on" => 10_000,
        _ => return Err("cycle policy must be off or on".into()),
    };
    let input = guarded_cycle_residual(vertices);
    let oracle = independent_cycle_wmc(vertices);
    let limits = Limits {
        cache_entries: cap,
        ..Limits::default()
    };
    let started = Instant::now();
    let result = solve_with_limits(&input, limits)
        .map_err(|e| format!("cycle/{vertices}/{trial}/{policy}: fail-closed {e}"))?;
    let elapsed_ns = started.elapsed().as_nanos();
    if !result.satisfiable || result.count != oracle {
        return Err(format!(
            "cycle/{vertices}/{trial}/{policy}: exact Full WMC oracle or SAT mismatch"
        ));
    }
    let stats = &result.stats;
    if policy == "off" && (stats.cache_hits != 0 || stats.cache_entries != 0) {
        return Err("cycle off unexpectedly retained or reused a residual".into());
    }
    if policy == "on" && stats.cache_hits == 0 {
        return Err("cycle on failed to reuse any nontrivial residual".into());
    }
    let peak = peak_rss_kib()?;
    println!(
        "{vertices},{},{trial},{policy},{elapsed_ns},{peak},{},{},{},{},{},{}",
        vertices + 1,
        stats.nodes,
        stats.cache_entries,
        stats.cache_hits,
        stats.decompositions,
        stats.cache_clause_items,
        stats.cache_literal_items,
    );
    Ok(())
}

fn launch_cycle(
    exe: &std::path::Path,
    vertices: usize,
    trial: usize,
    policy: &str,
) -> Result<(), String> {
    let vertex_arg = vertices.to_string();
    let trial_arg = trial.to_string();
    match launch_guarded(
        exe,
        &["--cycle-worker", &vertex_arg, policy, &trial_arg],
        512,
        Duration::from_secs(5),
    )? {
        GuardedExit::Success => Ok(()),
        GuardedExit::Failed(status) => Err(format!(
            "cycle/{vertices}/{trial}/{policy}: WORKER_FAILURE ({status}); no certified count"
        )),
        GuardedExit::Timeout => Err(format!(
            "cycle/{vertices}/{trial}/{policy}: TIMEOUT; no certified count"
        )),
    }
}

fn cycle_suite() -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("connected-cycle VmHWM research requires Linux".into());
    }
    let exe = std::env::current_exe().map_err(|e| format!("executable path: {e}"))?;
    guard_negative_controls(&exe)?;
    println!(
        "vertices,variables,trial,policy,solve_wall_ns,process_peak_rss_kib,nodes,cache_entries,cache_hits,decompositions,key_clauses,key_literals"
    );
    for vertices in [6, 10, 14] {
        for trial in 0..3 {
            // Deterministically alternate first policy across independent
            // process pairs to reduce drift; never reuse an in-process cache.
            let policies = if trial % 2 == 0 {
                ["off", "on"]
            } else {
                ["on", "off"]
            };
            for policy in policies {
                launch_cycle(&exe, vertices, trial, policy)?;
            }
        }
    }
    Ok(())
}

// Only compiled when explicitly profiling cache transactions. This suite
// uses the same independent oracle and parent-supervised children as cycle-suite,
// but timers perturb the solver and MUST NOT be compared as an uninstrumented
// performance benchmark or interpreted as a statistically reliable cost model.
#[cfg(feature = "cache-cost-profile")]
fn cost_worker(vertices: usize, policy: &str, trial: usize) -> Result<(), String> {
    if ![6, 10, 14].contains(&vertices) || trial >= 3 {
        return Err("cost cohort only accepts 6/10/14 vertices and trials 0..3".into());
    }
    let mut limits = Limits::default();
    if policy == "off" {
        limits.cache_entries = 0;
    } else if policy == "cap1" {
        // Independent research-only saturation cohort. Leave legacy/on/tight
        // cohorts and their 27-row evidence unchanged.
        limits.cache_entries = 1;
    } else if policy != "on" && policy != "tight" {
        return Err("cost policy must be off, on, tight or cap1".into());
    }
    let input = guarded_cycle_residual(vertices);
    let expected = independent_cycle_wmc(vertices);
    let started = Instant::now();
    let answer = if policy == "tight" {
        solve_with_cache_payload_budget(
            &input,
            limits,
            CachePayloadBudget {
                scope_items: 2,
                clause_items: 2,
                literal_items: 2,
                value_bits: 32,
            },
        )
    } else {
        solve_with_limits(&input, limits)
    }
    .map_err(|e| format!("cost/{vertices}/{trial}/{policy}: fail-closed {e}"))?;
    let elapsed_ns = started.elapsed().as_nanos();
    if !answer.satisfiable || answer.count != expected {
        return Err("instrumented exact Full WMC or SAT oracle mismatch".into());
    }
    let stats = &answer.stats;
    let c = &stats.cache_cost;
    if policy == "off" {
        if stats.cache_hits != 0
            || stats.cache_entries != 0
            || c.key_build_calls != 0
            || c.lookup_calls != 0
            || c.admission_calls != 0
            || c.insert_calls != 0
        {
            return Err("disabled cache still performed an instrumented transaction".into());
        }
    } else {
        if c.key_build_calls != stats.nodes
            || c.lookup_calls != c.key_build_calls
            || c.hit_clone_calls != stats.cache_hits
            || c.insert_calls != stats.cache_entries as u64
            || c.admission_calls != c.insert_calls + stats.cache_payload_rejections
        {
            return Err("cache transaction accounting mismatch".into());
        }
        if policy == "on" && stats.cache_hits == 0 {
            return Err("unbounded cache did not reuse a residual".into());
        }
        if policy == "tight" && stats.cache_payload_rejections == 0 {
            return Err("tight quota did not reject any cache entries".into());
        }
    }
    if policy == "cap1"
        && (stats.cache_entries != 1 || c.full_cache_lookup_calls == 0)
    {
        return Err("one-entry saturation was not exercised on a later lookup".into());
    }
    let peak_rss = peak_rss_kib()?;
    println!(
        "{vertices},{},{trial},{policy},{elapsed_ns},{peak_rss},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        vertices + 1,
        stats.nodes,
        stats.cache_hits,
        stats.cache_entries,
        stats.cache_payload_rejections,
        c.key_build_calls,
        c.key_build_ns,
        c.lookup_calls,
        c.lookup_ns,
        c.hit_clone_calls,
        c.hit_clone_ns,
        c.admission_calls,
        c.admission_ns,
        c.insert_calls,
        c.insert_ns,
        c.root_normalize_calls,
        c.root_normalize_ns,
        c.recursive_normalize_calls,
        c.recursive_normalize_ns,
        c.full_cache_lookup_calls,
        c.full_cache_lookup_hits,
    );
    Ok(())
}

#[cfg(feature = "cache-cost-profile")]
fn launch_cost(
    exe: &std::path::Path,
    vertices: usize,
    trial: usize,
    policy: &str,
) -> Result<(), String> {
    let vertices_arg = vertices.to_string();
    let trial_arg = trial.to_string();
    match launch_guarded(
        exe,
        &["--cost-worker", &vertices_arg, policy, &trial_arg],
        512,
        Duration::from_secs(5),
    )? {
        GuardedExit::Success => Ok(()),
        GuardedExit::Failed(status) => Err(format!(
            "cost/{vertices}/{trial}/{policy}: WORKER_FAILURE ({status}); no certified count"
        )),
        GuardedExit::Timeout => Err(format!(
            "cost/{vertices}/{trial}/{policy}: TIMEOUT; no certified count"
        )),
    }
}

#[cfg(feature = "cache-cost-profile")]
fn cost_capacity_suite() -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("cache capacity attribution requires Linux VmHWM".into());
    }
    let exe = std::env::current_exe().map_err(|e| format!("executable path: {e}"))?;
    guard_negative_controls(&exe)?;
    println!(
        "vertices,variables,trial,policy,solve_wall_ns,process_peak_rss_kib,nodes,cache_hits,cache_entries,cache_payload_rejections,key_build_calls,key_build_ns,lookup_calls,lookup_ns,hit_clone_calls,hit_clone_ns,admission_calls,admission_ns,insert_calls,insert_ns,root_normalize_calls,root_normalize_ns,recursive_normalize_calls,recursive_normalize_ns,full_cache_lookup_calls,full_cache_lookup_hits"
    );
    for vertices in [6, 10, 14] {
        for trial in 0..3 {
            launch_cost(&exe, vertices, trial, "cap1")?;
        }
    }
    Ok(())
}

#[cfg(feature = "cache-cost-profile")]
fn cost_suite() -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("cache transaction profiler requires Linux VmHWM".into());
    }
    let exe = std::env::current_exe().map_err(|e| format!("executable path: {e}"))?;
    guard_negative_controls(&exe)?;
    println!(
        "vertices,variables,trial,policy,solve_wall_ns,process_peak_rss_kib,nodes,cache_hits,cache_entries,cache_payload_rejections,key_build_calls,key_build_ns,lookup_calls,lookup_ns,hit_clone_calls,hit_clone_ns,admission_calls,admission_ns,insert_calls,insert_ns,root_normalize_calls,root_normalize_ns,recursive_normalize_calls,recursive_normalize_ns,full_cache_lookup_calls,full_cache_lookup_hits"
    );
    const POLICIES: [&str; 3] = ["off", "on", "tight"];
    for vertices in [6, 10, 14] {
        for trial in 0..3 {
            for position in 0..POLICIES.len() {
                // Rotate policy order; one fresh bounded process per row.
                let policy = POLICIES[(trial + position) % POLICIES.len()];
                launch_cost(&exe, vertices, trial, policy)?;
            }
        }
    }
    Ok(())
}

#[cfg(feature = "cache-admission-research")]
fn admission_worker(
    graph_label: &str,
    vertices: usize,
    trial: usize,
    policy: &str,
) -> Result<(), String> {
    if ![6, 10, 14].contains(&vertices) || trial >= 3 {
        return Err("admission sizes must be 6/10/14 and trials 0..3".into());
    }
    let graph = Graph::from_label(graph_label)
        .ok_or_else(|| "unknown predeclared graph family".to_string())?;
    let input = guarded_graph(graph, vertices);
    let expected = graph_oracle(graph, vertices);
    let mut limits = Limits::default();
    let threshold = match policy {
        "off" => {
            limits.cache_entries = 0;
            None
        }
        "legacy" => None,
        "min4" => Some(4),
        "min12" => Some(12),
        _ => return Err("admission policy must be off/legacy/min4/min12".into()),
    };
    let started = Instant::now();
    let answer = match threshold {
        Some(n) => solve_with_min_cache_clauses(&input, limits, n),
        None => solve_with_limits(&input, limits),
    }
    .map_err(|e| format!("admission/{graph_label}/{vertices}/{trial}/{policy}: {e}"))?;
    let elapsed_ns = started.elapsed().as_nanos();
    if !answer.satisfiable || answer.count != expected {
        return Err("admission independent Full Complex WMC / SAT mismatch".into());
    }
    let stats = &answer.stats;
    if policy == "off" {
        if stats.cache_entries != 0
            || stats.cache_hits != 0
            || stats.cache_min_clause_rejections != 0
        {
            return Err("disabled cache retained an entry or rejection".into());
        }
    } else if policy == "legacy" {
        if stats.cache_hits == 0 || stats.cache_min_clause_rejections != 0 {
            return Err("unmodified legacy cache did not reuse the guaranteed residual".into());
        }
    } else if stats.cache_min_clause_rejections == 0 {
        return Err("structural admission policy never rejected a candidate".into());
    }
    if stats.cache_payload_rejections != 0 {
        return Err("structural admission cohort unexpectedly applied a payload budget".into());
    }
    let rss = peak_rss_kib()?;
    println!(
        "{graph_label},{vertices},{trial},{policy},{elapsed_ns},{rss},{},{},{},{},{},{},{}",
        stats.nodes,
        stats.cache_hits,
        stats.cache_entries,
        stats.cache_min_clause_rejections,
        stats.cache_scope_items,
        stats.cache_clause_items,
        stats.cache_literal_items
    );
    Ok(())
}

#[cfg(feature = "cache-admission-research")]
fn launch_admission(
    exe: &std::path::Path,
    graph: &str,
    vertices: usize,
    trial: usize,
    policy: &str,
) -> Result<(), String> {
    let n_arg = vertices.to_string();
    let trial_arg = trial.to_string();
    match launch_guarded(
        exe,
        &["--admission-worker", graph, &n_arg, policy, &trial_arg],
        512,
        Duration::from_secs(5),
    )? {
        GuardedExit::Success => Ok(()),
        GuardedExit::Failed(status) => Err(format!(
            "admission/{graph}/{vertices}/{trial}/{policy}: WORKER_FAILURE ({status}); no certified result"
        )),
        GuardedExit::Timeout => Err(format!(
            "admission/{graph}/{vertices}/{trial}/{policy}: TIMEOUT; no certified result"
        )),
    }
}

#[cfg(feature = "cache-admission-research")]
fn admission_suite() -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("admission VmHWM study requires Linux".into());
    }
    let exe = std::env::current_exe().map_err(|e| format!("executable path: {e}"))?;
    guard_negative_controls(&exe)?;
    println!(
        "family,vertices,trial,policy,solve_wall_ns,process_peak_rss_kib,nodes,cache_hits,cache_entries,threshold_rejections,cache_scope_items,cache_clause_items,cache_literal_items"
    );
    const POLICIES: [&str; 4] = ["off", "legacy", "min4", "min12"];
    for graph in ["cycle", "rungs", "spokes"] {
        for vertices in [6, 10, 14] {
            for trial in 0..3 {
                for offset in 0..POLICIES.len() {
                    let policy = POLICIES[(trial + offset) % POLICIES.len()];
                    launch_admission(&exe, graph, vertices, trial, policy)?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(feature = "cache-admission-research")]
fn work_proxy_worker(
    family: &str,
    variables: usize,
    trial: usize,
    policy: &str,
) -> Result<(), String> {
    if ![8, 10, 12].contains(&variables) || trial >= 3 {
        return Err("work proxy requires source-pinned n=8/10/12 and trials 0..3".into());
    }
    let holdout = Holdout::parse(family).ok_or_else(|| "unknown holdout family".to_string())?;
    let input = selector_free_cnf(holdout, variables);
    let (expected_sat, expected_wmc) = independent_wmc(holdout, variables);
    let mut limits = Limits::default();
    let ratio = match policy {
        "off" => {
            limits.cache_entries = 0;
            None
        }
        "legacy" => None,
        "proxy1" => Some(1),
        "proxy4" => Some(4),
        _ => return Err("unknown source-pinned work proxy policy".into()),
    };
    let started = Instant::now();
    let result = match ratio {
        Some(ratio) => solve_with_observed_work_ratio(&input, limits, ratio),
        None => solve_with_limits(&input, limits),
    }
    .map_err(|e| format!("work/{family}/{variables}/{trial}/{policy}: {e}"))?;
    let elapsed_ns = started.elapsed().as_nanos();
    if result.satisfiable != expected_sat || result.count != expected_wmc {
        return Err("selector-free independent Full Complex WMC / SAT mismatch".into());
    }
    let stats = &result.stats;
    if policy == "off"
        && (stats.cache_hits != 0
            || stats.cache_entries != 0
            || stats.cache_work_proxy_rejections != 0)
    {
        return Err("disabled cache exercised research admission or retained entries".into());
    }
    if policy == "legacy" && stats.cache_work_proxy_rejections != 0 {
        return Err("default cache unexpectedly used work proxy filter".into());
    }
    if policy.starts_with("proxy") && stats.cache_work_proxy_rejections == 0 {
        return Err("research proxy was not exercised on selector-free CNF".into());
    }
    if stats.cache_payload_rejections != 0 || stats.cache_min_clause_rejections != 0 {
        return Err("other research admission policies unexpectedly active".into());
    }
    let rss = peak_rss_kib()?;
    println!(
        "{family},{variables},{trial},{policy},{elapsed_ns},{rss},{},{},{},{},{},{},{},{}",
        stats.nodes,
        stats.cache_hits,
        stats.cache_entries,
        stats.cache_work_proxy_rejections,
        stats.cache_scope_items,
        stats.cache_clause_items,
        stats.cache_literal_items,
        stats.cache_value_bits
    );
    Ok(())
}

#[cfg(feature = "cache-admission-research")]
fn launch_work_proxy(
    exe: &std::path::Path,
    family: &str,
    variables: usize,
    trial: usize,
    policy: &str,
) -> Result<(), String> {
    let variables_arg = variables.to_string();
    let trial_arg = trial.to_string();
    match launch_guarded(
        exe,
        &["--work-worker", family, &variables_arg, policy, &trial_arg],
        512,
        Duration::from_secs(5),
    )? {
        GuardedExit::Success => Ok(()),
        GuardedExit::Failed(status) => Err(format!(
            "work/{family}/{variables}/{trial}/{policy}: WORKER_FAILURE ({status})"
        )),
        GuardedExit::Timeout => Err(format!(
            "work/{family}/{variables}/{trial}/{policy}: TIMEOUT"
        )),
    }
}

#[cfg(feature = "cache-admission-research")]
fn work_proxy_suite() -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("selector-free work-proxy VmHWM research requires Linux".into());
    }
    let exe = std::env::current_exe().map_err(|e| format!("executable path: {e}"))?;
    guard_negative_controls(&exe)?;
    println!(
        "family,variables,trial,policy,solve_wall_ns,process_peak_rss_kib,nodes,cache_hits,cache_entries,work_proxy_rejections,key_scope_items,key_clauses,key_literals,value_bits"
    );
    const POLICIES: [&str; 4] = ["off", "legacy", "proxy1", "proxy4"];
    for family in ["ring", "chorded", "mixed"] {
        for variables in [8, 10, 12] {
            for trial in 0..3 {
                for offset in 0..POLICIES.len() {
                    let policy = POLICIES[(trial + offset) % POLICIES.len()];
                    launch_work_proxy(&exe, family, variables, trial, policy)?;
                }
            }
        }
    }
    Ok(())
}

// Branch-order ablation uses a distinct feature and the unaltered cache
// admission contract: either disabled or exactly the normal legacy policy.
#[cfg(feature = "branching-research")]
fn branching_worker(
    family: &str,
    variables: usize,
    trial: usize,
    cache: &str,
    pivot: &str,
) -> Result<(), String> {
    if ![8, 10, 12].contains(&variables) || trial >= 3 {
        return Err("branching holdouts require n=8/10/12 and trials 0..3".into());
    }
    let kind = BranchingFamily::parse(family)
        .ok_or_else(|| "unknown source-pinned branching family".to_string())?;
    let pivot_policy = match pivot {
        "frequency" => PivotPolicy::Frequency,
        "short" => PivotPolicy::ShortClause,
        "minfill" => PivotPolicy::MinFill,
        _ => return Err("pivot must be frequency|short|minfill".into()),
    };
    let cache_entries = match cache {
        "off" => 0,
        "legacy" => Limits::default().cache_entries,
        _ => return Err("cache must be off|legacy".into()),
    };
    let (problem, expected_sat, expected_count) = branching_fixture(kind, variables);
    let limits = Limits {
        cache_entries,
        ..Limits::default()
    };
    let started = Instant::now();
    let answer = solve_with_pivot_policy(&problem, limits, pivot_policy)
        .map_err(|e| format!("{family}/{variables}/{trial}/{cache}/{pivot}: {e}"))?;
    let elapsed_ns = started.elapsed().as_nanos();
    if answer.satisfiable != expected_sat || answer.count != expected_count {
        return Err("independent full Gaussian WMC / SAT oracle disagreed".into());
    }
    let stats = &answer.stats;
    if stats.pivot_decisions == 0 {
        return Err("branching experiment never selected a nontrivial pivot".into());
    }
    if pivot != "minfill" && stats.pivot_fallbacks != 0 {
        return Err("non-minfill pivot unexpectedly recorded fallback".into());
    }
    if cache == "off" && (stats.cache_hits != 0 || stats.cache_entries != 0) {
        return Err("cache-off pivot worker performed cached work".into());
    }
    if stats.cache_min_clause_rejections != 0
        || stats.cache_work_proxy_rejections != 0
        || stats.cache_payload_rejections != 0
    {
        return Err("cache admission policy confounded branching experiment".into());
    }
    let peak = peak_rss_kib()?;
    println!(
        "{family},{variables},{trial},{cache},{pivot},{elapsed_ns},{peak},{},{},{},{},{},{}",
        stats.nodes,
        stats.cache_hits,
        stats.cache_entries,
        stats.decompositions,
        stats.pivot_decisions,
        stats.pivot_fallbacks,
    );
    Ok(())
}

#[cfg(feature = "branching-research")]
fn launch_branching(
    exe: &std::path::Path,
    family: &str,
    variables: usize,
    trial: usize,
    cache: &str,
    pivot: &str,
) -> Result<(), String> {
    let n = variables.to_string();
    let t = trial.to_string();
    match launch_guarded(
        exe,
        &["--branch-worker", family, &n, cache, pivot, &t],
        512,
        Duration::from_secs(5),
    )? {
        GuardedExit::Success => Ok(()),
        GuardedExit::Failed(status) => Err(format!(
            "{family}/{variables}/{trial}/{cache}/{pivot}: WORKER_FAILURE {status}"
        )),
        GuardedExit::Timeout => Err(format!(
            "{family}/{variables}/{trial}/{cache}/{pivot}: TIMEOUT; no certified result"
        )),
    }
}

#[cfg(feature = "branching-research")]
fn branching_suite() -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("branching VmHWM study requires Linux".into());
    }
    let exe = std::env::current_exe().map_err(|e| format!("executable path: {e}"))?;
    guard_negative_controls(&exe)?;
    println!(
        "family,variables,trial,cache,pivot,solve_wall_ns,process_peak_rss_kib,nodes,cache_hits,cache_entries,decompositions,pivot_decisions,pivot_fallbacks"
    );
    const PIVOTS: [&str; 3] = ["frequency", "short", "minfill"];
    for family in ["ring", "chorded", "mixed", "skewed"] {
        for variables in [8, 10, 12] {
            for trial in 0..3 {
                for cache in ["off", "legacy"] {
                    for offset in 0..PIVOTS.len() {
                        let pivot = PIVOTS[(trial + offset) % PIVOTS.len()];
                        launch_branching(&exe, family, variables, trial, cache, pivot)?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    match args.as_slice() {
        [_] => suite(),
        [_, flag] if flag == "--quota-suite" => quota_suite(),
        [_, flag] if flag == "--repeat-suite" => repeated_suite(),
        [_, flag] if flag == "--cycle-suite" => cycle_suite(),
        #[cfg(feature = "cache-cost-profile")]
        [_, flag] if flag == "--cost-suite" => cost_suite(),
        #[cfg(feature = "cache-cost-profile")]
        [_, flag] if flag == "--cost-capacity-suite" => cost_capacity_suite(),
        #[cfg(feature = "cache-admission-research")]
        [_, flag] if flag == "--admission-suite" => admission_suite(),
        #[cfg(feature = "cache-admission-research")]
        [_, flag] if flag == "--work-suite" => work_proxy_suite(),
        #[cfg(feature = "branching-research")]
        [_, flag] if flag == "--branch-suite" => branching_suite(),
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
        [_, flag, gadgets, policy, trial] if flag == "--repeat-worker" => {
            let gadgets = gadgets.parse::<usize>().map_err(|e| format!("gadgets: {e}"))?;
            let trial = trial.parse::<usize>().map_err(|e| format!("trial: {e}"))?;
            repeated_worker(gadgets, policy, trial)
        }
        #[cfg(feature = "cache-cost-profile")]
        [_, flag, vertices, policy, trial] if flag == "--cost-worker" => {
            let vertices = vertices.parse::<usize>().map_err(|e| format!("vertices: {e}"))?;
            let trial = trial.parse::<usize>().map_err(|e| format!("trial: {e}"))?;
            cost_worker(vertices, policy, trial)
        }
        #[cfg(feature = "cache-admission-research")]
        [_, flag, graph, vertices, policy, trial] if flag == "--admission-worker" => {
            let vertices = vertices.parse::<usize>().map_err(|e| format!("vertices: {e}"))?;
            let trial = trial.parse::<usize>().map_err(|e| format!("trial: {e}"))?;
            admission_worker(graph, vertices, trial, policy)
        }
        #[cfg(feature = "cache-admission-research")]
        [_, flag, family, variables, policy, trial] if flag == "--work-worker" => {
            let variables = variables.parse::<usize>().map_err(|e| format!("variables: {e}"))?;
            let trial = trial.parse::<usize>().map_err(|e| format!("trial: {e}"))?;
            work_proxy_worker(family, variables, trial, policy)
        }
        #[cfg(feature = "branching-research")]
        [_, flag, family, variables, cache, pivot, trial] if flag == "--branch-worker" => {
            let variables = variables.parse::<usize>().map_err(|e| format!("variables: {e}"))?;
            let trial = trial.parse::<usize>().map_err(|e| format!("trial: {e}"))?;
            branching_worker(family, variables, trial, cache, pivot)
        }
        [_, flag, vertices, policy, trial] if flag == "--cycle-worker" => {
            let vertices = vertices.parse::<usize>().map_err(|e| format!("vertices: {e}"))?;
            let trial = trial.parse::<usize>().map_err(|e| format!("trial: {e}"))?;
            cycle_worker(vertices, policy, trial)
        }
        [_, flag, name, n, cap] if flag == "--worker" => {
            let variables = n.parse::<usize>().map_err(|e| format!("variables: {e}"))?;
            let cache_cap = cap
                .parse::<usize>()
                .map_err(|e| format!("cache cap: {e}"))?;
            worker(name, variables, cache_cap)
        }
        _ => Err("usage: cache_profile [--branch-suite (requires branching-research)|--branch-worker family variables off|legacy frequency|short|minfill trial|--work-suite (requires cache-admission-research)|--work-worker family variables off|legacy|proxy1|proxy4 trial|--admission-suite (requires cache-admission-research)|--admission-worker family vertices off|legacy|min4|min12 trial|--cost-suite|--cost-capacity-suite (requires cache-cost-profile feature)|--cost-worker vertices off|on|tight|cap1 trial|--cycle-suite|--cycle-worker vertices off|on trial|--repeat-suite|--repeat-worker gadgets off|legacy|tight|roomy trial|--quota-suite|--quota-worker chain|pairs|cache n legacy|tight|roomy] [--worker chain|pairs|cache n cache_cap] [--stress-memory|--stress-sleep]".into()),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("CACHE_PROFILE_FAIL: {error}");
        std::process::exit(1);
    }
}
