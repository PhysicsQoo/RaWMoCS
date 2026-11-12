//! Opt-in, isolated Linux cgroup v2 worker-memory experiment.
//! Default --self-test is read-only and runs on every platform. The
//! --delegated-run path requires an explicitly delegated non-root subtree;
//! it is not the production solver supervisor or an end-to-end WMC oracle.

#[cfg(target_os = "linux")]
mod linux {
    use std::collections::HashSet;
    use std::fs;
    use std::hint::black_box;
    use std::io::{Read, Write};
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, ExitStatus, Stdio};
    use std::thread;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    const LIMIT_BYTES: u64 = 64 * 1024 * 1024;
    const SMALL_BYTES: usize = 4 * 1024 * 1024;
    const LARGE_BYTES: usize = 192 * 1024 * 1024;
    const WORKER_DEADLINE: Duration = Duration::from_secs(8);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Events {
        max: u64,
        oom: u64,
        oom_kill: u64,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Verdict {
        Completed,
        LocalCgroupOomEvidence,
        UnknownWorkerFailure,
    }

    fn parse_events(source: &str) -> Result<Events, String> {
        let mut seen = HashSet::new();
        let (mut max, mut oom, mut oom_kill) = (None, None, None);
        for line in source.lines() {
            let mut parts = line.split_whitespace();
            let key = parts.next().ok_or("empty event line")?;
            let val = parts
                .next()
                .ok_or_else(|| format!("missing event count: {key}"))?;
            if parts.next().is_some() || !seen.insert(key) {
                return Err(format!("duplicate or malformed event: {key}"));
            }
            let parsed = val
                .parse::<u64>()
                .map_err(|e| format!("invalid event count {key}: {e}"))?;
            match key {
                "max" => max = Some(parsed),
                "oom" => oom = Some(parsed),
                "oom_kill" => oom_kill = Some(parsed),
                _ => {}
            }
        }
        Ok(Events {
            max: max.ok_or("missing max event")?,
            oom: oom.ok_or("missing oom event")?,
            oom_kill: oom_kill.ok_or("missing oom_kill event")?,
        })
    }

    fn delta(before: Events, after: Events) -> Result<Events, String> {
        Ok(Events {
            max: after.max.checked_sub(before.max).ok_or("max regressed")?,
            oom: after.oom.checked_sub(before.oom).ok_or("oom regressed")?,
            oom_kill: after
                .oom_kill
                .checked_sub(before.oom_kill)
                .ok_or("oom_kill regressed")?,
        })
    }

    fn classify(success: bool, over_limit: bool, change: Events) -> Verdict {
        if !over_limit && success && change.oom_kill == 0 {
            Verdict::Completed
        } else if over_limit && !success && change.max > 0 && change.oom > 0 && change.oom_kill > 0
        {
            Verdict::LocalCgroupOomEvidence
        } else {
            Verdict::UnknownWorkerFailure
        }
    }

    fn contract_self_test() -> Result<(), String> {
        let before = parse_events("low 0\nmax 1\noom 2\noom_kill 3\n")?;
        let after = parse_events("max 4\noom 5\noom_kill 6\n")?;
        let changed = delta(before, after)?;
        let expected_change = Events {
            max: 3,
            oom: 3,
            oom_kill: 3,
        };
        if changed != expected_change {
            return Err("incorrect event delta".into());
        }
        let no_event = Events {
            max: 0,
            oom: 0,
            oom_kill: 0,
        };
        let ambiguous_exit = Events {
            max: 1,
            oom: 1,
            oom_kill: 0,
        };
        if classify(true, false, no_event) != Verdict::Completed
            || classify(false, true, changed) != Verdict::LocalCgroupOomEvidence
            || classify(false, true, ambiguous_exit) != Verdict::UnknownWorkerFailure
            || classify(true, true, changed) != Verdict::UnknownWorkerFailure
        {
            return Err("fail-closed worker evidence classification mismatch".into());
        }
        for malformed in [
            "oom_kill 1\n",
            "max 1\noom 1\noom_kill -1\n",
            "max 1\noom 1\noom_kill 0\noom_kill 1\n",
            "max 1\noom 1\noom_kill 0 extra\n",
        ] {
            if parse_events(malformed).is_ok() {
                return Err(format!("accepted malformed local events: {malformed:?}"));
            }
        }
        if delta(after, before).is_ok() {
            return Err("accepted regressing kernel counters".into());
        }
        println!("DELEGATED_CGROUP_EVENT_NEGATIVE_CONTROLS=PASS");
        println!("DELEGATED_CGROUP_LOCAL_OOM_CLASSIFICATION=PASS");
        if validate_delegated_path(Path::new("/sys/fs/cgroup")).is_ok() {
            return Err("unsafe mount-root delegation accepted".into());
        }
        println!("DELEGATED_CGROUP_MOUNT_ROOT_REFUSAL=PASS");
        println!("DELEGATED_CGROUP_REAL_ENFORCEMENT=NOT_RUN");
        println!("DELEGATED_CGROUP_NO_KERNEL_WRITES=PASS");
        println!("DELEGATED_CGROUP_CONTRACT_SELF_TEST=PASS");
        Ok(())
    }

    fn validate_delegated_path(root: &Path) -> Result<PathBuf, String> {
        let mount = fs::canonicalize("/sys/fs/cgroup")
            .map_err(|e| format!("cgroup mount unavailable: {e}"))?;
        let root =
            fs::canonicalize(root).map_err(|e| format!("delegated subtree unavailable: {e}"))?;
        if root == mount || !root.starts_with(&mount) {
            return Err(
                "RESOURCE_LIMIT_UNAVAILABLE: explicit non-root cgroup subtree required".into(),
            );
        }
        let subtree = fs::read_to_string(root.join("cgroup.subtree_control"))
            .map_err(|e| format!("RESOURCE_LIMIT_UNAVAILABLE: subtree_control: {e}"))?;
        if !subtree.split_whitespace().any(|s| s == "memory") {
            return Err(
                "RESOURCE_LIMIT_UNAVAILABLE: memory controller not enabled for children".into(),
            );
        }
        Ok(root)
    }

    struct OwnedGroup {
        path: PathBuf,
        child: Option<Child>,
        removed: bool,
    }

    impl OwnedGroup {
        fn create(parent: &Path, label: &str) -> Result<Self, String> {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("clock: {e}"))?
                .as_nanos();
            let path = parent.join(format!(
                "rawmocs-memory-{}-{stamp}-{label}",
                std::process::id()
            ));
            fs::create_dir(&path).map_err(|e| {
                format!("RESOURCE_LIMIT_UNAVAILABLE: cannot create owned subgroup: {e}")
            })?;
            Ok(Self {
                path,
                child: None,
                removed: false,
            })
        }

        fn set_limit(&self) -> Result<(), String> {
            fs::write(self.path.join("memory.max"), LIMIT_BYTES.to_string())
                .map_err(|e| format!("RESOURCE_LIMIT_UNAVAILABLE: memory.max write: {e}"))?;
            let actual = fs::read_to_string(self.path.join("memory.max"))
                .map_err(|e| format!("RESOURCE_LIMIT_UNAVAILABLE: memory.max readback: {e}"))?;
            if actual.trim() != LIMIT_BYTES.to_string() {
                return Err("RESOURCE_LIMIT_UNAVAILABLE: memory.max readback mismatch".into());
            }
            Ok(())
        }

        fn events(&self) -> Result<Events, String> {
            let input = fs::read_to_string(self.path.join("memory.events.local"))
                .map_err(|e| format!("RESOURCE_LIMIT_UNAVAILABLE: local memory events: {e}"))?;
            parse_events(&input)
        }

        fn admit(&mut self, mode: &str) -> Result<(), String> {
            let executable = std::env::current_exe().map_err(|e| format!("current exe: {e}"))?;
            let child = Command::new(executable)
                .arg("--worker")
                .arg(mode)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| format!("gated worker spawn: {e}"))?;
            let pid = child.id();
            self.child = Some(child);
            // No worker allocation is allowed after the stdin gate until
            // the process is verified in its owned, capped leaf cgroup.
            fs::write(self.path.join("cgroup.procs"), format!("{pid}\n"))
                .map_err(|e| format!("RESOURCE_LIMIT_UNAVAILABLE: cgroup.procs admission: {e}"))?;
            let membership = fs::read_to_string(self.path.join("cgroup.procs"))
                .map_err(|e| format!("RESOURCE_LIMIT_UNAVAILABLE: cgroup.procs readback: {e}"))?;
            if !membership.lines().any(|s| s.trim() == pid.to_string()) {
                return Err("RESOURCE_LIMIT_UNAVAILABLE: worker PID membership unverified".into());
            }
            let child = self.child.as_mut().ok_or("worker ownership lost")?;
            let mut stdin = child.stdin.take().ok_or("worker gate missing")?;
            stdin
                .write_all(&[1])
                .map_err(|e| format!("worker gate release: {e}"))?;
            Ok(())
        }

        fn wait_direct(&mut self) -> Result<ExitStatus, String> {
            let started = Instant::now();
            loop {
                let child = self.child.as_mut().ok_or("worker ownership lost")?;
                if let Some(status) = child.try_wait().map_err(|e| format!("worker wait: {e}"))? {
                    self.child.take();
                    return Ok(status);
                }
                if started.elapsed() > WORKER_DEADLINE {
                    return Err("TIMEOUT: worker was not reaped before deadline".into());
                }
                thread::sleep(Duration::from_millis(10));
            }
        }

        fn finish(&mut self) -> Result<(), String> {
            if self.child.is_some() {
                return Err("worker still owned at cgroup cleanup".into());
            }
            let since = Instant::now();
            loop {
                let state = fs::read_to_string(self.path.join("cgroup.events"))
                    .map_err(|e| format!("cgroup.events verification: {e}"))?;
                if state.lines().any(|line| line.trim() == "populated 0") {
                    break;
                }
                if since.elapsed() > Duration::from_secs(3) {
                    return Err("cgroup remains populated after direct worker completion".into());
                }
                thread::sleep(Duration::from_millis(20));
            }
            fs::remove_dir(&self.path).map_err(|e| format!("owned cgroup removal failed: {e}"))?;
            self.removed = true;
            Ok(())
        }
    }

    impl Drop for OwnedGroup {
        fn drop(&mut self) {
            if self.removed {
                return;
            }
            // This path was uniquely created by this process. Only that
            // path may be targeted by cgroup.kill; never parent/global root.
            if self.child.is_some() {
                let _ = fs::write(self.path.join("cgroup.kill"), "1");
            }
            if let Some(mut child) = self.child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
            let _ = fs::remove_dir(&self.path);
        }
    }

    fn run_case(parent: &Path, mode: &str, over: bool) -> Result<(), String> {
        let mut owned = OwnedGroup::create(parent, mode)?;
        owned.set_limit()?;
        let baseline = owned.events()?;
        owned.admit(mode)?;
        let status = owned.wait_direct()?;
        let observed = owned.events()?;
        let change = delta(baseline, observed)?;
        let verdict = classify(status.success(), over, change);
        println!(
            "DELEGATED_{mode}_COUNTERS=max:{},oom:{},oom_kill:{}",
            change.max, change.oom, change.oom_kill
        );
        owned.finish()?;
        let target = if over {
            Verdict::LocalCgroupOomEvidence
        } else {
            Verdict::Completed
        };
        if verdict != target {
            return Err(format!(
                "{mode}: insufficient evidence: verdict={verdict:?}, exit={status}"
            ));
        }
        println!("DELEGATED_{mode}_WORKER_MEMBERSHIP_AND_CLEANUP=PASS");
        Ok(())
    }

    fn delegated_run(path: &Path) -> Result<(), String> {
        // This is a voluntarily invoked administrative experiment, never
        // part of a normal GitHub-hosted CI workflow or solver path.
        let parent = validate_delegated_path(path)?;
        run_case(&parent, "under", false)?;
        run_case(&parent, "over", true)?;
        println!("DELEGATED_CGROUP_REAL_ENFORCEMENT=OBSERVED");
        Ok(())
    }

    fn worker(mode: &str) -> Result<(), String> {
        let mut gate = [0u8];
        std::io::stdin()
            .read_exact(&mut gate)
            .map_err(|e| format!("worker not admitted: {e}"))?;
        if gate != [1] {
            return Err("invalid worker gate".into());
        }
        let bytes = match mode {
            "under" => SMALL_BYTES,
            "over" => LARGE_BYTES,
            _ => return Err("unrecognized memory workload".into()),
        };
        let mut memory = vec![0u8; bytes];
        for i in (0..bytes).step_by(4096) {
            memory[i] = (i / 4096) as u8;
        }
        black_box(&memory);
        Ok(())
    }

    pub fn entry(args: &[String]) -> Result<(), String> {
        match args {
            [_, flag] if flag == "--self-test" => contract_self_test(),
            [_, flag, root] if flag == "--delegated-run" => delegated_run(Path::new(root)),
            [_, flag, mode] if flag == "--worker" => worker(mode),
            _ => Err(
                "usage: delegated_cgroup_contract --self-test | --delegated-run DELEGATED_SUBTREE"
                    .into(),
            ),
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn unsupported(args: &[String]) -> Result<(), String> {
    if args.len() != 2 || args[1] != "--self-test" {
        return Err("Linux cgroup v2 is UNSUPPORTED on this platform".into());
    }
    println!("DELEGATED_CGROUP_CONTRACT_SELF_TEST=UNSUPPORTED");
    println!("DELEGATED_CGROUP_REAL_ENFORCEMENT=UNSUPPORTED");
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    #[cfg(target_os = "linux")]
    let result = linux::entry(&args);
    #[cfg(not(target_os = "linux"))]
    let result = unsupported(&args);
    if let Err(error) = result {
        eprintln!("DELEGATED_CGROUP_FAIL: {error}");
        std::process::exit(1);
    }
}
