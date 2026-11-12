//! The form rows the settings panel repeats: a labelled field, the theme choice, the font size,
//! and the key field's worded state.
//!
//! Split out of [`settings_view`](super::settings_view) so the panel's own file stays under the
//! 200-line budget: a field is a label over a control, and the key is the one value the panel
//! shows as a word instead of a value.

use rune::{El, Tx, col, row, text};
use rune_kit::{Field, Kit, SegmentedControl, TextInput};

use crate::files::{note, section_label};
use crate::kit::clamp_size;
use crate::settings::THEMES;

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

/// The theme choice: the two built-ins, and the name the library's own file gave it.
pub(super) fn theme_field<L: Live>(panel: &Panel, kit: &Kit) -> El {
    col()
        .key("settings-theme")
        .w_full()
        .gap(6.0)
        .child(section_label(kit, "Theme", "settings-theme-label"))
        .child(
            SegmentedControl::new(
                &THEMES,
                THEMES.iter().position(|offered| *offered == panel.theme),
            )
            .label("Theme")
            .width(Some(220.0))
            .on_pick(|shell: &mut Shell<L>, index: usize, _: &mut Tx| {
                if let (Some(theme), Some(panel)) = (THEMES.get(index), shell.settings.as_mut()) {
                    panel.theme = (*theme).to_string();
                }
            })
            .build(kit),
        )
}

/// The font size, with the clamp's own words while the typed number is out of the scale's bounds.
///
/// A number outside the bounds is not a refusal: it is drawn at the nearest bound and the line says
/// where it landed, so the pane never refuses a size for being bold.
pub(super) fn font_field<L: Live>(panel: &Panel, refused: bool, kit: &Kit) -> El {
    let input = TextInput::new(&panel.font_size)
        .label("Font size")
        .placeholder("13")
        .width(None)
        .invalid(refused)
        .on_edit(|shell: &mut Shell<L>| {
            &mut shell
                .settings
                .as_mut()
                .expect("the panel is open")
                .font_size
        })
        .on_key(settings_key::<L>)
        .build(kit);
    let mut body = col().key("settings-font").w_full().gap(6.0).child(field(
        kit,
        "Font size",
        "settings-font-size",
        input,
    ));
    if let Some(words) = clamp_note(&panel.font_size.text) {
        body = body.child(note(kit, &words).key("settings-font-note"));
    }
    body
}

/// The line [`clamp_size`] would say for `text`, when it is a number the scale clamps.
fn clamp_note(text: &str) -> Option<String> {
    let size: f32 = text.trim().parse().ok()?;
    size.is_finite().then(|| clamp_size(size).1).flatten()
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
