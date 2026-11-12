//! Starting one job: its process group, its log file, and the watcher that outlives nothing.
//!
//! No child outlives the app. Each job's stdin is a pipe the app holds; when the app exits the
//! write end closes, the wrapper reads end-of-file and kills its own process group. That is why
//! [`super::Job::_stdin`] stays open for the job's whole life, and why the wrapper carries a
//! watcher rather than relying on a shutdown hook, which a `SIGKILL` of the app would skip.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Instant;

use std::os::unix::process::CommandExt;

use super::{JOB_MAX_CONCURRENT, Job, State, lock, refresh, session};
use crate::{Error, Result};

/// Every job's log lands here, one file per job.
const LOG_DIR: &str = "perch-jobs";

/// Starts `command` in its own process group and answers its id at once.
pub(crate) fn start(command: &str, root: &Path) -> Result<(String, u32, PathBuf)> {
    let session = session(root)?;
    let mut guard = lock(&session);
    for job in &guard.jobs {
        refresh(job);
    }
    let running = guard
        .jobs
        .iter()
        .filter(|job| lock(job).state == State::Running)
        .count();
    if running >= JOB_MAX_CONCURRENT {
        return Err(Error::Message(format!(
            "{running} background jobs are already running here, which is the cap of \
             {JOB_MAX_CONCURRENT}; wait for one to exit or kill it with `jobs`"
        )));
    }

    let id = format!("j{}", guard.next);
    guard.next += 1;
    let log = log_path(root, &id)?;
    let file = std::fs::File::create(&log)
        .map_err(|e| Error::Message(format!("cannot open {}: {e}", log.display())))?;
    let mut child = Command::new("sh");
    child
        .arg("-c")
        .arg(wrapper(command))
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::from(file.try_clone()?))
        .stderr(Stdio::from(file))
        // Its own process group, so `jobs kill` reaches the command and its children.
        .process_group(0);
    let mut child = child
        .spawn()
        .map_err(|e| Error::Message(format!("cannot run `{command}`: {e}")))?;
    let pid = child.id();
    let stdin = child.stdin.take();
    guard.jobs.push(std::sync::Arc::new(Mutex::new(Job {
        id: id.clone(),
        command: command.to_string(),
        pid,
        log: log.clone(),
        started: Instant::now(),
        elapsed: None,
        state: State::Running,
        child,
        _stdin: stdin,
    })));
    Ok((id, pid, log))
}

/// The shell that runs one job: the command in this process group, plus a watcher that kills the
/// group when the app's end of stdin closes. Both streams go to the job's log file.
///
/// `kill` needs its `--` here for the same reason [`super::kill`] does: a bare `-PID` is read as
/// a signal specification, so the watcher would wake on end-of-file and then kill nothing.
fn wrapper(command: &str) -> String {
    format!(
        "exec 3<&0 </dev/null\n\
         ( read -r __perch_watch <&3; kill -s TERM -- -$$ 2>/dev/null; sleep 1; \
         kill -s KILL -- -$$ 2>/dev/null ) &\n\
         __perch_watch=$!\n\
         {command}\n\
         __perch_code=$?\n\
         kill $__perch_watch 2>/dev/null\n\
         exit $__perch_code\n"
    )
}

fn log_path(root: &Path, id: &str) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(LOG_DIR);
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join(format!("{}-{id}.log", fingerprint(root))))
}

/// A per-process name for the worktree, so two roots never share a log file.
fn fingerprint(root: &Path) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    root.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::builtin::job::{lock, refresh, session};

    /// Closing the app's end of the pipe is what an app exit does, `SIGKILL` included; the job's
    /// whole group must go with it, not just the wrapper shell.
    #[test]
    fn closing_the_apps_end_of_the_pipe_kills_the_group() {
        let root = tempfile::tempdir().unwrap();
        let (id, pid, _log) = start("sleep 60 & echo $! > child.pid; wait", root.path()).unwrap();
        let child = loop {
            if let Ok(text) = std::fs::read_to_string(root.path().join("child.pid"))
                && let Ok(child) = text.trim().parse::<u32>()
            {
                break child;
            }
            std::thread::sleep(Duration::from_millis(20));
        };

        let session = session(root.path()).unwrap();
        let job = {
            let guard = lock(&session);
            guard
                .jobs
                .iter()
                .find(|job| lock(job).id == id)
                .cloned()
                .unwrap()
        };
        // The app's write end closes here, exactly as when the process exits.
        lock(&job)._stdin = None;

        let stopped = Instant::now();
        loop {
            refresh(&job);
            if lock(&job).state != State::Running {
                break;
            }
            assert!(
                stopped.elapsed() < Duration::from_secs(5),
                "the job outlived the app's end of the pipe"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        for gone in [pid, child] {
            let ps = std::process::Command::new("ps")
                .args(["-p", &gone.to_string()])
                .output()
                .unwrap();
            assert!(
                !String::from_utf8_lossy(&ps.stdout).contains(&gone.to_string()),
                "pid {gone} outlived the app"
            );
        }
    }
}
