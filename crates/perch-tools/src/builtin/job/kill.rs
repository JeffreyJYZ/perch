//! Killing one job's whole process group, and reporting what was killed.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use wait_timeout::ChildExt;

use super::{State, find, lock, refresh};
use crate::Result;

/// How long a killed job is given to exit before its group is killed outright.
const KILL_GRACE: Duration = Duration::from_millis(500);

/// Kills one job's whole process group. A job that already stopped is a report, not an error.
pub(crate) fn kill(root: &Path, id: &str) -> Result<String> {
    let job = find(root, id)?;
    refresh(&job);
    let mut job = lock(&job);
    if job.state != State::Running {
        return Ok(format!(
            "{} already {}; nothing to kill",
            job.id,
            job.state.describe()
        ));
    }

    let pid = job.pid;
    let signalled = signal_group(pid, "TERM");
    let status = match job.child.wait_timeout(KILL_GRACE) {
        Ok(Some(status)) => Some(status),
        _ => {
            signal_group(pid, "KILL");
            job.child.wait().ok()
        }
    };
    job.state = State::Killed(status.and_then(|status| status.code()));
    job.elapsed = Some(job.started.elapsed());

    Ok(format!(
        "killed job {}: sent SIGTERM to process group {pid}{}; state is {}",
        job.id,
        if signalled {
            ""
        } else {
            " (the group was already gone)"
        },
        job.state.describe()
    ))
}

/// Signals the whole group by its leader's pid, which is why every job starts in its own group.
///
/// The `--` is load-bearing: without it this `sh` parses `-1234` as a signal specification and
/// refuses it, so the group is never signalled at all.
fn signal_group(pid: u32, signal: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("kill -s {signal} -- -{pid}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
