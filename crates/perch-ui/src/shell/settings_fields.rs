//! The one form row the settings panel repeats, and the key field's worded state.
//!
//! Split out of [`settings_view`](super::settings_view) so the panel's own file stays under the
//! 200-line budget: a field is a label over a control, and the key is the one value the panel
//! shows as a word instead of a value.

use rune::{El, col, row, text};
use rune_kit::{Field, Kit, TextInput};

use crate::files::section_label;

use super::live::Live;
use super::settings::{Panel, settings_key};
use super::state::Shell;

/// One labelled control, as the kit's form row. `key` names it for a test and for a press.
pub(super) fn field(kit: &Kit, label: &str, key: &str, control: El) -> El {
    Field::new()
        .label(label)
        .control(control)
        .build(kit)
        .key(key)
}

/// The key field, with the word the panel says instead of the key beside it.
pub(super) fn key_field<L: Live>(panel: &Panel, kit: &Kit) -> El {
    let input = TextInput::new(&panel.api_key)
        .label("API key")
        .placeholder("paste a key")
        // The word beside it needs its own room, so the field does not take the row.
        .width(Some(420.0))
        .on_edit(|shell: &mut Shell<L>| {
            &mut shell.settings.as_mut().expect("the panel is open").api_key
        })
        .on_key(settings_key::<L>)
        .build(kit);
    col()
        .key("settings-key")
        .w_full()
        .gap(6.0)
        .child(section_label(kit, "API key", "settings-key-label"))
        .child(
            row().w_full().gap(10.0).center_y().child(input).child(
                text(panel.key_words().to_string())
                    .key("settings-key-state")
                    .font(kit.ty.small_medium)
                    .color(kit.c.text_muted),
            ),
        )
}
