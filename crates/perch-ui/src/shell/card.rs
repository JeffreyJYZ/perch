//! A transcript card's key: Enter or Space on the focused card opens or closes it.
//!
//! The column draws one card per tool call and the card is focusable, so the press that opens it
//! needs nothing more than which card holds the focus. Rune reports that on [`Tx::focused`], which
//! is why this is read where the window's keys are — the card itself has no handler of its own.

use rune::{Key, KeyEvent, Tx};

use super::composer::composer_key;
use super::live::Live;
use super::state::Shell;
use crate::preview::preview_key;
use crate::transcript::Item;

/// Whether `ev` is the press that opens or closes a card: Enter or Space, with nothing held.
pub fn toggles_card(ev: &KeyEvent) -> bool {
    if ev.mods.cmd || ev.mods.ctrl || ev.mods.alt || ev.mods.logo {
        return false;
    }
    match &ev.key {
        Key::Enter => true,
        Key::Text(text) => text == " ",
        _ => false,
    }
}

impl<L: Live> Shell<L> {
    /// Flips the tool card keyed `key`, which is the `tool-<index>` the column draws.
    ///
    /// The app's transcript is the one toggled, because the shell holds a copy: toggling the copy
    /// alone would last only until the next frame replaced it. The copy is toggled too, so the
    /// frame after this press is already right.
    pub fn toggle_card(&mut self, key: &str) -> bool {
        let Some(index) = key
            .strip_prefix("tool-")
            .and_then(|index| index.parse::<usize>().ok())
        else {
            return false;
        };
        let Some(Item::Tool { id, .. }) = self.transcript.items().get(index) else {
            return false;
        };
        let id = id.clone();
        if let Some(transcript) = self.live.as_mut().and_then(Live::transcript_mut) {
            transcript.toggle(&id);
        }
        self.transcript.toggle(&id);
        true
    }
}

/// The focused card's key: Enter or Space opens or closes it, and nothing else is touched.
///
/// This is attached to the window's root, because Rune runs an element's `on_key` handlers —
/// the focused element's and its ancestors' — before a focused control's own activation. Space
/// is that activation key, and a card has no press handler, so the root is where Space can be
/// answered. A press that is not this one falls through to the rest of the window untouched.
pub fn card_key<L: Live>(shell: &mut Shell<L>, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let focused = tx.focused().map(str::to_owned);
    let Some(card) = focused.as_deref() else {
        return false;
    };
    toggles_card(ev) && shell.toggle_card(card)
}

/// Every key the window itself answers: a focused card first, then the preview's Escape, then the
/// composer's map.
///
/// The composer's own return would break the line, so a card that holds the focus takes Return
/// before the field sees it; with nothing focused this is exactly [`composer_key`] after
/// [`preview_key`], which only claims a bare Escape.
pub fn window_key<L: Live>(shell: &mut Shell<L>, ev: &KeyEvent, tx: &mut Tx) -> bool {
    card_key(shell, ev, tx) || preview_key(shell, ev) || composer_key(shell, ev, tx)
}

#[cfg(test)]
mod tests {
    use perch_agent::AgentEvent;
    use perch_agent::ToolCall;
    use rune::{Key, KeyEvent, Mods};

    use super::{Shell, toggles_card};

    fn press(key: Key, mods: Mods) -> KeyEvent {
        KeyEvent { key, mods }
    }

    fn card(id: &str) -> AgentEvent {
        AgentEvent::ToolCall(ToolCall {
            id: id.to_string(),
            name: "read".to_string(),
            args: serde_json::json!({ "path": "src/main.rs" }),
        })
    }

    #[test]
    fn enter_and_space_open_a_card_and_a_chord_never_does() {
        assert!(toggles_card(&press(Key::Enter, Mods::default())));
        assert!(toggles_card(&press(
            Key::Text(" ".to_string()),
            Mods::default()
        )));
        for mods in [
            Mods {
                cmd: true,
                ..Mods::default()
            },
            Mods {
                ctrl: true,
                ..Mods::default()
            },
            Mods {
                alt: true,
                ..Mods::default()
            },
        ] {
            assert!(
                !toggles_card(&press(Key::Enter, mods)),
                "a chord belongs to a shortcut, not to the card: {mods:?}"
            );
        }
        assert!(!toggles_card(&press(
            Key::Text("x".to_string()),
            Mods::default()
        )));
    }

    #[test]
    fn the_card_the_key_names_is_the_one_toggled() {
        let mut shell = Shell::idle(Vec::new(), 0);
        shell.transcript_mut().apply(&card("call-1"));

        assert!(shell.toggle_card("tool-0"), "the first card is a card");
        assert!(shell.transcript().is_expanded("call-1"));

        assert!(shell.toggle_card("tool-0"), "a second press closes it");
        assert!(!shell.transcript().is_expanded("call-1"));

        assert!(
            !shell.toggle_card("tool-4"),
            "a key past the last card names nothing"
        );
        assert!(
            !shell.toggle_card("note-0"),
            "only a tool card has something to open"
        );
        assert!(!shell.toggle_card("tool-x"), "an index must be a number");
    }
}
