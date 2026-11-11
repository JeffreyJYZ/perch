//! A draft, the window's ⌘, press, and the layout a test reads the panel out of.

use perch_ui::SettingsDraft;
use perch_ui::Shell;
use perch_ui::shell::{Live, Panels};
use rune::{Color, Env, Key, KeyEvent, Mods, Runtime};

/// A draft with every field stated, so a test names only the one it is about.
pub fn draft(base_url: &str, model: &str, theme: &str, api_key: &str) -> SettingsDraft {
    SettingsDraft {
        base_url: base_url.to_string(),
        model: model.to_string(),
        theme: theme.to_string(),
        update_check: String::new(),
        api_key: api_key.to_string(),
    }
}

/// The window's key press for ⌘,.
pub fn cmd_comma() -> KeyEvent {
    KeyEvent {
        key: Key::Command(','),
        mods: Mods {
            cmd: true,
            ..Mods::default()
        },
    }
}

/// A laid-out window over `shell`.
pub fn layout<L: Live + Panels>(shell: Shell<L>) -> Runtime<Shell<L>> {
    let mut runtime = Runtime::new(
        shell,
        perch_ui::shell::view,
        (1280.0, 820.0),
        Env::new(true, 1.0),
    );
    runtime.frame(0.0);
    runtime
}

/// The colour `key` was filled with, as the frame drew it: the kit a window chose, read off the
/// element rather than guessed from a word.
pub fn fill<L: Live + Panels>(runtime: &Runtime<Shell<L>>, key: &str) -> Color {
    runtime
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no {key} was drawn"))
        .fill
}

/// A click in the middle of `key`, followed by the frame that runs its handler and draws it.
pub fn click<L: Live + Panels>(runtime: &mut Runtime<Shell<L>>, key: &str) {
    let rect = runtime
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no {key} to press"))
        .rect;
    let (x, y) = (rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0);
    runtime.pointer_down(x, y);
    runtime.pointer_up(x, y);
    // A press is dispatched by the next frame, and a real one always has time on it.
    runtime.frame(1.0 / 60.0);
}

/// Lands every track the last press started, so a colour a test reads is the one the state asked
/// for rather than one step of its transition.
pub fn settle<L: Live + Panels>(runtime: &mut Runtime<Shell<L>>) {
    runtime.settle_all();
    runtime.frame(0.0);
}
