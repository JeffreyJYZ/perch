//! One board per project: the append-only log, the cursor one session reads it with, and the
//! filtering both tools share.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read as _, Seek, SeekFrom, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::cursor;
use crate::entry::{Entry, MAX_FILES, Post};
use crate::error::{Error, Result};
use crate::session::{Fixed, SessionName};
use crate::slug;

/// How many entries one read hands back before it says how many are left.
pub const READ_CAP: usize = 50;

/// What one read found: the entries, and how many were held back by the cap.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Read {
    pub entries: Vec<Entry>,
    pub remaining: usize,
}

/// The log one project's sessions share, and the cursor one of them reads it with.
#[derive(Clone)]
pub struct Store {
    dir: PathBuf,
    slug: String,
    session: Arc<dyn SessionName>,
}

impl Store {
    /// `root` is where the board directory lives; `project` is what the slug is derived from.
    ///
    /// Passing the same path as both puts the board inside the project, which is what the shipped
    /// tools do. Passing a shared root and two projects keeps them apart by slug alone.
    pub fn new(root: &Path, project: &Path, session: &str) -> Self {
        Self::with_source(root, project, Arc::new(Fixed(session.to_string())))
    }

    /// The same store, reading its session from `source` on every call.
    ///
    /// This is what a per-process registry needs: the tools outlive the session they post as, so
    /// the name is asked when a call runs rather than when the store was built.
    pub fn with_source(root: &Path, project: &Path, source: Arc<dyn SessionName>) -> Self {
        Self {
            dir: root.join("board"),
            slug: slug::project(project),
            session: source,
        }
    }

    /// The session this store reads and posts as, as of right now.
    pub fn session(&self) -> String {
        self.session.name()
    }

    /// The log's path: the only file a post touches.
    pub fn log(&self) -> PathBuf {
        self.dir.join(format!("{}.jsonl", self.slug))
    }

    /// The cursor's path, which moves with the session: one cursor per session name.
    pub fn cursor_path(&self) -> PathBuf {
        let session = slug::session(&self.session.name());
        self.dir.join(format!("{}.{session}.cursor", self.slug))
    }

    /// Appends one entry and answers what landed, including the id the log gave it.
    pub fn append(&self, post: Post) -> Result<Entry> {
        fs::create_dir_all(&self.dir)?;
        let from = self.session.name();
        if from.is_empty() {
            return Err(Error::Message("the board has no session name".into()));
        }
        let at = crate::now_millis();
        // Above every id already written, so a cursor stays meaningful across restarts, and at
        // least the clock, so two boards started at the same moment do not disagree about order.
        let id = match last_id(&self.log())? {
            Some(last) => (last + 1).max(at),
            None => at,
        };
        let entry = Entry {
            id,
            at,
            from,
            to: post.to,
            group: post.group,
            text: post.text,
            files: post.files.into_iter().take(MAX_FILES).collect(),
        };
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log())?;
        file.write_all(entry.line().as_bytes())?;
        Ok(entry)
    }

    /// Every entry written since byte `offset`, and where reading may resume.
    ///
    /// Only complete lines are returned: a post is one `write_all`, but a reader that arrives
    /// mid-write must see the line whole or not at all, so a trailing partial is left for the next
    /// call. A line that will not parse is skipped and the offset still passes it.
    pub fn since(&self, offset: u64) -> Result<(Vec<Entry>, u64)> {
        let mut file = match fs::File::open(self.log()) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok((Vec::new(), offset)),
            Err(error) => return Err(error.into()),
        };
        let len = file.metadata()?.len();
        if len <= offset {
            return Ok((Vec::new(), len));
        }
        file.seek(SeekFrom::Start(offset))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let Some(end) = bytes.iter().rposition(|byte| *byte == b'\n') else {
            return Ok((Vec::new(), offset));
        };
        let text = String::from_utf8_lossy(&bytes[..=end]);
        let entries = text.lines().filter_map(Entry::parse).collect();
        Ok((entries, offset + end as u64 + 1))
    }

    /// The unseen entries this session may read, capped, with the cursor moved past them.
    pub fn take(&self) -> Result<Read> {
        // The name and the cursor are taken together, so a slot that changes mid-read cannot make
        // this call filter as one session and advance another's cursor.
        let session = self.session.name();
        let cursor_file = self.cursor_path();
        let cursor = cursor::read(&cursor_file)?;
        let (entries, _) = self.since(0)?;
        let newest = entries
            .iter()
            .map(|entry| entry.id)
            .filter(|id| *id > cursor)
            .max();
        let mut visible: Vec<Entry> = entries
            .into_iter()
            .filter(|entry| entry.id > cursor && entry.visible_to(&session))
            .collect();
        let total = visible.len();
        visible.truncate(READ_CAP);
        let remaining = total - visible.len();
        // Past everything read, so an entry addressed elsewhere is never re-read forever; when
        // the cap held some back, only past what was handed over, so the rest stays unseen.
        if let Some(last) = visible.last() {
            cursor::write(&cursor_file, last.id)?;
        } else if let Some(newest) = newest {
            cursor::write(&cursor_file, newest)?;
        }
        Ok(Read {
            entries: visible,
            remaining,
        })
    }

    /// Whether a matching unseen entry exists, without moving the cursor.
    ///
    /// The wait polls this, so a match costs one read and a miss changes nothing: entries that do
    /// not match stay unseen for the next plain read.
    pub fn peek(&self, offset: u64, needle: &str) -> Result<(bool, u64)> {
        let session = self.session.name();
        let cursor = cursor::read(&self.cursor_path())?;
        let (entries, offset) = self.since(offset)?;
        let found = entries
            .iter()
            .any(|entry| entry.id > cursor && entry.visible_to(&session) && entry.contains(needle));
        Ok((found, offset))
    }
}

/// The id of the last entry, or none when the log is empty or holds no readable line.
fn last_id(path: &Path) -> Result<Option<u64>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let text = String::from_utf8_lossy(&bytes);
    Ok(text
        .lines()
        .rev()
        .find_map(|line| Entry::parse(line).map(|entry| entry.id)))
}
