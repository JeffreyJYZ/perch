//! The instruction files that belong to the person, not to a repository.
//!
//! A project can say how its code is written; only the machine's own files can say how the person
//! wants every session run — the same rules in every repo. They are read before any project file,
//! so a repository's `AGENTS.md` still has the last word on its own conventions.
//!
//! The well-known names are read because a person who already keeps one of them should not have to
//! copy it into a second place: perch's own, then the OpenCode, Claude and shared `~/.agents`
//! files, each contributing only when it exists.

use std::path::{Path, PathBuf};

use crate::instructions::{absorb, is_readable, push};

/// The files a machine may keep global instructions in, relative to the home directory.
const NAMES: [&str; 4] = [
    ".config/perch/AGENTS.md",
    ".config/opencode/AGENTS.md",
    ".agents/AGENTS.md",
    ".claude/CLAUDE.md",
];

/// Reads every global instruction file under `home` into `out`, in the order above.
pub(crate) fn absorb_home(out: &mut String, home: &Path) {
    for name in NAMES {
        let path = home.join(name);
        // No worktree to confine a link to, so a link is only followed when it resolves to a file
        // inside the home directory the person already owns.
        if !is_readable(&path, home) {
            continue;
        }
        match std::fs::read(&path) {
            Ok(bytes) => absorb(out, &path, &bytes),
            Err(error) => push(
                out,
                &format!(
                    "Instructions from {} could not be read: {error}",
                    path.display()
                ),
            ),
        }
    }
}

/// The home directory the global files are looked for in: `PERCH_HOME` when a test or a sandbox
/// sets one, otherwise the real home.
pub(crate) fn home() -> Option<PathBuf> {
    if let Some(stated) = std::env::var_os("PERCH_HOME") {
        return Some(PathBuf::from(stated));
    }
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn every_known_global_file_is_read_in_order() {
        let home = tempfile::tempdir().unwrap();
        write(&home.path().join(".config/perch/AGENTS.md"), "perch rules");
        write(&home.path().join(".claude/CLAUDE.md"), "claude rules");
        let mut out = String::new();
        absorb_home(&mut out, home.path());
        let perch = out.find("perch rules").expect("perch's own file");
        let claude = out.find("claude rules").expect("the claude file");
        assert!(
            perch < claude,
            "the order is the one declared, not the disk's"
        );
    }

    #[test]
    fn a_home_without_instruction_files_contributes_nothing() {
        let home = tempfile::tempdir().unwrap();
        let mut out = String::new();
        absorb_home(&mut out, home.path());
        assert!(out.is_empty(), "{out}");
    }

    #[test]
    fn an_unreadable_global_file_is_a_line_rather_than_a_failure() {
        let home = tempfile::tempdir().unwrap();
        // A directory where a file is expected: readable is false, so nothing is said and nothing
        // fails — the same rule a missing file follows.
        std::fs::create_dir_all(home.path().join(".config/perch/AGENTS.md")).unwrap();
        let mut out = String::new();
        absorb_home(&mut out, home.path());
        assert!(out.is_empty(), "{out}");
    }
}
