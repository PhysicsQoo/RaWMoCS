//! Source-only, cross-platform worker outcome/lifecycle conformance probe.
//! This is NOT the production solver supervisor, process-tree isolation,
//! cancellation of arbitrary user code, or a hard physical-memory cap.
//! No external WMC solver or mutable data is involved.

use std::env;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const SOLVER_ERROR_CODE: i32 = 17;
const UNKNOWN_ERROR_CODE: i32 = 29;
const FALSE_MEMORY_CODE: i32 = 42;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkerVerdict {
    Completed,
    SolverError,
    OtherWorkerExit,
    TimedOut,
    Cancelled,
}

/// Owns exactly one child and never drops an unreaped child on a parent
/// error. Does not kill grandchildren: process-tree cleanup is UNSUPPORTED.
struct OwnedChild(Option<Child>);

impl OwnedChild {
    fn new(child: Child) -> Self {
        Self(Some(child))
    }

    fn status(&mut self) -> Result<Option<ExitStatus>, String> {
        self.0
            .as_mut()
            .ok_or_else(|| "worker already reaped".to_string())?
            .try_wait()
            .map_err(|e| format!("worker poll failed: {e}"))
    }

    fn reaped(&mut self) {
        // try_wait already reaped the direct child; no Drop cleanup needed.
        self.0.take();
    }

    fn kill_and_reap(&mut self) -> Result<(), String> {
        let child = self
            .0
            .as_mut()
            .ok_or_else(|| "worker already reaped".to_string())?;
        // A failed kill must not silently certify a timeout or cancellation.
        child
            .kill()
            .map_err(|e| format!("worker kill failed: {e}"))?;
        child
            .wait()
            .map_err(|e| format!("worker reap failed: {e}"))?;
        self.0.take();
        Ok(())
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            // Best-effort cleanup on abnormal parent errors; the caller
            // receives Err and must never accept a solver verdict.
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn classify_exit(status: ExitStatus) -> WorkerVerdict {
    if status.success() {
        WorkerVerdict::Completed
    } else if status.code() == Some(SOLVER_ERROR_CODE) {
        WorkerVerdict::SolverError
    } else {
        // Unknown exit, signal or apparent allocation-denial code must NOT
        // be silently reclassified as MemoryLimit or exact UNSAT.
        WorkerVerdict::OtherWorkerExit
    }
}

fn supervise(
    executable: &Path,
    mode: &str,
    deadline: Duration,
    cancel: &AtomicBool,
) -> Result<WorkerVerdict, String> {
    if cancel.load(Ordering::SeqCst) {
        return Ok(WorkerVerdict::Cancelled);
    }
    let child = Command::new(executable)
        .arg("--worker")
        .arg(mode)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("worker spawn failed: {e}"))?;
    let mut owned = OwnedChild::new(child);
    let start = Instant::now();
    loop {
        // Reap completed workers first: classify an observed exit even if
        // a near-simultaneous cancellation/deadline also becomes true.
        if let Some(status) = owned.status()? {
            owned.reaped();
            return Ok(classify_exit(status));
        }
        if cancel.load(Ordering::SeqCst) {
            owned.kill_and_reap()?;
            return Ok(WorkerVerdict::Cancelled);
        }
        if start.elapsed() >= deadline {
            owned.kill_and_reap()?;
            return Ok(WorkerVerdict::TimedOut);
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn check(
    executable: &Path,
    mode: &str,
    cancel: &AtomicBool,
    duration: Duration,
    expected: WorkerVerdict,
    label: &str,
) -> Result<(), String> {
    let actual = supervise(executable, mode, duration, cancel)?;
    if actual != expected {
        return Err(format!("{label}: expected {expected:?}, got {actual:?}"));
    }
    println!("CONTRACT_{label}=PASS");
    Ok(())
}

fn conformance() -> Result<(), String> {
    let exe = env::current_exe().map_err(|e| format!("worker executable: {e}"))?;
    let flag = AtomicBool::new(false);
    check(
        &exe,
        "success",
        &flag,
        Duration::from_secs(2),
        WorkerVerdict::Completed,
        "COMPLETED",
    )?;
    check(
        &exe,
        "solver-error",
        &flag,
        Duration::from_secs(2),
        WorkerVerdict::SolverError,
        "SOLVER_ERROR",
    )?;
    check(
        &exe,
        "unknown-failure",
        &flag,
        Duration::from_secs(2),
        WorkerVerdict::OtherWorkerExit,
        "UNKNOWN_EXIT",
    )?;
    // An untrusted worker pretending to have an allocation failure cannot
    // prove a hard RSS limit was reached merely by exiting with code 42.
    check(
        &exe,
        "pretend-memory-limit",
        &flag,
        Duration::from_secs(2),
        WorkerVerdict::OtherWorkerExit,
        "UNVERIFIED_MEMORY_EXIT",
    )?;
    check(
        &exe,
        "sleep",
        &flag,
        Duration::from_millis(100),
        WorkerVerdict::TimedOut,
        "TIMEOUT",
    )?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let flip = Arc::clone(&cancelled);
    let trigger = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        flip.store(true, Ordering::SeqCst);
    });
    let result = check(
        &exe,
        "sleep",
        &cancelled,
        Duration::from_secs(2),
        WorkerVerdict::Cancelled,
        "CANCELLED",
    );
    trigger
        .join()
        .map_err(|_| "cancel trigger panicked".to_string())?;
    result?;
    let pre_cancelled = AtomicBool::new(true);
    check(
        &exe,
        "sleep",
        &pre_cancelled,
        Duration::from_secs(1),
        WorkerVerdict::Cancelled,
        "PRE_CANCELLED",
    )?;
    // These capabilities are deliberately NOT implemented or claimed here.
    println!("CONTRACT_HARD_RSS=UNSUPPORTED");
    println!("CONTRACT_PROCESS_TREE=UNSUPPORTED");
    println!("CONTRACT_LINUX_CGROUP_V2=UNSUPPORTED");
    println!("CONTRACT_WINDOWS_JOB_OBJECT=UNSUPPORTED");
    println!("CONTRACT_MACOS_MEMORY_ENFORCEMENT=UNSUPPORTED");
    println!("WORKER_LIFECYCLE_DIRECT_CHILD_CONFORMANCE=PASS");
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let result = match args.as_slice() {
        [_, arg] if arg == "--self-test" => conformance(),
        [_, cmd, mode] if cmd == "--worker" => match mode.as_str() {
            "success" => Ok(()),
            "solver-error" => std::process::exit(SOLVER_ERROR_CODE),
            "unknown-failure" => std::process::exit(UNKNOWN_ERROR_CODE),
            "pretend-memory-limit" => std::process::exit(FALSE_MEMORY_CODE),
            "sleep" => {
                thread::sleep(Duration::from_secs(3));
                Ok(())
            }
            _ => Err("unknown direct-child mode".to_string()),
        },
        _ => Err("usage: worker_contract --self-test".to_string()),
    };
    if let Err(error) = result {
        eprintln!("WORKER_LIFECYCLE_CONTRACT_FAIL: {error}");
        std::process::exit(1);
    }
}
