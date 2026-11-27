//! Uninstrumented public API cancellation probe for admitted original CNFs.
use rawmocs_mpfr::{
    solve_file_cancellable, CancellationToken, FailureKind, Precision, SolveOptions, SolverMode,
};
use std::time::{Duration, Instant};

fn main() {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().expect("CNF path");
    let mode = args.next().expect("mode");
    let mode = SolverMode::from_name(mode.to_str().unwrap()).unwrap();
    assert!(args.next().is_none());
    let options = SolveOptions {
        mode,
        precision: Precision::Digits130,
        timeout_ms: Some(20_000),
        ..SolveOptions::default()
    };
    let fresh = CancellationToken::new();
    let start = Instant::now();
    let baseline = solve_file_cancellable(&path, options, &fresh).unwrap();
    let baseline_ns = start.elapsed().as_nanos();
    assert!(baseline_ns > 200_000_000, "fixture too short");
    assert!(baseline.sat);
    println!("BASELINE_NS {baseline_ns}");
    for trial in 0..3 {
        let token = CancellationToken::new();
        let latency = std::thread::scope(|scope| {
            let (tx, rx) = std::sync::mpsc::channel();
            let worker = scope.spawn(|| solve_file_cancellable(&path, options, &token));
            // A separate thread observes completion without test-only hooks.
            let observer = scope.spawn(move || tx.send(worker.join().unwrap()).unwrap());
            match rx.recv_timeout(Duration::from_millis(100)) {
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                _ => panic!("solve did not remain in flight"),
            }
            let start = Instant::now();
            token.cancel();
            let result = rx.recv_timeout(Duration::from_secs(5)).unwrap();
            let latency = start.elapsed();
            assert_eq!(result.unwrap_err().kind, FailureKind::Cancelled);
            observer.join().unwrap();
            latency
        });
        println!(
            "NATURAL_CANCEL_PASS trial={trial} latency_ns={}",
            latency.as_nanos()
        );
    }
    let fresh = CancellationToken::new();
    let recovered = solve_file_cancellable(&path, options, &fresh).unwrap();
    assert_eq!(baseline.sat, recovered.sat);
    assert_eq!(baseline.route, recovered.route);
    assert_eq!(baseline.real_mpfr().binary(), recovered.real_mpfr().binary());
    assert_eq!(baseline.imag_mpfr().binary(), recovered.imag_mpfr().binary());
    println!("s SATISFIABLE");
    println!("c r mpfr {} {}", recovered.real, recovered.imag);
}
