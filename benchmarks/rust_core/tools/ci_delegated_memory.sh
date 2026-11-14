#!/usr/bin/env bash
# Trusted-main, GitHub-hosted Ubuntu cgroup v2 kernel qualification.
# Only a systemd-managed *delegated* private transient service receives writes.
# Never mount/remount cgroupfs or mutate the global/root cgroup hierarchy.
set -euo pipefail

mode="${1:-}"
case "$mode" in
  --check)
    # Static gate for PRs. Absolutely no kernel writes or systemd service.
    bash -n "$0"
    printf '%s\n' "DELEGATED_CI_SOURCE_CHECK=PASS" "DELEGATED_CI_REAL_ENFORCEMENT=NOT_RUN"
    exit 0
    ;;
  --run)
    # Do not use sudo for untrusted pull_request code. The workflow itself
    # limits this path to a trusted main-branch push/dispatch.
    [[ "${GITHUB_REF:-}" == "refs/heads/main" ]] || {
      echo "DELEGATED_CI_UNAVAILABLE=NON_MAIN_REF" >&2; exit 2;
    }
    [[ "${GITHUB_EVENT_NAME:-}" != "pull_request" ]] || {
      echo "DELEGATED_CI_UNAVAILABLE=UNTRUSTED_PR_CONTEXT" >&2; exit 2;
    }
    [[ "$(id -u)" -ne 0 ]] || {
      echo "DELEGATED_CI_UNAVAILABLE=ROOT_RUNNER_FORBIDDEN" >&2; exit 2;
    }
    [[ -n "${GITHUB_WORKSPACE:-}" && -n "${RUNNER_TEMP:-}" ]] || {
      echo "DELEGATED_CI_UNAVAILABLE=MISSING_RUNNER_CONTEXT" >&2; exit 2;
    }
    command -v systemd-run >/dev/null || {
      echo "DELEGATED_CI_UNAVAILABLE=SYSTEMD_RUN_MISSING" >&2; exit 2;
    }
    systemctl is-system-running --quiet || {
      # GitHub VM may report "degraded" for unrelated services; systemctl
      # being available and systemd PID 1 are the relevant prerequisites.
      [[ "$(cat /proc/1/comm 2>/dev/null)" == systemd ]] || {
        echo "DELEGATED_CI_UNAVAILABLE=SYSTEMD_NOT_PID1" >&2; exit 2;
      }
    }
    # Build and rustfmt run without privilege. Only the scoped test will be
    # executed inside the new dedicated service.
    cargo build --locked --manifest-path benchmarks/rust_core/Cargo.toml \
      --example delegated_cgroup_contract
    name="rawmocs-memory-${GITHUB_RUN_ID:-0}-${GITHUB_RUN_ATTEMPT:-1}"
    # The only privileged action is to request a dedicated, automatically
    # collected systemd transient unit with specific delegation to runner.
    # Never give the worker sudo. DelegateSubgroup is supported by systemd 254+.
    sudo -n systemd-run --wait --pipe --collect \
      --unit="$name" \
      --property="User=$(id -un)" \
      --property="Delegate=memory" \
      --property="DelegateSubgroup=supervisor" \
      --property="WorkingDirectory=$GITHUB_WORKSPACE" \
      --property="RuntimeMaxSec=180" \
      --property="TasksMax=64" \
      --setenv="HOME=$HOME" \
      --setenv="PATH=$PATH" \
      --setenv="RUSTUP_HOME=${RUSTUP_HOME:-$HOME/.rustup}" \
      --setenv="CARGO_HOME=${CARGO_HOME:-$HOME/.cargo}" \
      --setenv="RUNNER_TEMP=$RUNNER_TEMP" \
      --setenv="GITHUB_WORKSPACE=$GITHUB_WORKSPACE" \
      /usr/bin/bash "$GITHUB_WORKSPACE/benchmarks/rust_core/tools/ci_delegated_memory.sh" --inside-delegated-unit
    ;;
  --inside-delegated-unit)
    [[ "$(id -u)" -ne 0 ]] || {
      echo "DELEGATED_CI_UNAVAILABLE=NONROOT_DELEGATION_NOT_MET" >&2; exit 2;
    }
    [[ -n "${GITHUB_WORKSPACE:-}" && -n "${RUNNER_TEMP:-}" ]] || exit 2
    mountpoint="$(readlink -f /sys/fs/cgroup)"
    membership="$(awk -F: '$1 == "0" && $2 == "" {print $3}' /proc/self/cgroup)"
    # systemd v254+ must have placed the service inside /supervisor, which
    # keeps the delegated boundary itself free of internal processes.
    [[ "$membership" == /system.slice/rawmocs-memory-*.service/supervisor ]] || {
      echo "DELEGATED_CI_UNAVAILABLE=UNEXPECTED_SERVICE_CGROUP" >&2; exit 2;
    }
    subtree="$mountpoint${membership%/supervisor}"
    [[ "$subtree" != "$mountpoint" && "$subtree" == "$mountpoint"/* ]] || exit 2
    [[ -w "$subtree/cgroup.subtree_control" ]] || {
      echo "DELEGATED_CI_UNAVAILABLE=NO_DELEGATION_WRITE" >&2; exit 2;
    }
    grep -qw memory "$subtree/cgroup.controllers" || {
      echo "DELEGATED_CI_UNAVAILABLE=MEMORY_CONTROLLER_MISSING" >&2; exit 2;
    }
    # This write affects ONLY the private transient systemd unit created
    # above, not any parent cgroup or any unrelated process.
    if ! grep -qw memory "$subtree/cgroup.subtree_control"; then
      printf '+memory' > "$subtree/cgroup.subtree_control" || {
        echo "DELEGATED_CI_UNAVAILABLE=CONTROLLER_ENABLE_FAILED" >&2; exit 2;
      }
    fi
    preflight="$RUNNER_TEMP/rawmocs-cgroup-preflight.txt"
    python3 "$GITHUB_WORKSPACE/benchmarks/rust_core/tools/delegated_host_evidence.py" \
      --preflight "$subtree" > "$preflight"
    grep -Fx "DELEGATED_PREFLIGHT_STATUS=CANDIDATE_NOT_PROVEN" "$preflight" >/dev/null || {
      echo "DELEGATED_CI_UNAVAILABLE=PREFLIGHT_NOT_CANDIDATE" >&2; exit 2;
    }
    # Real run: distinct private 64 MiB leaves for positive and negative
    # workloads, source SHA, independent local OOM evidence and teardown.
    evidence="$RUNNER_TEMP/rawmocs-memory-$$-evidence.json"
    internal_log="$RUNNER_TEMP/rawmocs-delegated-internal.log"
    python3 "$GITHUB_WORKSPACE/benchmarks/rust_core/tools/delegated_host_evidence.py" \
      --delegated-run "$subtree" --output "$evidence" --confirm \
      > "$internal_log" 2>&1 || {
      # The private JSON and raw stderr can include cgroup host paths.
      # The diagnostic tool emits ONLY predefined, path-free categories.
      python3 "$GITHUB_WORKSPACE/benchmarks/rust_core/tools/ci_delegated_diagnostics.py" \
        --report "$evidence" --internal-log "$internal_log" >&2
      exit 1
    }
    # Emit only a sanitized, strictly typed status. Never upload the JSON
    # containing the private service path or raw stderr automatically.
    python3 - "$evidence" <<'PY'
import json, sys
data = json.load(open(sys.argv[1], encoding="utf-8"))
assert data["classification"] == "SCOPED_DELEGATED_MEMORY_OBSERVED"
assert set(data["cases"]) == {"under", "over"}
for case in data["cases"].values():
    assert case["populated_zero"] and case["removed"]
assert data["cases"]["under"]["direct_exit"] == "SUCCESS"
assert data["cases"]["over"]["direct_exit"] == "FAILURE"
print("DELEGATED_CI_REAL_ENFORCEMENT=QUALIFIED_SCOPED")
print("DELEGATED_CI_PRIVATE_CGROUP_CLEANUP=PASS")
print("DELEGATED_CI_GAUSSIAN_WMC_ORACLE=PASS")
PY
    ;;
  *)
    echo "usage: ci_delegated_memory.sh --check | --run | --inside-delegated-unit" >&2
    exit 2
    ;;
esac
