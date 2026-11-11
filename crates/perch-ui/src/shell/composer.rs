//! The prompt: one multi-line field, the command list above it, the card a waiting question
//! shows, and the presses that send, answer or stop a turn.

use rune::{El, Key, KeyEvent, Tx, col};
use rune_kit::Kit;
use rune_kit::TextArea;

use super::answer_view::{answer_key, card};
use super::commands_view::menu;
use super::keymap::{Intent, intent};
use super::live::Live;
use super::meter::composer_meter;
use super::model_picker_view::composer_model;
use super::state::Shell;
use crate::files::section_label;

/// The prompt: one multi-line field, in its own labelled section, under the command list and the
/// answer card when either is showing.
pub(super) fn composer<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    let mut area = col()
        .key("composer")
        .w_full()
        .px(20.0)
        .pt(4.0)
        .pb(12.0)
        .gap(4.0)
        .child(composer_model(shell, kit))
        .child(composer_meter(shell, kit))
        .child(section_label(kit, "Prompt", "prompt-label"));
    if let Some(menu) = menu(shell, kit) {
        area = area.child(menu);
    }
    if let Some(card) = card(shell, kit) {
        area = area.child(card);
    }
    area.child(
        TextArea::new(&shell.prompt)
            .label("Prompt")
            .placeholder("Ask perch…")
            .rows(3)
            .width(None)
            .on_edit(|shell: &mut Shell<L>| &mut shell.prompt)
            .on_key(composer_key::<L>)
            .on_submit(submit::<L>)
            .build(kit),
    )
}

/// Sends what the box holds, as the same path Enter takes, then carries out whatever the command
/// asked of the window itself — a close, a focus — through the `Tx` only this path has.
fn submit<L: Live>(shell: &mut Shell<L>, tx: &mut Tx) {
    shell.submit();
    drain(shell, tx);
}

/// Applies the one-shot requests a command left for the window.
fn drain<L: Live>(shell: &mut Shell<L>, tx: &mut Tx) {
    if shell.take_quit() {
        tx.close_window();
    }
    if shell.take_focus_sessions() {
        tx.focus("sessions", true);
    }
}

/// Whether `ev` is the slash that opens the command list: an empty composer, no chord.
fn opens_menu<L: Live>(shell: &Shell<L>, ev: &KeyEvent) -> bool {
    !ev.mods.cmd
        && !ev.mods.alt
        && !ev.mods.ctrl
        && matches!(&ev.key, Key::Text(text) if text == "/")
        && shell.prompt.text.is_empty()
}

/// Applies one press to the shell through the app behind it. True when the key was consumed.
///
/// This is the one handler the composer and the window share: a field's own Return would break
/// the line, so the window answers Return first and lets Shift+Return through to the field. The
/// palette and the command list are answered here first, because they own the keys they name.
pub fn composer_key<L: Live>(shell: &mut Shell<L>, ev: &KeyEvent, tx: &mut Tx) -> bool {
    // The answer field is open, so it owns every press the card does not name.
    if shell.typing() {
        return answer_key(shell, ev, tx);
    }
    if shell.palette_key(ev) {
        return true;
    }
    // Typing `/` into an empty box asks for the list; the field never sees the key, so the caret
    // is placed after the slash here rather than by the edit.
    if opens_menu(shell, ev) {
        shell.start_command();
        return true;
    }
    if shell.menu_key(ev) {
        return true;
    }
    match intent(ev, shell.waiting) {
        // A line break is the field's to insert, and nothing else does it for us.
        Intent::Ignore | Intent::Newline => {
            if matches!(ev.key, Key::Text(_)) {
                shell.typed();
            }
            false
        }
        Intent::Submit => {
            shell.submit();
            drain(shell, tx);
            true
        }
        Intent::Cancel => {
            shell.interrupt();
            true
        }
        Intent::Approve => {
            shell.approve();
            true
        }
        Intent::AllowForSession => {
            shell.allow_for_session();
            true
        }
        Intent::Deny => {
            shell.deny();
            true
        }
        Intent::Select(index) => shell.select_option(index),
        // Enter is consumed either way: an unanswered confirmation must not type a newline.
        Intent::Confirm => {
            shell.confirm();
            true
        }
        // The field is new, so the focus moves to it: the next press must land in the answer,
        // not in the composer underneath.
        Intent::Type => {
            if shell.toggle_typing() {
                tx.focus("answer-field", true);
                true
            } else {
                false
            }
        }
        // The gear takes this same path: one action, two ways to ask for it.
        Intent::Settings => {
            shell.toggle_settings();
            true
        }
        // ⌘K, wherever the keyboard is: one list over the commands and the app actions.
        Intent::Palette => {
            shell.toggle_palette();
            true
        }
    }
}
