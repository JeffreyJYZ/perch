//! The one place a key press becomes what the window means by it.

use rune::{Key, KeyEvent};

use super::live::Waiting;

/// What one key press asks of the app.
///
/// The map from a press to this is [`intent`], the one place the window's keys are decided;
/// [`composer_key`](super::composer_key) is what carries each one out, and the app is the only
/// thing that ever sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    /// Send the composer's text as a turn.
    Submit,
    /// Let the field put a line break in the composer.
    Newline,
    /// Stop the turn.
    Cancel,
    /// Allow the call that is waiting.
    Approve,
    /// Refuse it.
    Deny,
    /// Highlight the waiting question's option at this index (zero-based).
    Select(usize),
    /// Answer the question with the option [`Intent::Select`] highlighted.
    Confirm,
    /// Show the settings panel, or hide it when it is open: ⌘,.
    Settings,
    /// Nothing this window does with that key.
    Ignore,
}

/// What `ev` asks for, given what the session is waiting on: Enter sends, Shift+Enter breaks the
/// line, Ctrl+C interrupts, ⌘, shows the settings, and while an approval or a question is up a
/// number key picks an option, Enter confirms it and Escape refuses or dismisses it.
pub fn intent(ev: &KeyEvent, waiting: Waiting) -> Intent {
    // Ctrl+C is the shell's interrupt, not a character: it stops the turn whatever is on screen.
    // On macOS a Control chord arrives as text with `ctrl` set, so this is where it lands.
    if ev.mods.ctrl
        && !ev.mods.cmd
        && matches!(&ev.key, Key::Text(text) if text.eq_ignore_ascii_case("c"))
    {
        return Intent::Cancel;
    }
    // ⌘, is the settings panel, the window's own chord rather than a menu's.
    if ev.mods.cmd && !ev.mods.alt && matches!(&ev.key, Key::Command(',')) {
        return Intent::Settings;
    }
    // A chord with Cmd or Option belongs to a shortcut or to the input method, never to a turn.
    if ev.mods.cmd || ev.mods.alt {
        return Intent::Ignore;
    }
    match &ev.key {
        Key::Enter if ev.mods.shift => Intent::Newline,
        Key::Enter => match waiting {
            Waiting::Approval => Intent::Approve,
            Waiting::Question { .. } => Intent::Confirm,
            Waiting::Nothing => Intent::Submit,
        },
        Key::Escape => match waiting {
            Waiting::Approval => Intent::Deny,
            Waiting::Question { .. } => Intent::Cancel,
            Waiting::Nothing => Intent::Ignore,
        },
        Key::Text(text) => match (waiting, number(text)) {
            (Waiting::Question { options }, Some(index)) if index < options => {
                Intent::Select(index)
            }
            _ => Intent::Ignore,
        },
        _ => Intent::Ignore,
    }
}

/// The zero-based option a digit key names, when it names one.
fn number(text: &str) -> Option<usize> {
    let mut chars = text.chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    first
        .to_digit(10)
        .filter(|digit| (1..=9).contains(digit))
        .map(|digit| digit as usize - 1)
}

#[cfg(test)]
mod tests {
    use super::{Intent, Waiting, intent};
    use rune::{Key, KeyEvent, Mods};

    fn press(key: Key, shift: bool) -> KeyEvent {
        KeyEvent {
            key,
            mods: Mods {
                shift,
                ..Mods::default()
            },
        }
    }

    fn text(value: &str) -> KeyEvent {
        KeyEvent {
            key: Key::Text(value.to_string()),
            mods: Mods::default(),
        }
    }

    const QUESTION: Waiting = Waiting::Question { options: 3 };

    #[test]
    fn enter_sends_and_shift_enter_breaks_the_line() {
        assert_eq!(
            intent(&press(Key::Enter, false), Waiting::Nothing),
            Intent::Submit
        );
        assert_eq!(
            intent(&press(Key::Enter, true), Waiting::Nothing),
            Intent::Newline
        );
        // A turn that is already running takes the same pair: send, or break the line.
        assert_eq!(
            intent(&press(Key::Enter, false), Waiting::Approval),
            Intent::Approve
        );
    }

    #[test]
    fn a_number_key_picks_and_enter_confirms() {
        assert_eq!(intent(&text("2"), QUESTION), Intent::Select(1));
        assert_eq!(
            intent(&text("9"), QUESTION),
            Intent::Ignore,
            "past the last option"
        );
        assert_eq!(
            intent(&text("0"), QUESTION),
            Intent::Ignore,
            "options count from one"
        );
        assert_eq!(intent(&press(Key::Enter, false), QUESTION), Intent::Confirm);
        // With nothing waiting, a digit is typing: the field keeps it.
        assert_eq!(intent(&text("2"), Waiting::Nothing), Intent::Ignore);
    }

    #[test]
    fn escape_refuses_an_approval_and_dismisses_a_question() {
        assert_eq!(
            intent(&press(Key::Escape, false), Waiting::Approval),
            Intent::Deny
        );
        assert_eq!(intent(&press(Key::Escape, false), QUESTION), Intent::Cancel);
        assert_eq!(
            intent(&press(Key::Escape, false), Waiting::Nothing),
            Intent::Ignore
        );
    }

    #[test]
    fn cmd_comma_asks_for_the_settings() {
        let settings = KeyEvent {
            key: Key::Command(','),
            mods: Mods {
                cmd: true,
                ..Mods::default()
            },
        };
        for waiting in [Waiting::Nothing, Waiting::Approval, QUESTION] {
            assert_eq!(intent(&settings, waiting), Intent::Settings, "{waiting:?}");
        }
        // Without ⌘ a comma is a character in the field, not a shortcut.
        assert_eq!(intent(&text(","), Waiting::Nothing), Intent::Ignore);
    }

    #[test]
    fn ctrl_c_interrupts_whatever_is_on_screen() {
        let interrupt = KeyEvent {
            key: Key::Text("c".to_string()),
            mods: Mods {
                ctrl: true,
                ..Mods::default()
            },
        };
        for waiting in [Waiting::Nothing, Waiting::Approval, QUESTION] {
            assert_eq!(intent(&interrupt, waiting), Intent::Cancel, "{waiting:?}");
        }
        // ⌘C is a copy, not an interrupt.
        let copy = KeyEvent {
            key: Key::Command('c'),
            mods: Mods {
                cmd: true,
                ..Mods::default()
            },
        };
        assert_eq!(intent(&copy, Waiting::Nothing), Intent::Ignore);
    }
}
