//! The keybinds section of the settings panel: every intent, the chord it answers to, and the
//! field that takes a new one.
//!
//! A field is empty until a chord is typed into it: an empty field means the intent keeps the
//! chord it has, and a save that typed nothing writes nothing at all.

use rune::{El, col, row, text};
use rune_kit::{Kit, TextInput};

use crate::files::section_label;

use super::keybind::KeybindView;
use super::live::Live;
use super::settings::{Panel, settings_key};
use super::state::Shell;

/// The panel's keybinds section: the intents, their chords, and the fields that take a new one.
pub(super) fn keybinds<L: Live>(panel: &Panel, kit: &Kit) -> El {
    let mut body = col()
        .key("settings-keybinds")
        .w_full()
        .gap(6.0)
        .child(section_label(kit, "Keybinds", "settings-keybinds-label"));
    for (index, view) in panel.keybinds.views.iter().enumerate() {
        body = body.child(row_for::<L>(panel, index, view, kit));
    }
    body
}

/// One intent: the chord it answers to, and the field that takes a new one.
///
/// A saved chord that could not be used is said in words under its row, so a binding the window
/// ignored is never a silent one.
fn row_for<L: Live>(panel: &Panel, index: usize, view: &KeybindView, kit: &Kit) -> El {
    let field = panel
        .keybinds
        .field(index)
        .expect("a field for every intent the pane lists");
    let input = TextInput::new(field)
        .label(view.label.clone())
        .placeholder(&view.chord)
        .width(Some(220.0))
        .on_edit(move |shell: &mut Shell<L>| {
            shell
                .settings
                .as_mut()
                .expect("the panel is open")
                .keybinds
                .field_mut(index)
                .expect("a field for every intent the pane lists")
        })
        .on_key(settings_key::<L>)
        .build(kit);
    let mut body = col()
        .key(format!("settings-keybind-{index}"))
        .w_full()
        .gap(4.0)
        .child(
            row()
                .key(format!("settings-keybind-row-{index}"))
                .w_full()
                .gap(10.0)
                .center_y()
                .child(
                    text(view.label.clone())
                        .key(format!("settings-keybind-label-{index}"))
                        .font(kit.ty.small_medium)
                        .color(kit.c.text_base)
                        .w(160.0),
                )
                .child(
                    text(view.chord.clone())
                        .key(format!("settings-keybind-chord-{index}"))
                        .font(kit.ty.mono)
                        .color(kit.c.text_muted)
                        .w(120.0),
                )
                .child(input),
        );
    if let Some(words) = &view.refusal {
        body = body.child(
            text(words.clone())
                .key(format!("settings-keybind-refusal-{index}"))
                .font(kit.ty.small)
                .color(kit.c.danger.fg),
        );
    }
    body
}
