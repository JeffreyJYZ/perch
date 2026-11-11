//! The one place a key press becomes what the window means by it.

use rune::{Key, KeyEvent, Tx};

use super::live::{Live, Waiting};
use super::state::Shell;

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
    /// Allow it for the rest of the session.
    AllowForSession,
    /// Refuse it.
    Deny,
    /// Highlight the waiting question's option at this index (zero-based).
    Select(usize),
    /// Answer the question with every option picked.
    Confirm,
    /// Open the field a question is answered in, with text.
    Type,
    /// Show the settings panel, or hide it when it is open: ⌘,.
    Settings,
    /// Show the command palette, or hide it when it is open: ⌘K.
    Palette,
    /// Show the file finder, or hide it when it is open: ⌘P.
    Finder,
    /// Nothing this window does with that key.
    Ignore,
}

/// What a press asks while the text field is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Typing {
    /// Send what was typed as the answer.
    Send,
    /// Go back to the options without answering.
    Back,
}

/// What `ev` asks for, given what the session is waiting on: Enter sends, Shift+Enter breaks the
/// line, Ctrl+C interrupts, ⌘, shows the settings, and while an approval or a question is up a
/// number key picks an option, `t` opens the answer field, Enter confirms, ⌘Enter allows the call
/// for the session and Escape refuses or dismisses it.
pub fn intent(ev: &KeyEvent, waiting: Waiting) -> Intent {
    // Ctrl+C is the shell's interrupt, not a character: it stops the turn whatever is on screen.
    // On macOS a Control chord arrives as text with `ctrl` set, so this is where it lands.
    if interrupts(ev) {
        return Intent::Cancel;
    }
    // ⌘, is the settings panel, the window's own chord rather than a menu's.
    if ev.mods.cmd && !ev.mods.alt && matches!(&ev.key, Key::Command(',')) {
        return Intent::Settings;
    }
    // ⌘K is the palette. K is not a composer key and not a terminal key, so it collides with
    // neither: the dock owns ⌘C and ⌘V, and the settings own ⌘, and ⌘Enter.
    if ev.mods.cmd && !ev.mods.alt && matches!(&ev.key, Key::Command('k')) {
        return Intent::Palette;
    }
    // ⌘P is the file finder. P is not a composer key and not a terminal key either, and the dock
    // owns ⌘C and ⌘V only, so nothing else answers this chord.
    if ev.mods.cmd && !ev.mods.alt && matches!(&ev.key, Key::Command('p')) {
        return Intent::Finder;
    }
    // ⌘Enter is the second approval key, read before the chord guard below: it is the window's
    // chord, and only an approval has anything to allow for the session.
    if ev.mods.cmd
        && !ev.mods.alt
        && !ev.mods.shift
        && matches!(&ev.key, Key::Enter)
        && waiting == Waiting::Approval
    {
        return Intent::AllowForSession;
    }
    // A chord with Cmd or Option belongs to a shortcut or to the input method, never to a turn.
    if ev.mods.cmd || ev.mods.alt {
        return Intent::Ignore;
    }
    match &ev.key {
        Key::Enter if ev.mods.shift => Intent::Newline,
        Key::Enter => match waiting {
            Waiting::Approval => Intent::Approve,
            // A question that offers nothing is answered in the person's own words, so Enter
            // opens the field rather than confirming a pick there is none of.
            Waiting::Question { options: 0, .. } => Intent::Type,
            Waiting::Question { .. } => Intent::Confirm,
            Waiting::Nothing => Intent::Submit,
        },
        Key::Escape => match waiting {
            Waiting::Approval => Intent::Deny,
            Waiting::Question { .. } => Intent::Cancel,
            Waiting::Nothing => Intent::Ignore,
        },
        Key::Text(text) => match (waiting, number(text)) {
            (Waiting::Question { options, .. }, Some(index)) if index < options => {
                Intent::Select(index)
            }
            (Waiting::Question { .. }, None) if text.eq_ignore_ascii_case("t") => Intent::Type,
            _ => Intent::Ignore,
        },
        _ => Intent::Ignore,
    }
}

/// The session list's own key, while `/sessions` has the keyboard in it: Up and Down move the
/// selection and Escape returns to the prompt. Every other press is ignored, so a stray letter
/// does not open a session.
pub fn session_list_key<L: Live>(shell: &mut Shell<L>, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if !shell.sessions_focused() {
        return false;
    }
    match &ev.key {
        Key::Up => {
            shell.select(shell.active().saturating_sub(1));
            true
        }
        Key::Down => {
            let last = shell.session_count().saturating_sub(1);
            shell.select((shell.active() + 1).min(last));
            true
        }
        Key::Escape if !ev.mods.cmd && !ev.mods.alt => {
            shell.leave_sessions();
            tx.focus("composer", true);
            true
        }
        _ => false,
    }
}

/// What `ev` asks while the answer field is open, or `None` when the field keeps the press.///
/// This is a map of its own rather than a branch of [`intent`] because every other key belongs to
/// the field: a digit that picked an option a moment ago is a character in the answer now.
pub fn typing_intent(ev: &KeyEvent) -> Option<Typing> {
    if ev.mods.cmd || ev.mods.alt {
        return None;
    }
    match &ev.key {
        Key::Enter if !ev.mods.shift => Some(Typing::Send),
        Key::Escape => Some(Typing::Back),
        _ => None,
    }
}

/// Whether `ev` is the shell's interrupt. Only Ctrl+C is: a bare Escape is a question's cancel
/// and an approval's refusal, and means nothing at all while a turn is simply running.
pub fn interrupts(ev: &KeyEvent) -> bool {
    ev.mods.ctrl
        && !ev.mods.cmd
        && matches!(&ev.key, Key::Text(text) if text.eq_ignore_ascii_case("c"))
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
