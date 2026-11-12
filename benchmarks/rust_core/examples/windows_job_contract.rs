//! Windows Job Object lifecycle research for a controlled worker whose child
//! inherits job membership. This is not the production WMC supervisor.
//! Non-Windows platforms explicitly report UNSUPPORTED, not a false PASS.

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;
    use std::fs;
    use std::io::{Read, Write};
    use std::os::windows::io::AsRawHandle;
    use std::path::Path;
    use std::process::{Child, Command, Stdio};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    type Handle = *mut c_void;
    const KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;
    const EXTENDED_LIMIT_INFORMATION: i32 = 9;

    // Win32 layouts from winnt.h: integer and pointer-size alignment matter.
    #[repr(C)]
    #[derive(Default)]
    struct BasicLimit {
        process_user_time: i64,
        job_user_time: i64,
        flags: u32,
        minimum_working_set: usize,
        maximum_working_set: usize,
        active_processes: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct IoCounters {
        read_ops: u64,
        write_ops: u64,
        other_ops: u64,
        read_bytes: u64,
        write_bytes: u64,
        other_bytes: u64,
    }

    #[repr(C)]
    #[derive(Default)]
    struct ExtendedLimit {
        basic: BasicLimit,
        io: IoCounters,
        process_memory: usize,
        job_memory: usize,
        peak_process_memory: usize,
        peak_job_memory: usize,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateJobObjectW(attributes: *mut c_void, name: *const u16) -> Handle;
        fn SetInformationJobObject(
            job: Handle,
            info_class: i32,
            data: *const c_void,
            length: u32,
        ) -> i32;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
        fn TerminateJobObject(job: Handle, exit_code: u32) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
        fn GetLastError() -> u32;
    }

    fn win_error(message: &str) -> String {
        let code = unsafe { GetLastError() };
        format!("{message}: Win32 error {code}")
    }

    struct Job(Handle);

    impl Job {
        fn create() -> Result<Self, String> {
            let handle = unsafe { CreateJobObjectW(std::ptr::null_mut(), std::ptr::null()) };
            if handle.is_null() {
                return Err(win_error("CreateJobObjectW failed"));
            }
            let job = Self(handle);
            let mut limits = ExtendedLimit::default();
            limits.basic.flags = KILL_ON_JOB_CLOSE;
            let length = u32::try_from(std::mem::size_of::<ExtendedLimit>())
                .map_err(|_| "extended job structure exceeds Win32 size".to_string())?;
            let result = unsafe {
                SetInformationJobObject(
                    job.0,
                    EXTENDED_LIMIT_INFORMATION,
                    (&limits as *const ExtendedLimit).cast(),
                    length,
                )
            };
            if result == 0 {
                return Err(win_error("SetInformationJobObject kill-on-close failed"));
            }
            Ok(job)
        }

        fn assign(&self, process: &Child) -> Result<(), String> {
            let handle = process.as_raw_handle() as Handle;
            let result = unsafe { AssignProcessToJobObject(self.0, handle) };
            if result == 0 {
                // Nested Job policies / access denied are an explicit failure,
                // not a claim that descendants were contained.
                return Err(win_error("AssignProcessToJobObject failed"));
            }
            Ok(())
        }

        fn terminate(&self) -> Result<(), String> {
            let result = unsafe { TerminateJobObject(self.0, 1) };
            if result == 0 {
                return Err(win_error("TerminateJobObject failed"));
            }
            Ok(())
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // KILL_ON_JOB_CLOSE is set. This is only the process group's
            // lifetime guarantee, not a hard RSS memory policy.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }

    struct Owned {
        job: Option<Job>,
        child: Option<Child>,
    }

    impl Owned {
        fn new(job: Job, child: Child) -> Self {
            Self {
                job: Some(job),
                child: Some(child),
            }
        }

        fn start(&mut self) -> Result<(), String> {
            // Assign BEFORE unblocking the child. The child cannot create
            // its grandchild before receiving the one-byte start token.
            self.job.as_ref().unwrap().assign(self.child.as_ref().unwrap())?;
            let mut stdin = self
                .child
                .as_mut()
                .unwrap()
                .stdin
                .take()
                .ok_or_else(|| "gated child stdin missing".to_string())?;
            stdin
                .write_all(&[1])
                .map_err(|e| format!("send worker start token: {e}"))?;
            Ok(())
        }

        fn running(&mut self) -> Result<(), String> {
            if let Some(status) = self.child.as_mut().unwrap().try_wait().map_err(|e| e.to_string())? {
                return Err(format!("direct worker exited before explicit stop: {status}"));
            }
            Ok(())
        }

        fn stop(&mut self, mode: &str) -> Result<(), String> {
            match mode {
                "TIMEOUT" => self.job.as_ref().unwrap().terminate()?,
                "CANCELLED" => {
                    // Dropping the last Job handle triggers KILL_ON_JOB_CLOSE.
                    self.job.take();
                }
                _ => return Err("unknown controlled stop mode".into()),
            }
            let started = Instant::now();
            loop {
                if self.child.as_mut().unwrap().try_wait().map_err(|e| e.to_string())?.is_some() {
                    self.child.take();
                    return Ok(());
                }
                if started.elapsed() > Duration::from_secs(3) {
                    return Err("job signalled but direct child did not exit".into());
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
    }

    impl Drop for Owned {
        fn drop(&mut self) {
            // Close the job before falling back to direct-child cleanup.
            self.job.take();
            if let Some(mut child) = self.child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    fn heartbeat(path: &Path) -> Result<(), String> {
        let mut count = 0u64;
        loop {
            count = count.saturating_add(1);
            fs::write(path, count.to_string()).map_err(|e| format!("heartbeat write: {e}"))?;
            thread::sleep(Duration::from_millis(25));
        }
    }

    fn worker(path: &Path) -> Result<(), String> {
        let mut token = [0u8];
        std::io::stdin()
            .read_exact(&mut token)
            .map_err(|e| format!("waiting for gated Job assignment: {e}"))?;
        if token != [1] {
            return Err("invalid job-assignment start token".into());
        }
        let exe = std::env::current_exe().map_err(|e| format!("own binary: {e}"))?;
        // A descendant of a process already assigned to the Job inherits
        // its job membership by default, absent explicit breakaway.
        let child = Command::new(exe)
            .arg("--descendant")
            .arg(path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("descendant spawn: {e}"))?;
        drop(child);
        loop {
            thread::sleep(Duration::from_millis(100));
        }
    }

    fn wait_live(path: &Path, owner: &mut Owned) -> Result<(), String> {
        let since = Instant::now();
        loop {
            owner.running()?;
            if let Ok(text) = fs::read_to_string(path) {
                if let Ok(n) = text.parse::<u64>() {
                    if n >= 3 {
                        return Ok(());
                    }
                }
            }
            if since.elapsed() > Duration::from_secs(5) {
                return Err("gated Job descendant never produced live heartbeat".into());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn case(exe: &Path, label: &str) -> Result<(), String> {
        let path = std::env::temp_dir().join(format!(
            "rawmocs-winjob-{}-{label}.heartbeat",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        let job = Job::create()?;
        let child = Command::new(exe)
            .arg("--worker")
            .arg(&path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("gated worker spawn: {e}"))?;
        let mut owner = Owned::new(job, child);
        owner.start()?;
        wait_live(&path, &mut owner)?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal = if label == "CANCELLED" {
            let flag = Arc::clone(&cancelled);
            Some(thread::spawn(move || {
                thread::sleep(Duration::from_millis(60));
                flag.store(true, Ordering::SeqCst);
            }))
        } else {
            None
        };
        let since = Instant::now();
        let verdict = loop {
            owner.running()?;
            if cancelled.load(Ordering::SeqCst) {
                break "CANCELLED";
            }
            if since.elapsed() > Duration::from_millis(130) {
                break "TIMEOUT";
            }
            thread::sleep(Duration::from_millis(5));
        };
        if let Some(thread) = signal {
            thread.join().map_err(|_| "cancel flag thread panic".to_string())?;
        }
        if verdict != label {
            return Err(format!("unexpected {verdict} for {label}"));
        }
        owner.stop(verdict)?;
        // Allow any in-flight write to settle before checking stability.
        thread::sleep(Duration::from_millis(50));
        let before = fs::read_to_string(&path)
            .map_err(|e| format!("post Job-stop heartbeat: {e}"))?;
        thread::sleep(Duration::from_millis(230));
        let after = fs::read_to_string(&path)
            .map_err(|e| format!("heartbeat recheck: {e}"))?;
        let _ = fs::remove_file(&path);
        if before != after {
            return Err(format!("{label}: descendant continued after Job termination"));
        }
        println!("WINDOWS_JOB_{label}=PASS");
        println!("WINDOWS_JOB_LIVE_DESCENDANT_{label}=PASS");
        Ok(())
    }

    pub fn dispatch(args: &[String]) -> Result<(), String> {
        match args {
            [_, flag] if flag == "--self-test" => {
                let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                case(&exe, "TIMEOUT")?;
                case(&exe, "CANCELLED")?;
                println!("CONTRACT_WINDOWS_JOB_SCOPED_DESCENDANTS=PASS");
                println!("CONTRACT_WINDOWS_HARD_RSS=UNSUPPORTED");
                println!("CONTRACT_WINDOWS_ADVERSARIAL_BREAKAWAY=UNSUPPORTED");
                println!("CONTRACT_LINUX_CGROUP_V2=UNSUPPORTED");
                println!("WINDOWS_JOB_OBJECT_SCOPED_CONFORMANCE=PASS");
                Ok(())
            }
            [_, flag, path] if flag == "--worker" => worker(Path::new(path)),
            [_, flag, path] if flag == "--descendant" => heartbeat(Path::new(path)),
            _ => Err("usage: windows_job_contract --self-test".into()),
        }
    }
}

#[cfg(not(windows))]
fn unsupported(args: &[String]) -> Result<(), String> {
    if args.len() != 2 || args[1] != "--self-test" {
        return Err("usage: windows_job_contract --self-test".into());
    }
    println!("CONTRACT_WINDOWS_JOB_SCOPED_DESCENDANTS=UNSUPPORTED");
    println!("WINDOWS_JOB_OBJECT_SCOPED_CONFORMANCE=UNSUPPORTED");
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    #[cfg(windows)]
    let result = windows::dispatch(&args);
    #[cfg(not(windows))]
    let result = unsupported(&args);
    if let Err(error) = result {
        eprintln!("WINDOWS_JOB_CONTRACT_FAIL: {error}");
        std::process::exit(1);
    }
}
