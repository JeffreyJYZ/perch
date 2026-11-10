//! The prompt: one multi-line field, and the presses that send or stop a turn.

use rune::{El, KeyEvent, Tx, col};
use rune_kit::Kit;
use rune_kit::TextArea;

use super::keymap::{Intent, intent};
use super::live::Live;
use super::state::Shell;
use crate::files::section_label;

/// The prompt: one multi-line field, in its own labelled section.
pub(super) fn composer<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    col()
        .key("composer")
        .w_full()
        .px(20.0)
        .pt(4.0)
        .pb(12.0)
        .gap(4.0)
        .child(section_label(kit, "Prompt", "prompt-label"))
        .child(
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

/// Sends what the box holds, as the same path Enter takes.
fn submit<L: Live>(shell: &mut Shell<L>, _: &mut Tx) {
    shell.submit();
}

/// Applies one press to the shell through the app behind it. True when the key was consumed.
///
/// This is the one handler the composer and the window share: a field's own Return would break
/// the line, so the window answers Return first and lets Shift+Return through to the field.
pub fn composer_key<L: Live>(shell: &mut Shell<L>, ev: &KeyEvent, _: &mut Tx) -> bool {
    match intent(ev, shell.waiting) {
        // A line break is the field's to insert, and nothing else does it for us.
        Intent::Ignore | Intent::Newline => false,
        Intent::Submit => {
            shell.submit();
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
    }
}
