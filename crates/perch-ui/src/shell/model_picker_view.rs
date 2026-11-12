//! The picker as the window draws it: a searchable list in the settings panel, and the composer's
//! one control that opens that same list for the current session.
//!
//! Neither view writes anything: a row's press calls the shell, which asks the app. The settings
//! panel falls back to its typed field only when there is no list to draw, so a provider without
//! the listing endpoint is still configurable.

use rune::{El, ScrollSpec, Tx, col, row, text};
use rune_kit::{Button, ButtonVariant, Kit, List, ListItem, TextInput};

use crate::files::{note, section_label};

use super::live::Live;
use super::model_picker::{ModelPicker, SHOWN};
use super::settings::{Panel, settings_key};
use super::state::Shell;

/// The height the settings list may take before it scrolls inside itself.
const LIST_HEIGHT: f32 = 180.0;

/// The settings panel's model section: the provider's list, or the typed field when it cannot be
/// listed, with the reason in words above it either way.
pub(super) fn settings_model<L: Live>(panel: &Panel, invalid: bool, kit: &Kit) -> El {
    let picker = &panel.picker;
    let mut body = col().key("settings-model").w_full().gap(6.0).child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(section_label(kit, "Model", "settings-model-label"))
            .child(
                Button::new(if panel.models_open {
                    "Hide list"
                } else {
                    "Show list"
                })
                .variant(ButtonVariant::GhostMuted)
                .on_press(|shell: &mut Shell<L>| shell.toggle_models())
                .build(kit)
                .key("settings-models-fold"),
            )
            .child(
                Button::new("Refresh list")
                    .variant(ButtonVariant::GhostMuted)
                    .on_press(|shell: &mut Shell<L>| shell.refresh_models())
                    .build(kit)
                    .key("settings-models-refresh"),
            ),
    );
    if picker.loading() {
        body = body.child(note(
            kit,
            "asking the provider for its models. Typing an id works meanwhile.",
        ));
    }
    if let Some(failure) = picker.failure() {
        body = body
            .child(
                text(format!("models not listed: {failure}"))
                    .key("settings-models-failure")
                    .font(kit.ty.small)
                    .color(kit.c.danger.fg),
            )
            .child(
                TextInput::new(&panel.model)
                    .label("Model")
                    .placeholder("type the model id")
                    .width(None)
                    .invalid(invalid)
                    .on_edit(|shell: &mut Shell<L>| {
                        &mut shell.settings.as_mut().expect("the panel is open").model
                    })
                    .on_key(settings_key::<L>)
                    .build(kit),
            )
            .child(note(
                kit,
                "Typed ids still work: a provider need not list its models.",
            ));
    } else {
        // The list is capped and scrolls on its own: grown to its full height it would fill the
        // pane and take the wheel, leaving the fields under it unreachable.
        if panel.models_open {
            body = body.child(
                col()
                    .key("settings-models-viewport")
                    .w_full()
                    .max_h(LIST_HEIGHT)
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    })
                    .child(picker_list::<L>(
                        picker,
                        &panel.model.text,
                        SHOWN,
                        "settings-models",
                        |shell: &mut Shell<L>| {
                            &mut shell
                                .settings
                                .as_mut()
                                .expect("the panel is open")
                                .picker
                                .query
                        },
                        |id| {
                            Box::new(move |shell: &mut Shell<L>, _: &mut Tx| {
                                shell.pick_settings_model(&id);
                            })
                        },
                        kit,
                    )),
            );
        } else {
            body = body.child(note(
                kit,
                "The list is folded, so the fields below stay in reach. Show list unfolds it.",
            ));
        }
    }
    body
}

/// The composer's model control: what this session asks for, and the press that changes it.
///
/// The list itself takes the transcript's place (see [`Shell::open_model_panel`]): the prompt
/// column has no room for it, and a list drawn there would sit under the dock where a press never
/// reaches it.
pub(super) fn composer_model<L: Live>(shell: &Shell<L>, kit: &Kit) -> El {
    let current = shell.current_model();
    let empty = current.is_empty();
    let word = if empty {
        "none chosen".to_string()
    } else {
        current
    };
    let ink = if empty {
        kit.c.danger.fg
    } else {
        kit.c.text_muted
    };
    row()
        .key("composer-model")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(section_label(kit, "Model", "composer-model-label"))
        .child(
            text(word)
                .key("composer-model-current")
                .font(kit.ty.small_medium)
                .color(ink)
                .grow(1.0)
                .truncate(),
        )
        .child(
            Button::new(if empty { "Choose a model" } else { "Change" })
                .variant(ButtonVariant::Outline)
                .on_press(|shell: &mut Shell<L>| shell.open_model_panel())
                .build(kit)
                .key("composer-model-change"),
        )
}

/// What a row press is: the id it carries, turned into the press that picks it.
type RowPress<L> = fn(String) -> Box<dyn Fn(&mut Shell<L>, &mut Tx)>;

/// One searchable list of model rows, the chosen one marked, capped at `limit` rows.
fn picker_list<L: Live>(
    picker: &ModelPicker,
    current: &str,
    limit: usize,
    key: &str,
    filter: fn(&mut Shell<L>) -> &mut rune::TextEdit,
    press: RowPress<L>,
    kit: &Kit,
) -> El {
    let (rows, total) = picker.rows(limit);
    let mut list = List::new()
        .search(picker.query(), "Search models")
        .on_search_edit(filter);
    for id in rows {
        list = list.item(
            ListItem::new(id.clone())
                .selected(id == current)
                .on_select(press(id)),
        );
    }
    let mut body = col().key(key).w_full().gap(4.0).child(list.build(kit));
    if total > limit {
        body = body.child(note(
            kit,
            &format!("showing the first {limit} of {total} matches: type to narrow"),
        ));
    }
    body
}
