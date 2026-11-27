//! External consumer fixture: compiled outside the extracted RaWMoCS tree.
use rawmocs_mpfr::{
    solve_file, solve_file_cancellable, solve_text, solve_text_cancellable, CancellationToken,
    FailureKind, Precision, SolveOptions, SolverMode, SolveRoute,
};

fn main() {
    let path = std::env::args_os().nth(1).expect("SAT-zero fixture");
    let options = SolveOptions {
        precision: Precision::Digits60,
        mode: SolverMode::General,
        ..SolveOptions::default()
    };
    let raw = std::fs::read_to_string(&path).unwrap();
    let file = solve_file(&path, options).unwrap();
    let text = solve_text(&raw, options).unwrap();
    assert!(file.sat && text.sat);
    assert_eq!(file.route, SolveRoute::General);
    assert_eq!(file.route, text.route);
    assert_eq!(file.precision.bits(), 200);
    let real = file.real_mpfr().binary();
    let imag = file.imag_mpfr().binary();
    assert_eq!(real.significand, "0");
    assert_eq!(imag.significand, "0");
    assert_eq!(real.precision_bits, 200);
    assert_eq!(real, text.real_mpfr().binary());
    assert_eq!(imag, text.imag_mpfr().binary());
    assert!(file.diagnostics.general.is_some());
    let unsat = solve_text("p cnf 1 2\nc t wmc\n1 0\n-1 0\n", options).unwrap();
    assert!(!unsat.sat);
    assert_eq!(unsat.real_mpfr().binary().significand, "0");
    assert_eq!(
        solve_text("invalid", options).unwrap_err().kind,
        FailureKind::InvalidInput
    );
    let expired = SolveOptions {
        timeout_ms: Some(0),
        ..options
    };
    assert_eq!(
        solve_file(&path, expired).unwrap_err().kind,
        FailureKind::Timeout
    );
    let token = CancellationToken::new();
    token.cancel();
    assert_eq!(
        solve_file_cancellable(&path, options, &token)
            .unwrap_err()
            .kind,
        FailureKind::Cancelled
    );
    assert_eq!(
        solve_text_cancellable(&raw, options, &token)
            .unwrap_err()
            .kind,
        FailureKind::Cancelled
    );
    let fresh = CancellationToken::new();
    let recovered = solve_text_cancellable(&raw, options, &fresh).unwrap();
    assert_eq!(recovered.real_mpfr().binary(), real);
    assert_eq!(recovered.imag_mpfr().binary(), imag);
    println!("EXTRACTED_RUST_CONSUMER_PASS");
}
