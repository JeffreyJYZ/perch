//! The host's renderer for a plugin panel.
//!
//! A plugin hands over data — a title, text, rows, named buttons and named inputs — and this
//! module is the only thing that turns it into elements. The panel can therefore never position
//! itself, colour itself or draw over the app's chrome: it gets the same fonts, spacing and
//! surfaces as every other region.
//!
//! A control the panel declares is a control this module wires. A button's press and an input's
//! submit both go through the shell to the app that owns the plugin host, carrying the plugin, the
//! control's id and — for an input — the text the field holds. Nothing here calls a plugin: the
//! app does, and the column redraws from whatever model the plugin gives back.
//!
//! [`panel_lines`] is the flattening the tests and any text-only surface use: text and rows
//! become lines, while buttons and inputs stay controls.

use perch_plugins::{PanelModel, PanelSection};
use rune::{El, KeyEvent, Tx, col, row, text};
use rune_kit::{Kit, TextInput};

use crate::shell::{Live, Panels, Shell};

/// The panel's content as plain lines: each text section split on newlines, then each row.
///
/// Buttons and inputs are intentionally absent — they are controls, not prose, and a caller
/// that wants them reads the model. This is what lets panel content be asserted without pixels.
#[must_use]
pub fn panel_lines(model: &PanelModel) -> Vec<String> {
    let mut lines = Vec::new();
    for section in &model.sections {
        match section {
            PanelSection::Text(body) => lines.extend(body.lines().map(str::to_string)),
            PanelSection::Rows(rows) => lines.extend(rows.iter().cloned()),
            PanelSection::Button { .. } | PanelSection::Input { .. } => {}
        }
    }
    lines
}

/// The panel as elements: its title, then one element per section in the order declared.
///
/// `plugin` is the name a press is reported with; the model carries no name of its own, because a
/// plugin does not get to say who it is.
#[must_use]
pub fn panel_view<L: Live + Panels>(
    shell: &Shell<L>,
    plugin: &str,
    kit: &Kit,
    model: &PanelModel,
) -> El {
    let mut panel = col()
        .key("plugin-panel")
        .role("region")
        .label(model.title.clone())
        .w_full()
        .gap(8.0)
        .px(12.0)
        .py(10.0)
        .child(
            text(model.title.clone())
                .font(kit.ty.small_medium)
                .color(kit.c.text_faint),
        );
    for (index, section) in model.sections.iter().enumerate() {
        panel = panel.child(section_view(shell, plugin, kit, index, section));
    }
    panel
}

/// One section. The index keys the element so a panel that changes keeps the others in place.
fn section_view<L: Live + Panels>(
    shell: &Shell<L>,
    plugin: &str,
    kit: &Kit,
    index: usize,
    section: &PanelSection,
) -> El {
    match section {
        PanelSection::Text(body) => text(body.clone())
            .key(format!("panel-text-{index}"))
            .font(kit.ty.body)
            .color(kit.c.text_base)
            .selectable(true),
        PanelSection::Rows(rows) => {
            let mut list = col().key(format!("panel-rows-{index}")).w_full().gap(2.0);
            for (line, body) in rows.iter().enumerate() {
                list = list.child(
                    row()
                        .key(format!("panel-row-{index}-{line}"))
                        .w_full()
                        .child(
                            text(body.clone())
                                .font(kit.ty.small)
                                .color(kit.c.text_muted),
                        ),
                );
            }
            list
        }
        PanelSection::Button { id, label } => {
            let (owner, control) = (plugin.to_string(), id.clone());
            row()
                .key(format!("panel-button-{id}"))
                .role("button")
                .label(label.clone())
                .focusable(true)
                .px(10.0)
                .py(4.0)
                .radius(kit.radius.sm)
                .fill(kit.c.layer_01)
                // A button has no value: the press is the whole event.
                .on_press(move |shell: &mut Shell<L>| shell.panel_press(&owner, &control))
                .child(
                    text(label.clone())
                        .font(kit.ty.small)
                        .color(kit.c.text_base),
                )
        }
        PanelSection::Input { id, label } => {
            let (edit_owner, edit_control) = (plugin.to_string(), id.clone());
            let (key_owner, key_control) = (plugin.to_string(), id.clone());
            col()
                .key(format!("panel-input-{id}"))
                .w_full()
                .gap(2.0)
                .child(
                    text(label.clone())
                        .font(kit.ty.small)
                        .color(kit.c.text_faint),
                )
                .child(
                    TextInput::new(&shell.panel_draft(plugin, id))
                        .label(label.clone())
                        .width(None)
                        .on_edit(move |shell: &mut Shell<L>| {
                            shell.panel_draft_mut(&edit_owner, &edit_control)
                        })
                        .on_key(move |shell: &mut Shell<L>, ev: &KeyEvent, _: &mut Tx| {
                            if !submits(ev) {
                                return false;
                            }
                            // The text is read here, at the press, so what the plugin receives is
                            // what the field held rather than what it held when it was drawn.
                            shell.panel_submit(&key_owner, &key_control);
                            true
                        })
                        .build(kit)
                        .key(format!("panel-input-field-{id}")),
                )
        }
    }
}

/// Whether a press submits the field: plain Return, with no chord and no line to break.
fn submits(ev: &KeyEvent) -> bool {
    matches!(&ev.key, rune::Key::Enter) && !ev.mods.cmd && !ev.mods.alt && !ev.mods.shift
}
