//! A project path and a session name each turned into one file name, so no two boards or cursors
//! can land on the same file.

use std::path::Path;

/// The longest a readable name may get before it is cut.
const NAME_MAX: usize = 40;

/// The board's name for a project: its directory name plus a hash of the whole path.
///
/// Two checkouts both called `app` are different projects, so the hash of the canonical path is
/// what keeps their boards apart; the name in front is only there to be readable.
pub fn project(path: &Path) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let name = canonical
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("board");
    format!("{}-{:012x}", clean(name), hash(&canonical))
}

/// The per-session half of a cursor's name. A session called `a/b` must not make a directory.
pub fn session(name: &str) -> String {
    let cleaned = clean(name);
    if cleaned == name {
        cleaned
    } else {
        format!("{cleaned}-{:08x}", hash(Path::new(name)))
    }
}

fn clean(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out.truncate(NAME_MAX);
    if out.is_empty() {
        "board".to_string()
    } else {
        out
    }
}

/// FNV-1a over the path's bytes: short, stable across runs, and not a security boundary — a
/// collision only means two projects share a name the readable part already tells apart.
fn hash(path: &Path) -> u64 {
    let mut value: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.to_string_lossy().as_bytes() {
        value ^= u64::from(*byte);
        value = value.wrapping_mul(0x0000_0100_0000_01b3);
    }
    value
}
