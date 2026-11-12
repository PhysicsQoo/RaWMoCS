//! Research-only descendant cleanup conformance for a dedicated Unix process
//! group. NOT a production solver supervisor, sandbox or memory limit.
//! On Windows the supported contract here is explicitly UNSUPPORTED.

#[cfg(unix)]
mod unix {
    use std::env;
    use std::fs;
    use std::os::unix::process::CommandExt;
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    const SIGKILL: i32 = 9;

    // Stable Rust CommandExt::process_group(0) invokes setpgid for the
    // spawned worker. The only platform FFI is the POSIX signal / PGID API.
    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
        fn getpgid(pid: i32) -> i32;
    }

    fn group_id() -> i32 {
        // getpgid(0) queries the current process; no external shell needed.
        unsafe { getpgid(0) }
    }

    fn kill_group(pgid: i32) -> Result<(), String> {
        // Never target group 0, the caller's process group or an unverified
        // identifier. The dedicated child PGID equals its own PID.
        if pgid <= 1 || pgid == group_id() {
            return Err("unsafe or non-isolated worker process group".into());
        }
        let result = unsafe { kill(-pgid, SIGKILL) };
        if result == -1 {
            return Err(format!(
                "dedicated process-group signal failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    /// The owner retains the direct child and dedicated PGID; on unexpected
    /// failures Drop attempts group termination and reaps the direct child.
    /// Grandchildren cannot be wait()-reaped by this parent.
    struct GroupOwner {
        child: Option<Child>,
        pgid: i32,
    }

    impl GroupOwner {
        fn new(child: Child) -> Result<Self, String> {
            let pgid = i32::try_from(child.id())
                .map_err(|_| "worker PID does not fit POSIX PGID".to_string())?;
            if pgid <= 1 || pgid == group_id() {
                return Err("invalid or shared worker process group".into());
            }
            Ok(Self {
                child: Some(child),
                pgid,
            })
        }

        fn assert_running(&mut self) -> Result<(), String> {
            let child = self.child.as_mut().ok_or("direct child already reaped")?;
            if let Some(status) = child.try_wait().map_err(|e| format!("poll: {e}"))? {
                return Err(format!(
                    "worker exited before deadline/cancellation: {status}"
                ));
            }
            Ok(())
        }

        fn terminate(&mut self) -> Result<(), String> {
            // Signal the entire verified dedicated group, rather than only
            // calling Child::kill (which leaves ordinary descendants alive).
            kill_group(self.pgid)?;
            let child = self.child.as_mut().ok_or("direct child already reaped")?;
            child.wait().map_err(|e| format!("direct child reap: {e}"))?;
            self.child.take();
            Ok(())
        }
    }

    impl Drop for GroupOwner {
        fn drop(&mut self) {
            if let Some(mut child) = self.child.take() {
                let _ = kill_group(self.pgid);
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    fn grandchild(heartbeat: &Path, expected_pgid: i32) -> Result<(), String> {
        if group_id() != expected_pgid {
            return Err(format!(
                "grandchild escaped the dedicated group: actual={}, expected={expected_pgid}",
                group_id()
            ));
        }
        let mut tick = 0u64;
        loop {
            tick = tick.saturating_add(1);
            fs::write(heartbeat, tick.to_string())
                .map_err(|e| format!("heartbeat write: {e}"))?;
            thread::sleep(Duration::from_millis(25));
        }
    }

    fn worker(heartbeat: &Path) -> Result<(), String> {
        let pgid = group_id();
        if pgid != i32::try_from(std::process::id()).unwrap_or(-1) {
            return Err("dedicated group worker PGID does not match PID".into());
        }
        let exe = env::current_exe().map_err(|e| format!("own binary: {e}"))?;
        let child = Command::new(exe)
            .arg("--grandchild")
            .arg(heartbeat)
            .arg(pgid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("grandchild spawn: {e}"))?;
        // A descendant intentionally remains alive while the direct worker
        // loops. It inherits the isolated PGID, but is not this test's child.
        drop(child);
        loop {
            thread::sleep(Duration::from_millis(100));
        }
    }

    fn wait_heartbeat(path: &Path, owner: &mut GroupOwner) -> Result<u64, String> {
        let started = Instant::now();
        loop {
            owner.assert_running()?;
            if let Ok(s) = fs::read_to_string(path) {
                if let Ok(value) = s.parse::<u64>() {
                    if value >= 3 {
                        return Ok(value);
                    }
                }
            }
            if started.elapsed() > Duration::from_secs(5) {
                return Err("grandchild heartbeat never became live".into());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    enum Stop {
        Timeout,
        Cancel,
    }

    fn case(exe: &Path, label: &str, stop: Stop) -> Result<(), String> {
        let heartbeat = env::temp_dir().join(format!(
            "rawmocs-group-{}-{label}.heartbeat",
            std::process::id()
        ));
        let _ = fs::remove_file(&heartbeat);
        let mut cmd = Command::new(exe);
        cmd.arg("--worker")
            .arg(&heartbeat)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        let child = cmd.spawn().map_err(|e| format!("worker spawn: {e}"))?;
        let mut owner = GroupOwner::new(child)?;
        let initial_tick = wait_heartbeat(&heartbeat, &mut owner)?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let trigger = if matches!(stop, Stop::Cancel) {
            let flag = Arc::clone(&cancelled);
            Some(thread::spawn(move || {
                thread::sleep(Duration::from_millis(65));
                flag.store(true, Ordering::SeqCst);
            }))
        } else {
            None
        };
        let started = Instant::now();
        let verdict = loop {
            owner.assert_running()?;
            if cancelled.load(Ordering::SeqCst) {
                break "CANCELLED";
            }
            if started.elapsed() > Duration::from_millis(130) {
                break "TIMEOUT";
            }
            thread::sleep(Duration::from_millis(5));
        };
        owner.terminate()?;
        if let Some(trigger) = trigger {
            trigger
                .join()
                .map_err(|_| "cancel trigger thread panicked".to_string())?;
        }
        let expected = match stop {
            Stop::Timeout => "TIMEOUT",
            Stop::Cancel => "CANCELLED",
        };
        if verdict != expected {
            return Err(format!("{label}: expected {expected}, got {verdict}"));
        }
        let end = fs::read_to_string(&heartbeat)
            .map_err(|e| format!("post-kill heartbeat read: {e}"))?;
        thread::sleep(Duration::from_millis(200));
        let stable = fs::read_to_string(&heartbeat)
            .map_err(|e| format!("stable heartbeat read: {e}"))?;
        let _ = fs::remove_file(&heartbeat);
        if stable != end {
            return Err(format!(
                "{label}: descendant heartbeat kept advancing after group termination"
            ));
        }
        println!("GROUP_{label}=PASS");
        println!("GROUP_DESCENDANT_WITNESS_{label}=PASS");
        if initial_tick < 3 {
            return Err("missing pre-termination live descendant witness".into());
        }
        Ok(())
    }

    pub fn conformance() -> Result<(), String> {
        let exe = env::current_exe().map_err(|e| format!("current binary: {e}"))?;
        case(&exe, "TIMEOUT", Stop::Timeout)?;
        case(&exe, "CANCELLED", Stop::Cancel)?;
        println!("CONTRACT_UNIX_ISOLATED_PROCESS_GROUP=PASS");
        println!("CONTRACT_ESCAPED_PROCESS_GROUP=UNSUPPORTED");
        println!("CONTRACT_GRANDCHILD_WAIT_REAP=UNSUPPORTED");
        println!("CONTRACT_HARD_RSS=UNSUPPORTED");
        println!("CONTRACT_LINUX_CGROUP_V2=UNSUPPORTED");
        println!("CONTRACT_WINDOWS_JOB_OBJECT=UNSUPPORTED");
        println!("UNIX_PROCESS_GROUP_SCOPED_CONFORMANCE=PASS");
        Ok(())
    }

    pub fn handle(args: &[String]) -> Result<(), String> {
        match args {
            [_, cmd] if cmd == "--self-test" => conformance(),
            [_, cmd, path] if cmd == "--worker" => worker(Path::new(path)),
            [_, cmd, path, pgid] if cmd == "--grandchild" => {
                let pgid = pgid.parse().map_err(|e| format!("invalid PGID: {e}"))?;
                grandchild(Path::new(path), pgid)
            }
            _ => Err("usage: process_group_contract --self-test".into()),
        }
    }
}

#[cfg(not(unix))]
fn unsupported(args: &[String]) -> Result<(), String> {
    if args.len() != 2 || args[1] != "--self-test" {
        return Err("usage: process_group_contract --self-test".into());
    }
    println!("CONTRACT_UNIX_ISOLATED_PROCESS_GROUP=UNSUPPORTED");
    println!("CONTRACT_WINDOWS_JOB_OBJECT=UNSUPPORTED");
    println!("CONTRACT_ESCAPED_PROCESS_GROUP=UNSUPPORTED");
    println!("CONTRACT_HARD_RSS=UNSUPPORTED");
    println!("UNIX_PROCESS_GROUP_SCOPED_CONFORMANCE=UNSUPPORTED");
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    #[cfg(unix)]
    let result = unix::handle(&args);
    #[cfg(not(unix))]
    let result = unsupported(&args);
    if let Err(error) = result {
        eprintln!("PROCESS_GROUP_CONTRACT_FAIL: {error}");
        std::process::exit(1);
    }
}
