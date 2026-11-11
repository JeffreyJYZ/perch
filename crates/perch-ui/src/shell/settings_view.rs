//! The settings panel's elements: one labelled field per value, the theme choice, and Save.
//!
//! The view draws the panel's state and attaches its presses; nothing here writes anything. A
//! refusal is one line under the fields that names the field and says what is wrong with it, and
//! the key is shown as a word ("stored" or "missing") rather than a value, because the value is
//! the keychain's and never the window's.

use rune::{El, LiveRegion, ScrollSpec, Tx, col, row, text};
use rune_kit::glyphs;
use rune_kit::{
    Button, ButtonVariant, IconButton, IconButtonVariant, Kit, SegmentedControl, TextInput,
};

use crate::files::{note, section_label};
use crate::settings::THEMES;

use super::live::Live;
use super::model_picker_view::settings_model;
use super::settings::settings_key;
use super::settings_fields::{field, key_field};
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
        .child(header::<L>(
            kit,
            if panel.session_only {
                "Model"
            } else {
                "Settings"
            },
        ));
    if panel.session_only {
        body = body
            .child(note(
                kit,
                "This changes the current session only. The saved default is not touched.",
            ))
            .child(settings_model::<L>(panel, false, kit));
    } else {
        body = body
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
            .child(settings_model::<L>(
                panel,
                refused.is_some_and(|e| e.field == crate::settings::Field::Model),
                kit,
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
            .child(field(
                kit,
                "Update check URL",
                "settings-update-check",
                TextInput::new(&panel.update_check)
                    .label("Update check URL")
                    .placeholder("empty means no check")
                    .width(None)
                    .invalid(
                        refused.is_some_and(|e| e.field == crate::settings::Field::UpdateCheck),
                    )
                    .on_edit(|shell: &mut Shell<L>| {
                        &mut shell
                            .settings
                            .as_mut()
                            .expect("the panel is open")
                            .update_check
                    })
                    .on_key(settings_key::<L>)
                    .build(kit),
            ))
            .child(note(
                kit,
                "Empty is no check: perch stays off the network until a manifest is named.",
            ))
            .child(key_field::<L>(panel, kit))
            .child(note(
                kit,
                "The key goes to the keychain, never to settings.toml.",
            ));
    }
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
    if panel.session_only {
        return body;
    }
    body.child(actions::<L>(kit))
}

/// The title and the one way out that is not Save.
fn header<L: Live>(kit: &Kit, title: &str) -> El {
    row()
        .key("settings-header")
        .w_full()
        .center_y()
        .child(
            text(title.to_string())
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
