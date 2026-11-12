//! The window the history tests lay out, the app behind it, and the home its files live under.
//!
//! Not a test binary itself: `tests/history/main.rs` takes the part of this each module needs.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use perch_store::SessionMeta;
use perch_ui::Shell;
use perch_ui::Transcript;
use perch_ui::shell::{Live, Panels, Waiting};
use rune::{Env, Key, KeyEvent, Mods, Runtime};

/// The window's size in points: the same size the binary opens.
pub const WINDOW: (f32, f32) = (1280.0, 820.0);

/// An app that records every prompt it is sent and reaches no socket.
#[derive(Default)]
pub struct App {
    pub prompts: Vec<String>,
    pub transcript: Transcript,
}

impl Panels for App {}

impl Live for App {
    fn pump(&mut self, _: Duration) -> bool {
        false
    }
    fn transcript(&self) -> Option<&Transcript> {
        Some(&self.transcript)
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
}

/// The home the prompt history resolves its data directory from, made once per test binary.
///
/// `PERCH_HOME` is the seam the window documents, so setting it here is what keeps these tests
/// out of a person's own `Application Support`.
pub fn home() -> &'static Path {
    static HOME: OnceLock<PathBuf> = OnceLock::new();
    HOME.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap().keep();
        // SAFETY: every test in this binary calls this before it builds a window, so no thread is
        // reading the environment while it is written.
        unsafe { std::env::set_var("PERCH_HOME", &dir) };
        dir
    })
}

/// A worktree of this test's own, so two tests never share a history file.
pub fn worktree() -> PathBuf {
    home();
    tempfile::tempdir().unwrap().keep()
}

/// A session header bound to `worktree`.
pub fn meta(worktree: &Path) -> SessionMeta {
    SessionMeta {
        id: "s".to_string(),
        title: "s".to_string(),
        worktree: worktree.to_path_buf(),
        branch: "dev".to_string(),
        model: String::new(),
        created: 0,
    }
}

/// A window over one session on `worktree`, laid out once with the binary's key map.
pub fn window(worktree: &Path) -> Runtime<Shell<App>> {
    let app = App::default();
    let mut runtime = Runtime::new(
        Shell::new(vec![meta(worktree)], 0).with_live(app),
        perch_ui::shell::view,
        WINDOW,
        Env::new(true, 2.0),
    );
    runtime.on_key =
        Some(|shell: &mut Shell<App>, ev, tx| perch_ui::shell::window_key(shell, ev, tx));
    runtime.frame(0.0);
    focus(&mut runtime);
    runtime
}

/// Clicks the composer's field, which is what puts the keyboard in it.
///
/// A frame draws the field but focuses nothing, so a typed key would otherwise reach no input.
pub fn focus(runtime: &mut Runtime<Shell<App>>) {
    let field = runtime
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some("composer-field"))
        .expect("the composer's field is in the tree");
    let at = (
        field.rect[0] + field.rect[2] * 0.5,
        field.rect[1] + field.rect[3] * 0.5,
    );
    runtime.pointer_down(at.0, at.1);
    runtime.pointer_up(at.0, at.1);
    runtime.frame(0.0);
}

/// One press, followed by the frame that runs its handler.
pub fn press(runtime: &mut Runtime<Shell<App>>, key: Key) {
    runtime.key(KeyEvent {
        key,
        mods: Mods::default(),
    });
    runtime.frame(0.0);
}

/// Types `text` one press per character, which is what a person does.
pub fn type_text(runtime: &mut Runtime<Shell<App>>, text: &str) {
    for ch in text.chars() {
        press(runtime, Key::Text(ch.to_string()));
    }
}

/// Types `text` and sends it, as a person does.
pub fn send(runtime: &mut Runtime<Shell<App>>, text: &str) {
    type_text(runtime, text);
    press(runtime, Key::Enter);
}

/// What the composer holds.
pub fn prompt(runtime: &Runtime<Shell<App>>) -> String {
    runtime.state.prompt_text().to_owned()
}

/// The prompts the app was sent, in order.
pub fn sent(runtime: &Runtime<Shell<App>>) -> Vec<String> {
    runtime
        .state
        .live()
        .map(|app| app.prompts.clone())
        .unwrap_or_default()
}
