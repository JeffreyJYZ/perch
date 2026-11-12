//! The app a command test runs against: every effect recorded, nothing reaching a socket.
//!
//! One fake for the whole target, so a test asserts what the command *did* — the id chosen, the
//! title saved, the rows cleared — rather than that a word was printed.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use perch_store::SessionMeta;
use perch_ui::Shell;
use perch_ui::Transcript;
use perch_ui::files::Editor;
use perch_ui::shell::{Live, Panels, Waiting};

/// One editor that records the paths it was handed.
#[derive(Default)]
pub struct Recorder {
    pub opened: Mutex<Vec<PathBuf>>,
}

impl Editor for Recorder {
    fn open_in_zed(&self, path: &Path) -> std::result::Result<(), String> {
        self.opened.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
}

/// A session header named `title`, bound to `worktree`.
pub fn meta(title: &str, worktree: &Path) -> SessionMeta {
    SessionMeta {
        id: title.to_string(),
        title: title.to_string(),
        worktree: worktree.to_path_buf(),
        branch: "dev".to_string(),
        model: "test/model".to_string(),
        created: 1,
    }
}

/// The app behind the window: every press recorded, every answer given.
#[derive(Default)]
pub struct Fake {
    pub metas: Vec<SessionMeta>,
    pub transcript: Transcript,
    pub prompts: Vec<String>,
    pub compacted: bool,
    pub chosen_model: Option<String>,
    pub theme: Option<String>,
    pub data: Option<PathBuf>,
    pub renamed: Option<(usize, String)>,
    pub deleted: Option<usize>,
    pub answered: Option<(usize, String)>,
    /// What `/undo` answers: the lines it restored, or the refusal, in words.
    pub undo: Option<Result<Vec<String>, String>>,
}

impl Fake {
    /// An app with one session on `worktree`, named `title`.
    pub fn one(title: &str, worktree: &Path) -> Fake {
        Fake {
            metas: vec![meta(title, worktree)],
            ..Fake::default()
        }
    }
}

impl Panels for Fake {}

impl Live for Fake {
    fn pump(&mut self, _: Duration) -> bool {
        false
    }
    fn transcript(&self) -> Option<&Transcript> {
        Some(&self.transcript)
    }
    fn transcript_mut(&mut self) -> Option<&mut Transcript> {
        Some(&mut self.transcript)
    }
    fn waiting(&self) -> Waiting {
        Waiting::Nothing
    }
    fn busy(&self) -> bool {
        false
    }
    fn prompt(&mut self, text: String) -> std::result::Result<(), String> {
        self.prompts.push(text);
        Ok(())
    }
    fn cancel(&mut self) {}
    fn approve(&mut self) {}
    fn deny(&mut self) {}
    fn pick(&mut self, _: usize) {}
    fn switch(&mut self, _: usize) {}
    fn compact(&mut self) {
        self.compacted = true;
    }
    fn data_dir(&self) -> Option<PathBuf> {
        self.data.clone()
    }
    fn new_session(
        &mut self,
        worktree: &Path,
    ) -> std::result::Result<(usize, SessionMeta), String> {
        let title = worktree.file_name().map_or_else(
            || "session".to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        self.metas.push(meta(&title, worktree));
        Ok((self.metas.len() - 1, self.metas.last().unwrap().clone()))
    }
    fn rename_session(
        &mut self,
        index: usize,
        title: &str,
    ) -> std::result::Result<SessionMeta, String> {
        self.renamed = Some((index, title.to_string()));
        let Some(meta) = self.metas.get_mut(index) else {
            return Err(format!("session {index} is not open"));
        };
        meta.title = title.to_string();
        Ok(meta.clone())
    }
    fn delete_session(&mut self, index: usize) -> std::result::Result<usize, String> {
        if index >= self.metas.len() {
            return Err(format!("session {index} is not open"));
        }
        self.deleted = Some(index);
        self.metas.remove(index);
        Ok(0)
    }
    fn set_theme(&mut self, theme: &str) -> std::result::Result<(), String> {
        self.theme = Some(theme.to_string());
        Ok(())
    }
    fn choose_model(&mut self, id: String) -> std::result::Result<(), String> {
        self.chosen_model = Some(id);
        Ok(())
    }
    fn undo(&mut self) -> std::result::Result<Vec<String>, String> {
        self.undo
            .clone()
            .unwrap_or_else(|| Err("no app is running, so nothing was restored".to_string()))
    }
}

/// The window over `app`, laid out once with the binary's key map.
pub fn window(app: Fake) -> rune::Runtime<Shell<Fake>> {
    let mut runtime = rune::Runtime::new(
        Shell::new(app.metas.clone(), 0).with_live(app),
        perch_ui::shell::view,
        (1280.0, 820.0),
        rune::Env::new(true, 2.0),
    );
    runtime.on_key =
        Some(|shell: &mut Shell<Fake>, ev, tx| perch_ui::shell::window_key(shell, ev, tx));
    runtime.frame(0.0);
    runtime
}

/// Every note the transcript holds, in order.
pub fn notes(shell: &Shell<Fake>) -> Vec<String> {
    shell
        .transcript()
        .items()
        .iter()
        .filter_map(|item| match item {
            perch_ui::Item::Note(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// A shared editor for a test that presses `/editor`.
pub fn editor() -> Arc<Recorder> {
    Arc::new(Recorder::default())
}
