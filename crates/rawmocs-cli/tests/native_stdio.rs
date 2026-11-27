//! Process-level stdio isolation: no Rust test-harness output capture.
use rawmocs_mpfr::{solve_file, solve_text, FailureKind, SolveOptions, SolverMode};
use std::path::Path;
use std::process::Command;

const SMALL: &str = "p cnf 2 1\nc t wmc\n1 2 0\n";
const WIDE: &str = "p cnf 10 1\nc t wmc\n1 2 3 4 5 6 7 8 9 10 0\n";

fn frontier() -> String {
    let mut raw = String::from("p cnf 24 24\nc t wmc\n");
    for v in 1..=24 {
        raw.push_str(&format!("c p weight {v} 1e256 0 0\n"));
    }
    for v in 1..=24 {
        raw.push_str(&format!("{v} 0\n"));
    }
    raw
}

fn run_cases(dir: &Path, cli: bool) {
    let frontier = frontier();
    let cases: [(&str, SolverMode, bool, &[&str]); 6] = [
        (
            SMALL,
            SolverMode::General,
            false,
            &["RAWMOCS_GENERAL_COMPLETE"],
        ),
        (
            SMALL,
            SolverMode::GeneralSeparator,
            false,
            &["RAWMOCS_GENERAL_COMPLETE", "RAWMOCS_SEPARATOR_COMPLETE"],
        ),
        (
            SMALL,
            SolverMode::Prejoin,
            false,
            &["PREJOIN_PHASE_NS", "PREJOIN_COMPLETE"],
        ),
        (
            &frontier,
            SolverMode::AutoFrontier,
            false,
            &["PREJOIN_FRONTIER_STOP", "PREJOIN_COMPLETE"],
        ),
        (
            WIDE,
            SolverMode::Auto,
            false,
            &["RAWMOCS_FALLBACK", "RAWMOCS_GENERAL_COMPLETE"],
        ),
        (
            SMALL,
            SolverMode::General,
            true,
            &["RAWMOCS_SOURCE_ADMITTED", "RAWMOCS_GENERAL_COMPLETE"],
        ),
    ];
    let path = dir.join("case.cnf");
    for (raw, mode, large_general, markers) in cases {
        std::fs::write(&path, raw).unwrap();
        let options = SolveOptions {
            mode,
            large_general,
            timeout_ms: Some(20_000),
            ..SolveOptions::default()
        };
        let file = solve_file(path.as_os_str(), options).unwrap();
        let text = solve_text(raw, options).unwrap();
        assert!(file.sat && text.sat);
        assert_eq!(file.route, text.route);
        assert_eq!(file.real_mpfr().binary(), text.real_mpfr().binary());
        assert_eq!(file.imag_mpfr().binary(), text.imag_mpfr().binary());
        assert_eq!(file.diagnostics.general, text.diagnostics.general);
        assert_eq!(file.diagnostics.separator, text.diagnostics.separator);
        assert_eq!(
            file.diagnostics.frontier_stop,
            text.diagnostics.frontier_stop
        );
        assert_eq!(file.diagnostics.fallback, text.diagnostics.fallback);
        assert_eq!(file.diagnostics.streaming_source.is_some(), large_general);
        assert_eq!(
            file.diagnostics.frontier_stop.is_some(),
            mode == SolverMode::AutoFrontier
        );
        assert_eq!(file.diagnostics.fallback.is_some(), raw == WIDE);
        if cli {
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_rawmocs"));
            cmd.args(["--digits", "30", "--solver", mode.as_str()]);
            if large_general {
                cmd.arg("--large-general");
            }
            let output = cmd
                .args(["--timeout-ms", "20000"])
                .arg(&path)
                .output()
                .unwrap();
            assert!(output.status.success(), "{mode:?}: {:?}", output.stderr);
            let expected = format!("s SATISFIABLE\nc r mpfr {} {}\n", file.real, file.imag);
            assert_eq!(output.stdout, expected.as_bytes());
            let stderr = String::from_utf8(output.stderr).unwrap();
            for marker in markers {
                assert!(stderr.contains(marker), "missing {marker}: {stderr}");
            }
            assert!(stderr.contains("RAWMOCS_COMPLETE"));
        }
    }

    for (raw, options, kind) in [
        (
            "bad CNF",
            SolveOptions::default(),
            FailureKind::InvalidInput,
        ),
        (
            SMALL,
            SolveOptions {
                timeout_ms: Some(0),
                ..SolveOptions::default()
            },
            FailureKind::Timeout,
        ),
        (
            WIDE,
            SolveOptions {
                mode: SolverMode::Prejoin,
                ..SolveOptions::default()
            },
            FailureKind::RouteRejected,
        ),
    ] {
        std::fs::write(&path, raw).unwrap();
        assert_eq!(solve_text(raw, options).unwrap_err().kind, kind);
        assert_eq!(
            solve_file(path.as_os_str(), options).unwrap_err().kind,
            kind
        );
    }
}

fn main() {
    let mut args = std::env::args_os().skip(1);
    if let Some(arg) = args.next() {
        assert_eq!(arg, "--quiet-child");
        let dir = args.next().unwrap();
        run_cases(Path::new(&dir), false);
        return;
    }

    let dir = std::env::temp_dir().join(format!("rawmocs-stdio-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .arg("--quiet-child")
        .arg(&dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "native probe: {:?}", output);
    assert!(
        output.stdout.is_empty(),
        "native stdout: {:?}",
        output.stdout
    );
    assert!(
        output.stderr.is_empty(),
        "native stderr: {:?}",
        output.stderr
    );
    run_cases(&dir, true);
    std::fs::remove_dir_all(&dir).unwrap();
    println!(
        "NATIVE_STDIO_ISOLATION_PASS stdout=0 stderr=0 success=6 failure=3 file_and_text=true"
    );
    println!("CLI_DIAGNOSTIC_COMPATIBILITY_PASS cases=6");
}
