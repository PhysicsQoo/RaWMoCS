use rawmocs_mpfr::{solve_file, solve_text, FailureKind, Precision, SolveOptions, SolverMode};

#[test]
fn shared_file_memory_cli_parity() {
    let cases = [
        ("p cnf 1 0\nc t wmc\nc p weight 1 -1 0 0\n", true),
        ("p cnf 1 2\nc t wmc\n1 0\n-1 0\n", false),
        (
            "p cnf 2 1\nc t wmc\nc p weight 1 -0.125 0.25 0\n1 2 0\n",
            true,
        ),
    ];
    let path = std::env::temp_dir().join(format!("rawmocs-shared-{}.cnf", std::process::id()));
    for (raw, sat) in cases {
        std::fs::write(&path, raw).unwrap();
        for mode in [
            SolverMode::Auto,
            SolverMode::General,
            SolverMode::AutoFrontier,
            SolverMode::AutoSeparator,
        ] {
            for precision in [Precision::Digits30, Precision::Digits130] {
                let options = SolveOptions {
                    precision,
                    mode,
                    timeout_ms: Some(20_000),
                    large_general: false,
                };
                let a = solve_file(path.as_os_str(), options).unwrap();
                let b = solve_text(raw, options).unwrap();
                assert_eq!(a.sat, sat);
                assert_eq!(
                    (a.sat, &a.real, &a.imag, a.route, a.precision),
                    (b.sat, &b.real, &b.imag, b.route, b.precision)
                );
                let output = std::process::Command::new(env!("CARGO_BIN_EXE_rawmocs"))
                    .args([
                        "--digits",
                        &precision.digits().to_string(),
                        "--solver",
                        mode.as_str(),
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
                assert!(String::from_utf8_lossy(&output.stderr).contains(&format!(
                    "route={} digits={} bits={}",
                    a.route.as_str(),
                    precision.digits(),
                    precision.bits()
                )));
            }
        }
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn library_failure_has_no_count_and_reentry_succeeds() {
    let raw = "p cnf 1 0\nc t wmc\n";
    let invalid = solve_text("not a CNF", SolveOptions::default()).unwrap_err();
    assert_eq!(invalid.kind, FailureKind::InvalidInput);
    assert_eq!(invalid.exit_code(), 2);
    let timeout = solve_text(
        raw,
        SolveOptions {
            timeout_ms: Some(0),
            ..SolveOptions::default()
        },
    )
    .unwrap_err();
    assert_eq!(timeout.kind, FailureKind::Timeout);
    assert_eq!(timeout.exit_code(), 4);
    let answer = solve_text(raw, SolveOptions::default()).unwrap();
    assert!(answer.sat);
}

#[test]
fn typed_selectors_and_failure_contract() {
    for (digits, bits) in [(30, 100), (60, 200), (100, 333), (130, 432)] {
        let precision = Precision::from_digits(digits).unwrap();
        assert_eq!((precision.digits(), precision.bits()), (digits, bits));
    }
    assert_eq!(Precision::from_digits(31), None);
    for name in [
        "auto",
        "auto-frontier",
        "auto-separator",
        "prejoin",
        "general",
        "general-separator",
        "reference",
    ] {
        assert_eq!(SolverMode::from_name(name).unwrap().as_str(), name);
    }
    assert_eq!(SolverMode::from_name("unknown"), None);
    let raw = "p cnf 1 0\nc t wmc\n";
    for options in [
        SolveOptions {
            timeout_ms: Some(86_400_001),
            ..SolveOptions::default()
        },
        SolveOptions {
            mode: SolverMode::Reference,
            timeout_ms: Some(1),
            ..SolveOptions::default()
        },
        SolveOptions {
            large_general: true,
            ..SolveOptions::default()
        },
    ] {
        let err = solve_text(raw, options).unwrap_err();
        assert_eq!(err.kind, FailureKind::InvalidInput);
        assert_eq!(err.exit_code(), 2);
        assert!(err.reason().starts_with("INPUT:"));
    }
    let unsupported = solve_text("p cnf 1 0\nc t pmc\n", SolveOptions::default()).unwrap_err();
    assert_eq!(unsupported.kind, FailureKind::UnsupportedInput);
    assert_eq!(unsupported.exit_code(), 3);
}

#[test]
fn binary_count_and_call_local_diagnostics() {
    let raw = "p cnf 1 1\nc t wmc\nc p weight 1 -0.125 0.25 0\n1 0\n";
    let options = SolveOptions {
        precision: Precision::Digits130,
        mode: SolverMode::General,
        large_general: true,
        ..SolveOptions::default()
    };
    let result = solve_text(raw, options).unwrap();
    assert!(result.sat);
    let admission = result.diagnostics.streaming_source.as_ref().unwrap();
    assert_eq!(
        (admission.variables, admission.clauses, admission.literals),
        (1, 1, 1)
    );
    assert!(result.diagnostics.fallback.is_none());
    for (value, expected) in [(result.real_mpfr(), -0.125), (result.imag_mpfr(), 0.25)] {
        let binary = value.binary();
        let integer = rug::Integer::from_str_radix(&binary.significand, 10).unwrap();
        let mut restored = rug::Float::with_val(binary.precision_bits, integer);
        restored <<= binary.exponent2;
        assert_eq!(restored, expected);
        assert_eq!(binary.precision_bits, 432);
    }

    let wide = format!(
        "p cnf 21 1\nc t wmc\n{} 0\n",
        (1..=21).map(|v| v.to_string()).collect::<Vec<_>>().join(" ")
    );
    let fallback = solve_text(&wide, SolveOptions::default()).unwrap();
    let info = fallback.diagnostics.fallback.as_ref().unwrap();
    assert_eq!(info.route, rawmocs_mpfr::SolveRoute::General);
    assert_eq!(info.reason, "clause width cap");
    assert!(fallback.diagnostics.streaming_source.is_none());

    let failed = solve_text(
        raw,
        SolveOptions {
            timeout_ms: Some(0),
            ..options
        },
    )
    .unwrap_err();
    assert_eq!(failed.kind, FailureKind::Timeout);
    let fresh = solve_text(raw, SolveOptions::default()).unwrap();
    assert!(fresh.diagnostics.streaming_source.is_none());
    assert!(fresh.diagnostics.fallback.is_none());
}
