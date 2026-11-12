//! Read-only Linux cgroup v2 interface discovery plus a reversible,
//! empty-subgroup permission probe. NOT memory enforcement, OOM causality
//! or production worker isolation. No process is ever moved into a cgroup.

#[cfg(target_os = "linux")]
mod linux {
    use std::collections::HashSet;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct MemoryEvents {
        oom_kill: u64,
    }

    fn parse_events(input: &str) -> Result<MemoryEvents, String> {
        let mut seen = HashSet::new();
        let mut oom_kill = None;
        for line in input.lines() {
            let mut columns = line.split_whitespace();
            let name = columns.next().ok_or("empty memory.events line")?;
            let count = columns
                .next()
                .ok_or_else(|| format!("missing value for {name}"))?
                .parse::<u64>()
                .map_err(|e| format!("invalid unsigned event count for {name}: {e}"))?;
            if columns.next().is_some() || !seen.insert(name) {
                return Err(format!("duplicate or malformed memory.events field {name}"));
            }
            if name == "oom_kill" {
                oom_kill = Some(count);
            }
        }
        Ok(MemoryEvents {
            oom_kill: oom_kill.ok_or("missing oom_kill counter")?,
        })
    }

    fn observed_delta(before: MemoryEvents, after: MemoryEvents) -> Result<u64, String> {
        after
            .oom_kill
            .checked_sub(before.oom_kill)
            .ok_or_else(|| "oom_kill counter moved backwards (nonmatching cgroup or reset)".into())
    }

    fn parser_negative_controls() -> Result<(), String> {
        let first = parse_events("low 0\nhigh 0\nmax 0\noom 1\noom_kill 2\n")?;
        let second = parse_events("low 0\nhigh 0\nmax 3\noom 1\noom_kill 4\n")?;
        if observed_delta(first, second)? != 2 {
            return Err("expected strictly observed oom_kill delta=2".into());
        }
        if observed_delta(second, first).is_ok() {
            return Err("counter regression incorrectly certified".into());
        }
        for invalid in [
            "oom 0\n",
            "oom_kill -1\n",
            "oom_kill 0\noom_kill 1\n",
            "oom_kill 0 extra\n",
        ] {
            if parse_events(invalid).is_ok() {
                return Err(format!("malformed event fixture accepted: {invalid:?}"));
            }
        }
        // A job exit code of 42 is never sufficient evidence of cgroup
        // memory.events attribution; this parser only reads event counters.
        println!("CGROUP_V2_EVENT_PARSER_NEGATIVE_CONTROLS=PASS");
        println!("CGROUP_V2_EVENT_DELTA_FIXTURE=PASS");
        Ok(())
    }

    fn mounted_at_default_root() -> bool {
        let Ok(input) = fs::read_to_string("/proc/self/mountinfo") else {
            return false;
        };
        input.lines().any(|line| {
            let Some((before, after)) = line.split_once(" - ") else {
                return false;
            };
            before.split_whitespace().nth(4) == Some("/sys/fs/cgroup")
                && after.split_whitespace().next() == Some("cgroup2")
        })
    }

    fn membership_observed() -> bool {
        let Ok(input) = fs::read_to_string("/proc/self/cgroup") else {
            return false;
        };
        input.lines().any(|line| line.starts_with("0::/"))
    }

    fn subgroup_create_probe(root: &Path) -> Result<bool, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("clock: {e}"))?
            .as_nanos();
        let directory = root.join(format!("rawmocs-v2-probe-{}-{stamp}", std::process::id()));
        match fs::create_dir(&directory) {
            Ok(()) => {
                // Never migrate a PID or change subtree_control/memory.max.
                // The empty directory must be removed even if the check
                // detects no controller delegation.
                fs::remove_dir(&directory).map_err(|e| {
                    format!("failed to remove short-lived probe cgroup {directory:?}: {e}")
                })?;
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    fn read_memory_events(path: &Path) -> bool {
        match fs::read_to_string(path.join("memory.events")) {
            Ok(contents) => parse_events(&contents).is_ok(),
            Err(_) => false,
        }
    }

    fn probe() -> Result<(), String> {
        parser_negative_controls()?;
        let root = Path::new("/sys/fs/cgroup");
        let visible = mounted_at_default_root() && membership_observed();
        println!(
            "CGROUP_V2_INTERFACE={}",
            if visible { "VISIBLE" } else { "UNAVAILABLE" }
        );
        let readable = visible && read_memory_events(root);
        println!(
            "CGROUP_V2_MEMORY_EVENTS={}",
            if readable { "READABLE" } else { "UNAVAILABLE" }
        );
        let enabled = visible
            && fs::read_to_string(root.join("cgroup.subtree_control"))
                .map(|text| text.split_whitespace().any(|name| name == "memory"))
                .unwrap_or(false);
        println!(
            "CGROUP_V2_MEMORY_SUBTREE={}",
            if !visible {
                "UNAVAILABLE"
            } else if enabled {
                "ENABLED"
            } else {
                "NOT_ENABLED"
            }
        );
        let can_create = if visible {
            subgroup_create_probe(root)?
        } else {
            false
        };
        println!(
            "CGROUP_V2_SUBGROUP_CREATE={}",
            if can_create {
                "AVAILABLE"
            } else {
                "UNAVAILABLE"
            }
        );
        // Readable events or a creatable empty directory do not prove that
        // a worker can be migrated, capped or that OOM was caused by it.
        println!("CGROUP_V2_MEMORY_ENFORCEMENT=NOT_VALIDATED");
        println!("CGROUP_V2_OOM_KILL_ATTRIBUTION=NOT_VALIDATED");
        println!("CGROUP_V2_NO_PROCS_MOVED=PASS");
        println!("CGROUP_V2_CAPABILITY_PROBE_CONFORMANCE=PASS");
        Ok(())
    }

    pub fn run(args: &[String]) -> Result<(), String> {
        if args.len() == 2 && args[1] == "--self-test" {
            probe()
        } else {
            Err("usage: linux_cgroup_probe --self-test".into())
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn unsupported(args: &[String]) -> Result<(), String> {
    if args.len() != 2 || args[1] != "--self-test" {
        return Err("usage: linux_cgroup_probe --self-test".into());
    }
    println!("CGROUP_V2_INTERFACE=UNSUPPORTED");
    println!("CGROUP_V2_MEMORY_EVENTS=UNSUPPORTED");
    println!("CGROUP_V2_CAPABILITY_PROBE_CONFORMANCE=UNSUPPORTED");
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    #[cfg(target_os = "linux")]
    let result = linux::run(&args);
    #[cfg(not(target_os = "linux"))]
    let result = unsupported(&args);
    if let Err(e) = result {
        eprintln!("CGROUP_V2_CAPABILITY_PROBE_FAIL: {e}");
        std::process::exit(1);
    }
}
