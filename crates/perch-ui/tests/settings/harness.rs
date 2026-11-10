//! A draft, the window's ⌘, press, and the layout a test reads the panel out of.

use perch_ui::SettingsDraft;
use perch_ui::Shell;
use perch_ui::shell::{Live, Panels};
use rune::{Env, Key, KeyEvent, Mods, Runtime};

/// A draft with every field stated, so a test names only the one it is about.
pub fn draft(base_url: &str, model: &str, theme: &str, api_key: &str) -> SettingsDraft {
    SettingsDraft {
        base_url: base_url.to_string(),
        model: model.to_string(),
        theme: theme.to_string(),
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
