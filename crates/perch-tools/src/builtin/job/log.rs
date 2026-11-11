//! Reading one job's log: the tail, never the whole file, and never more than the byte budget.

use std::io::{Read as IoRead, Seek, SeekFrom};
use std::path::Path;

use super::{find, lock, refresh};
use crate::{Result, builtin::BASH_MAX_OUTPUT, builtin::text::truncate};

/// Bytes read from the end of a log; the tool then takes its lines and its byte budget.
const TAIL_BYTES: u64 = 256 * 1024;

/// The last `lines` lines of one job's log, plus where that job stands.
pub(crate) fn read(root: &Path, id: &str, lines: usize) -> Result<String> {
    let job = find(root, id)?;
    refresh(&job);
    let job = lock(&job);
    let text = tail(&job.log)?;
    let all: Vec<&str> = text.lines().collect();
    let total = all.len();
    let keep = lines.max(1);
    let start = total.saturating_sub(keep);

    let mut body = String::new();
    for line in &all[start..] {
        body.push_str(line);
        body.push('\n');
    }
    let mut out = format!(
        "{} {} {:.1}s\n{body}",
        job.id,
        job.state.describe(),
        job.elapsed().as_secs_f64()
    );
    if start > 0 {
        out.push_str(&format!("[showing the last {keep} of {total} lines]\n"));
    }
    Ok(truncate(out, BASH_MAX_OUTPUT))
}

/// Reads the end of a log, so a chatty job cannot make `read` load the whole file.
fn tail(path: &Path) -> Result<String> {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => return Err(error.into()),
    };
    let len = file.metadata()?.len();
    if len > TAIL_BYTES {
        file.seek(SeekFrom::Start(len - TAIL_BYTES))?;
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
