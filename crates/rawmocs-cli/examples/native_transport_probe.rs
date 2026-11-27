//! R6 transport probe; the emitted diagnostic lines belong to this executable.
use rawmocs_mpfr::{solve_file, solve_text, Precision, SolveOptions, SolverMode};

fn main() {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().expect("CNF path");
    let transport = args.next().expect("transport");
    let mode = args.next().expect("mode");
    let digits = args.next().expect("digits");
    assert!(args.next().is_none());
    let options = SolveOptions {
        mode: SolverMode::from_name(mode.to_str().unwrap()).unwrap(),
        precision: Precision::from_digits(digits.to_str().unwrap().parse().unwrap()).unwrap(),
        timeout_ms: Some(120_000),
        ..SolveOptions::default()
    };
    let answer = match transport.to_str().unwrap() {
        "file" => solve_file(&path, options).unwrap(),
        "text" => {
            let raw = std::fs::read_to_string(&path).unwrap();
            solve_text(&raw, options).unwrap()
        }
        _ => panic!("unknown transport"),
    };
    let stats = answer.diagnostics.prejoin.as_ref().unwrap();
    let sat = if answer.sat {
        "SATISFIABLE"
    } else {
        "UNSATISFIABLE"
    };
    println!("s {sat}\nc r mpfr {} {}", answer.real, answer.imag);
    eprintln!(
        "RAWMOCS_COMPLETE route={} digits={} bits={} solve_ns={}",
        answer.route.as_str(),
        answer.precision.digits(),
        answer.precision.bits(),
        answer.solve_ns
    );
    eprintln!(
        "PREJOIN_COMPLETE residual={} cuts={} leaves={} width={} work={} peak={} joins={} plan_ns={}",
        stats.residual,
        stats.cuts,
        stats.leaves,
        stats.width,
        stats.work,
        stats.peak,
        stats.joins,
        stats.plan_ns
    );
}
