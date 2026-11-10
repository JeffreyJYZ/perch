//! The settings panel's elements: one labelled field per value, the theme choice, and Save.
//!
//! The view draws the panel's state and attaches its presses; nothing here writes anything. A
//! refusal is one line under the fields that names the field and says what is wrong with it, and
//! the key is shown as a word ("stored" or "missing") rather than a value, because the value is
//! the keychain's and never the window's.

use rune::{El, LiveRegion, ScrollSpec, Tx, col, row, text};
use rune_kit::glyphs;
use rune_kit::{
    Button, ButtonVariant, Field, IconButton, IconButtonVariant, Kit, SegmentedControl, TextInput,
};

use crate::files::{note, section_label};
use crate::settings::THEMES;

use super::live::Live;
use super::settings::settings_key;
use super::state::Shell;

/// The panel itself, in the centre of the window where the transcript was.
pub(super) fn panel<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    let Some(panel) = shell.settings.as_ref() else {
        return col();
    };
    let refused = panel.error();
    let mut body = col()
        .key("settings")
        .role("region")
        .label("Settings")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .px(24.0)
        .py(20.0)
        .gap(16.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(header::<L>(kit))
        .child(field(
            kit,
            "Base URL",
            "settings-base-url",
            TextInput::new(&panel.base_url)
                .label("Base URL")
                .placeholder("https://openrouter.ai/api/v1")
                .width(None)
                .invalid(refused.is_some_and(|e| e.field == crate::settings::Field::BaseUrl))
                .on_edit(|shell: &mut Shell<L>| {
                    &mut shell.settings.as_mut().expect("the panel is open").base_url
                })
                .on_key(settings_key::<L>)
                .build(kit),
        ))
        .child(field(
            kit,
            "Model",
            "settings-model",
            TextInput::new(&panel.model)
                .label("Model")
                .placeholder("deepseek/deepseek-v4.1-flash")
                .width(None)
                .invalid(refused.is_some_and(|e| e.field == crate::settings::Field::Model))
                .on_edit(|shell: &mut Shell<L>| {
                    &mut shell.settings.as_mut().expect("the panel is open").model
                })
                .on_key(settings_key::<L>)
                .build(kit),
        ))
        .child(
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
                        if let (Some(theme), Some(panel)) =
                            (THEMES.get(index), shell.settings.as_mut())
                        {
                            panel.theme = (*theme).to_string();
                        }
                    })
                    .build(kit),
                ),
        )
        .child(key_field::<L>(panel, kit))
        .child(note(
            kit,
            "The key goes to the keychain, never to settings.toml.",
        ));
    if let Some(invalid) = refused {
        body = body.child(
            text(invalid.words.clone())
                .key("settings-error")
                .font(kit.ty.small)
                .color(kit.c.danger.fg)
                .live_region(LiveRegion::Assertive),
        );
    }
    if let Some(notice) = panel.notice() {
        body = body.child(
            text(notice.to_string())
                .key("settings-notice")
                .font(kit.ty.small)
                .color(kit.c.text_muted),
        );
    }
    body.child(actions::<L>(kit))
}

/// The title and the one way out that is not Save.
fn header<L: Live>(kit: &Kit) -> El {
    row()
        .key("settings-header")
        .w_full()
        .center_y()
        .child(
            text("Settings".to_string())
                .w_full()
                .grow(1.0)
                .font(kit.ty.ui_medium)
                .color(kit.c.text_base),
        )
        .child(
            IconButton::new(&glyphs::CLOSE)
                .variant(IconButtonVariant::Ghost)
                .label("Close settings")
                .on_press(|shell: &mut Shell<L>| shell.close_settings())
                .build(kit)
                .key("settings-close"),
        )
}

/// One labelled control, as the kit's form row. `key` names it for a test and for a press.
fn field(kit: &Kit, label: &str, key: &str, control: El) -> El {
    Field::new()
        .label(label)
        .control(control)
        .build(kit)
        .key(key)
}

/// The key field, with the word the panel says instead of the key beside it.
fn key_field<L: Live>(panel: &super::settings::Panel, kit: &Kit) -> El {
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

/// Save, which is the only press that writes anything.
fn actions<L: Live>(kit: &Kit) -> El {
    row().key("settings-actions").w_full().gap(8.0).child(
        Button::new("Save")
            .variant(ButtonVariant::Submit)
            .on_press(|shell: &mut Shell<L>| {
                shell.save_settings();
            })
            .build(kit)
            .key("settings-save"),
    )
}
