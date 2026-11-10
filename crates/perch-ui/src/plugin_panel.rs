//! The host's renderer for a plugin panel.
//!
//! A plugin hands over data — a title, text, rows, named buttons and named inputs — and this
//! module is the only thing that turns it into elements. The panel can therefore never position
//! itself, colour itself or draw over the app's chrome: it gets the same fonts, spacing and
//! surfaces as every other region, and the shell owns the presses.
//!
//! [`panel_lines`] is the flattening the tests and any text-only surface use: text and rows
//! become lines, while buttons and inputs stay controls.

use perch_plugins::{PanelModel, PanelSection};
use rune::{El, col, row, text};
use rune_kit::Kit;

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
#[must_use]
pub fn panel_view(model: &PanelModel) -> El {
    let kit = Kit::neutral(true);
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
        panel = panel.child(section_view(&kit, index, section));
    }
    panel
}

/// One section. The index keys the element so a panel that changes keeps the others in place.
fn section_view(kit: &Kit, index: usize, section: &PanelSection) -> El {
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
        // Drawn as a control, named by its role, and keyed by the id a press will report. The
        // press itself belongs to the shell, which holds the plugin host a click must reach.
        PanelSection::Button { id, label } => row()
            .key(format!("panel-button-{id}"))
            .role("button")
            .label(label.clone())
            .focusable(true)
            .px(10.0)
            .py(4.0)
            .radius(kit.radius.sm)
            .fill(kit.c.layer_01)
            .child(
                text(label.clone())
                    .font(kit.ty.small)
                    .color(kit.c.text_base),
            ),
        // A labelled field outline. The editable text lives in the shell's state, so this view
        // stays pure and a submit is reported by the section's id.
        PanelSection::Input { id, label } => col()
            .key(format!("panel-input-{id}"))
            .w_full()
            .gap(2.0)
            .child(
                text(label.clone())
                    .font(kit.ty.small)
                    .color(kit.c.text_faint),
            )
            .child(
                row()
                    .key(format!("panel-input-field-{id}"))
                    .role("textbox")
                    .label(label.clone())
                    .w_full()
                    .min_h(24.0)
                    .px(8.0)
                    .radius(kit.radius.sm)
                    .fill(kit.c.bg_base),
            ),
    }
}
