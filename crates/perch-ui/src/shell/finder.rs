//! The file finder: ⌘P opens one field and the worktree's files under it.
//!
//! The list is read once when the overlay opens, not per keystroke: [`Finder::files`] is what the
//! scan handed over, and typing only ranks it. Matching is a subsequence over the whole path, so
//! `shlout` reaches `crates/perch-ui/src/shell/layout.rs`; one dropped letter is forgiven on a
//! needle of five or more, so `shedlo` reaches it too. The shorter rows and the ones whose file
//! name starts with the needle come first, and at most [`MAX_ROWS`] are drawn.
//!
//! The ranking is kept beside the needle it was made for, so a frame that draws an unchanged field
//! re-reads a dozen rows rather than walking a list of twenty thousand.

use std::cell::{RefCell, RefMut};
use std::path::{Path, PathBuf};

use rune::TextEdit;

use super::finder_match::{Hit, hit};

/// How many rows the overlay draws. More than a dozen is a list nobody reads; the rest is a count
/// said in words under them.
pub const MAX_ROWS: usize = 12;

/// The most files one open keeps. A worktree past this says so in the overlay rather than putting
/// a bound nobody can see on the list.
pub const MAX_FILES: usize = 20_000;

/// The finder's state: the filter field, the files read when it opened, and the rows it keeps.
#[derive(Default)]
pub(super) struct Finder {
    pub(super) query: TextEdit,
    /// Every file the worktree held when the overlay opened, relative to its root.
    pub(super) files: Vec<PathBuf>,
    /// Whether the scan stopped at [`MAX_FILES`] instead of reading the whole worktree.
    pub(super) truncated: bool,
    /// The rows the field keeps, and the needle they were ranked against.
    kept: RefCell<Kept>,
}

/// What the current field text ranks to: the rows to draw, how many matched, and the pick.
#[derive(Default)]
struct Kept {
    /// The needle these rows were ranked for; `None` until one is asked for.
    needle: Option<String>,
    rows: Vec<FinderRow>,
    /// How many files matched before the row cap, so the overlay can say "12 of 40".
    total: usize,
    index: usize,
}

/// One row the overlay draws: the file, its name, and the folder it sits in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinderRow {
    /// The file's path, relative to the worktree root — the form the shell's `open_file` takes.
    pub path: PathBuf,
    /// The row's own name: what the list says.
    pub name: String,
    /// The folder above it, quieter under the name. Empty for a file at the root.
    pub dir: String,
}

impl Finder {
    /// The finder over the files one scan read.
    pub(super) fn read(files: Vec<PathBuf>, truncated: bool) -> Finder {
        Finder {
            files,
            truncated,
            ..Finder::default()
        }
    }

    /// The rows the field keeps, best first, at most [`MAX_ROWS`].
    pub(super) fn rows(&self) -> Vec<FinderRow> {
        self.kept().rows.clone()
    }

    /// How many files matched, before the row cap.
    pub(super) fn total(&self) -> usize {
        self.kept().total
    }

    /// Which row is picked, clamped to the rows the field keeps.
    pub(super) fn index(&self) -> usize {
        let kept = self.kept();
        kept.index.min(kept.rows.len().saturating_sub(1))
    }

    /// Picks the row at `index`, which is what a press on it means before it is taken.
    pub(super) fn pick(&self, index: usize) {
        let mut kept = self.kept();
        if index < kept.rows.len() {
            kept.index = index;
        }
    }

    /// Moves the pick by `step`, wrapping at both ends.
    pub(super) fn step(&self, step: isize) {
        let mut kept = self.kept();
        let len = kept.rows.len() as isize;
        if len <= 0 {
            return;
        }
        kept.index = (kept.index as isize + step).rem_euclid(len) as usize;
    }

    /// The rows for what the field holds now, ranked against it when the text changed.
    fn kept(&self) -> RefMut<'_, Kept> {
        let needle = self.query.text.trim().to_lowercase();
        let kept = self.kept.borrow_mut();
        if kept.needle.as_deref() != Some(needle.as_str()) {
            let mut kept = kept;
            let (rows, total) = rank(&self.files, &needle);
            *kept = Kept {
                needle: Some(needle),
                rows,
                total,
                index: 0,
            };
            return kept;
        }
        kept
    }
}

impl FinderRow {
    /// The row for `path`, split into the name the list shows and the folder it came from.
    fn new(path: &Path) -> FinderRow {
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let dir = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(|parent| parent.display().to_string())
            .unwrap_or_default();
        FinderRow {
            path: path.to_path_buf(),
            name,
            dir,
        }
    }
}

/// `files` ranked against `needle`: the rows to draw and how many matched.
fn rank(files: &[PathBuf], needle: &str) -> (Vec<FinderRow>, usize) {
    let mut kept: Vec<(Hit, usize, FinderRow)> = Vec::new();
    for file in files {
        let shown = file.to_string_lossy().to_lowercase();
        if let Some(hit) = hit(&shown, needle) {
            kept.push((hit, shown.chars().count(), FinderRow::new(file)));
        }
    }
    let total = kept.len();
    kept.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(a.1.cmp(&b.1))
            .then_with(|| a.2.path.cmp(&b.2.path))
    });
    kept.truncate(MAX_ROWS);
    (kept.into_iter().map(|(_, _, row)| row).collect(), total)
}
