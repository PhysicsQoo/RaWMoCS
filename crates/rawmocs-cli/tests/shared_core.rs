use rawmocs_mpfr::{solve_file, solve_text, SolveOptions};

#[test]
fn shared_file_memory_cli_parity() {
    let cases = [
        ("p cnf 1 0\nc t wmc\nc p weight 1 -1 0 0\n", true),
        ("p cnf 1 2\nc t wmc\n1 0\n-1 0\n", false),
        ("p cnf 2 1\nc t wmc\nc p weight 1 -0.125 0.25 0\n1 2 0\n", true),
    ];
    let path = std::env::temp_dir().join(format!("rawmocs-shared-{}.cnf", std::process::id()));
    for (raw, sat) in cases {
        std::fs::write(&path, raw).unwrap();
        for mode in ["auto", "general", "auto-frontier", "auto-separator"] {
            for digits in [30, 130] {
                let options = SolveOptions {
                    digits,
                    mode,
                    timeout_ms: Some(20_000),
                    large_general: false,
                };
                let a = solve_file(path.as_os_str(), options).unwrap();
                let b = solve_text(raw, options).unwrap();
                assert_eq!(a.sat, sat);
                assert_eq!(
                    (a.sat, &a.real, &a.imag, a.route, a.bits),
                    (b.sat, &b.real, &b.imag, b.route, b.bits)
                );
                let output = std::process::Command::new(env!("CARGO_BIN_EXE_rawmocs"))
                    .args([
                        "--digits",
                        &digits.to_string(),
                        "--solver",
                        mode,
                        "--timeout-ms",
                        "20000",
                    ])
                    .arg(&path)
                    .output()
                    .unwrap();
                assert!(output.status.success());
                let expected_sat = if sat { "SATISFIABLE" } else { "UNSATISFIABLE" };
                assert_eq!(
                    String::from_utf8(output.stdout).unwrap(),
                    format!("s {expected_sat}\nc r mpfr {} {}\n", a.real, a.imag)
                );
                assert!(String::from_utf8_lossy(&output.stderr)
                    .contains(&format!("route={} digits={} bits={}", a.route, digits, a.bits)));
            }
        }
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn library_failure_has_no_count_and_reentry_succeeds() {
    let raw = "p cnf 1 0\nc t wmc\n";
    let invalid = solve_text("not a CNF", SolveOptions::default()).unwrap_err();
    assert_eq!((invalid.kind, invalid.exit_code), ("INVALID_INPUT", 2));
    let timeout = solve_text(
        raw,
        SolveOptions {
            timeout_ms: Some(0),
            ..SolveOptions::default()
        },
    )
    .unwrap_err();
    assert_eq!((timeout.kind, timeout.exit_code), ("TIMEOUT", 4));
    let answer = solve_text(raw, SolveOptions::default()).unwrap();
    assert!(answer.sat);
}
