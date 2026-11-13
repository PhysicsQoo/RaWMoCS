//! Research-only fresh-process native CLI using the QuPRS GPMC printed-CNF dialect.
//! This does not change rawmocs-core public API or QuPRS default SolverAdapter.
use rawmocs_benchmarks::quprs_printed_cnf::{parse_printed_dimacs, Dialect};
use rawmocs_core::{solve_with_limits, Limits};

fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let file = args
        .next()
        .ok_or_else(|| "usage: quprs_native_cli <source-pinned-gpmc.cnf>".to_string())?;
    if args.next().is_some() {
        return Err("expected exactly one CNF file argument".to_string());
    }
    let raw = std::fs::read_to_string(file).map_err(|err| format!("read CNF: {err}"))?;
    let problem = parse_printed_dimacs(&raw, Dialect::Gpmc)?;
    let report = solve_with_limits(&problem, Limits::default())
        .map_err(|err| format!("bounded exact Full WMC failed: {err}"))?;
    println!(
        "s {}",
        if report.satisfiable {
            "SATISFIABLE"
        } else {
            "UNSATISFIABLE"
        }
    );
    println!("c r rational {} {}", report.count.real, report.count.imag);
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("RAWMOCS_FAIL_CLOSED: {err}");
        std::process::exit(1);
    }
}
