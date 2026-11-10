//! Running a child under a deadline with both streams drained.

use std::io::Read as IoRead;
use std::process::Command;
use std::time::Duration;

use wait_timeout::ChildExt;

use crate::{Error, Result};

/// Bytes kept from each child stream before the rest is drained and discarded.
const MAX_CAPTURE: usize = 1024 * 1024;

pub(super) struct Captured {
    pub(super) code: Option<i32>,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
}

/// Runs a child under a deadline, draining both pipes so a chatty command cannot deadlock.
pub(super) fn run_captured(mut cmd: Command, timeout: Duration) -> Result<Captured> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    let mut child = cmd
        .spawn()
        .map_err(|e| Error::Message(format!("cannot run {program}: {e}")))?;
    let out_pipe = child.stdout.take();
    let err_pipe = child.stderr.take();

    std::thread::scope(|scope| {
        let out_reader = out_pipe.map(|pipe| scope.spawn(move || drain(pipe)));
        let err_reader = err_pipe.map(|pipe| scope.spawn(move || drain(pipe)));

        let status = match child.wait_timeout(timeout) {
            Ok(Some(status)) => Ok(status),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(Error::Timeout {
                    program: program.clone(),
                    after: timeout,
                })
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(Error::Io(e))
            }
        };

        let stdout = out_reader.and_then(|h| h.join().ok()).unwrap_or_default();
        let stderr = err_reader.and_then(|h| h.join().ok()).unwrap_or_default();
        match status {
            Ok(status) => Ok(Captured {
                code: status.code(),
                stdout,
                stderr,
            }),
            Err(e) => Err(e),
        }
    })
}

/// Reads a pipe to the end, keeping the first `MAX_CAPTURE` bytes so the child never blocks.
fn drain(mut pipe: impl IoRead) -> Vec<u8> {
    let mut kept = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match pipe.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if kept.len() < MAX_CAPTURE {
                    let room = MAX_CAPTURE - kept.len();
                    kept.extend_from_slice(&buf[..n.min(room)]);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    kept
}
