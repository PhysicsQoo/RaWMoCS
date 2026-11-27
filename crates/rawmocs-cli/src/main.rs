//! Command-line transport for the shared MPFR production solver.
use rawmocs_mpfr::{precision_bits, solve_file_cli, SolveFailure};

const HELP: &str = "rawmocs --digits {30|60|100|130} [--solver auto|auto-frontier|auto-separator|prejoin|general|general-separator|reference] [--timeout-ms N] [--large-general] FILE\n\
Full signed complex WMC for QuPRS weighted CNF. Output is rounded MPFR, not a certified interval.\n\
Exit 0: complete SAT and complex count. Nonzero: classified failure, empty stdout. Use an external process resource supervisor.";
fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let first = args.next().ok_or(HELP)?;
    if first == "--help" && args.next().is_none() {
        println!("{HELP}");
        return Ok(());
    }
    if first == "--version" && args.next().is_none() {
        println!("rawmocs {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if first != "--digits" {
        return Err(HELP.into());
    }
    let digits: u32 = args
        .next()
        .and_then(|x| x.into_string().ok())
        .ok_or("missing digits")?
        .parse()
        .map_err(|_| "invalid digits")?;
    precision_bits(digits).ok_or("unsupported digits")?;
    let mut file = args.next().ok_or("missing source")?;
    let mut mode = "auto".to_owned();
    let mut timeout_ms = None;
    let mut mode_seen = false;
    let mut large_general = false;
    loop {
        if file == "--solver" && !mode_seen {
            mode = args
                .next()
                .and_then(|x| x.into_string().ok())
                .ok_or("missing solver")?;
            mode_seen = true;
        } else if file == "--large-general" && !large_general {
            large_general = true;
        } else if file == "--timeout-ms" && timeout_ms.is_none() {
            let ms = args
                .next()
                .and_then(|x| x.into_string().ok())
                .and_then(|x| x.parse::<u64>().ok())
                .filter(|&ms| ms <= 86_400_000)
                .ok_or("INPUT: invalid timeout-ms (expected 0..86400000)")?;
            timeout_ms = Some(ms);
        } else {
            break;
        }
        file = args.next().ok_or("missing source")?;
    }
    if args.next().is_some() {
        return Err("unexpected arguments".into());
    }
    let answer = solve_file_cli(&file, digits, &mode, timeout_ms, large_general)
        .map_err(SolveFailure::into_reason)?;
    let sat = if answer.sat {
        "SATISFIABLE"
    } else {
        "UNSATISFIABLE"
    };
    let (real, imag) = (&answer.real, &answer.imag);
    println!("s {sat}\nc r mpfr {real} {imag}");
    eprintln!(
        "RAWMOCS_COMPLETE route={} digits={} bits={} solve_ns={}",
        answer.route.as_str(),
        answer.precision.digits(),
        answer.precision.bits(),
        answer.solve_ns
    );
    Ok(())
}
fn main() {
    if let Err(reason) = run() {
        let (kind, code) = if reason.starts_with("INPUT:")
            || matches!(
                reason.as_str(),
                HELP | "missing digits"
                    | "invalid digits"
                    | "unsupported digits"
                    | "missing source"
                    | "missing solver"
                    | "unexpected arguments"
            ) {
            ("INVALID_INPUT", 2)
        } else {
            let error = SolveFailure::from_reason(reason.clone());
            (error.kind.as_str(), error.exit_code())
        };
        eprintln!("RAWMOCS_FAILURE kind={kind}: {reason}");
        std::process::exit(code);
    }
}
