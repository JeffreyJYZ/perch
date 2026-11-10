//! A child process captured under a timeout, never a bare blocking wait.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::Duration;

use wait_timeout::ChildExt;

use crate::{Error, Result};

pub(crate) struct Captured {
    pub(crate) ok: bool,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: String,
}

/// Spawns `cmd` with both streams captured and kills it when `timeout` expires.
///
/// Each pipe is drained on its own thread, so a chatty child cannot deadlock on a full
/// pipe buffer while the main thread waits for it to exit.
pub(crate) fn run(mut cmd: Command, timeout: Duration) -> Result<Captured> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| Error::Message(format!("cannot run {program}: {e}")))?;

    let out_pipe = child.stdout.take();
    let err_pipe = child.stderr.take();

    let (status, stdout, stderr) = std::thread::scope(|scope| {
        let out_reader = out_pipe.map(|mut pipe| {
            scope.spawn(move || {
                let mut buf = Vec::new();
                let _ = pipe.read_to_end(&mut buf);
                buf
            })
        });
        let err_reader = err_pipe.map(|mut pipe| {
            scope.spawn(move || {
                let mut buf = Vec::new();
                let _ = pipe.read_to_end(&mut buf);
                buf
            })
        });

        let status = match child.wait_timeout(timeout) {
            Ok(Some(status)) => Ok(status),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(Error::Timeout {
                    program: program.clone(),
                    seconds: timeout.as_secs(),
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
        (status, stdout, stderr)
    });

    let status = status?;
    Ok(Captured {
        ok: status.success(),
        stdout,
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    })
}
