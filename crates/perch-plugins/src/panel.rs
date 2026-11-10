//! The panel model a plugin declares, in the host's own types.
//!
//! A panel is data and only data: text, rows of text, named buttons and named inputs. Nothing
//! here can position, size, colour or draw anything, so a plugin has no way to put content over
//! the app's chrome; the host owns layout, theme and hit-testing and renders this model itself.
//! The types are deliberately the host's rather than the WIT bindings', so the app's rendering
//! code never depends on the ABI and a WIT revision cannot reach into the UI.

use crate::abi::exports::perch::plugin::panels::{
    PanelButton as WitButton, PanelInput as WitInput, PanelModel as WitModel,
    PanelSection as WitSection,
};

/// One plugin's panel: a title and the sections the host draws in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelModel {
    /// The panel's heading.
    pub title: String,
    /// What the panel shows, top to bottom.
    pub sections: Vec<PanelSection>,
}

/// One piece of a panel.
///
/// Every variant is content the host knows how to lay out. There is no variant for a shape, a
/// colour, a coordinate or a nested element, which is what keeps a panel from becoming a canvas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PanelSection {
    /// Paragraphs of text; a newline in the string is a line break.
    Text(String),
    /// Pre-formed rows, one line each — a small table without column types.
    Rows(Vec<String>),
    /// A button the host draws and reports a press for by `id`.
    Button {
        /// The control's name, which a press is reported with.
        id: String,
        /// What the button says.
        label: String,
    },
    /// A text field the host draws and reports a submit for by `id`.
    Input {
        /// The control's name, which a submit is reported with.
        id: String,
        /// The field's label.
        label: String,
    },
}

impl PanelModel {
    /// Converts what the guest returned into the host's model.
    pub(crate) fn from_wit(model: WitModel) -> PanelModel {
        PanelModel {
            title: model.title,
            sections: model.sections.into_iter().map(section_from_wit).collect(),
        }
    }

    /// Whether the panel declares `control` as a button or an input, which is what a press may
    /// address. Text and rows are content, so an id that only looks like one of them is not a
    /// control a plugin can be reported a press for.
    pub fn has_control(&self, control: &str) -> bool {
        self.sections.iter().any(|section| match section {
            PanelSection::Button { id, .. } | PanelSection::Input { id, .. } => id == control,
            PanelSection::Text(_) | PanelSection::Rows(_) => false,
        })
    }
}

fn section_from_wit(section: WitSection) -> PanelSection {
    match section {
        WitSection::Text(body) => PanelSection::Text(body),
        WitSection::Rows(rows) => PanelSection::Rows(rows),
        WitSection::Button(WitButton { id, label }) => PanelSection::Button { id, label },
        WitSection::Input(WitInput { id, label }) => PanelSection::Input { id, label },
    }
}
