use rawmocs::{solve_text, FailureKind, Precision, SolveOptions, SolverMode};
fn main() {
    let cases = [
        ("signed", "p cnf 1 1\nc t wmc\nc p weight 1 -0.125 0.25 0\n1 0\n", true, -0.125, 0.25),
        ("zero_sat", "p cnf 1 0\nc t wmc\nc p weight 1 -1 0 0\n", true, 0.0, 0.0),
        ("unsat", "p cnf 1 2\nc t wmc\n1 0\n-1 0\n", false, 0.0, 0.0),
    ];
    for precision in [Precision::Digits30, Precision::Digits130] {
        for mode in [SolverMode::Auto, SolverMode::General] {
            for (label, input, sat, re, im) in cases {
                let opt = SolveOptions { precision, mode, timeout_ms: Some(2000), large_general: false };
                let answer = solve_text(input, opt).unwrap();
                assert_eq!(answer.sat, sat);
                for (value, expected) in [(answer.real_mpfr(), re), (answer.imag_mpfr(), im)] {
                    let b = value.binary();
                    let integer = rug::Integer::from_str_radix(&b.significand, 10).unwrap();
                    let mut restored = rug::Float::with_val(b.precision_bits, integer);
                    restored <<= b.exponent2;
                    assert_eq!(restored, expected);
                    assert_eq!(b.precision_bits, precision.bits());
                }
                println!("W1_COUNT {label} {} {} {} {} {}", precision.digits(), mode.as_str(), answer.sat, answer.real, answer.imag);
            }
        }
    }
    let valid = "p cnf 1 0\nc t wmc\n";
    assert_eq!(solve_text("not a CNF", SolveOptions::default()).unwrap_err().kind, FailureKind::InvalidInput);
    assert_eq!(solve_text(valid, SolveOptions { timeout_ms: Some(0), ..SolveOptions::default() }).unwrap_err().kind, FailureKind::Timeout);
    assert!(solve_text(valid, SolveOptions::default()).unwrap().sat);
    println!("W1_MPFR_SMOKE_PASS cases=12 failure_controls=2 reentry=1");
}
