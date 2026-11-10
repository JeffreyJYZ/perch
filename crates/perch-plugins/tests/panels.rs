//! The panel a plugin declares, and what a fault inside it costs.
//!
//! A panel is data: text, rows of text, named buttons and named inputs. These tests read the
//! model the host decoded from the guest, so content is proved without a pixel, and then prove
//! that a guest which faults while producing one is disabled instead of taking the host down.

use std::path::{Path, PathBuf};

use perch_plugins::{PanelSection, PluginHost};
use perch_tools::ToolRegistry;

/// The checked-in fixtures live beside the crate, and a test's working directory is not
/// something to rely on.
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

#[test]
fn a_panel_is_data_with_named_controls() {
    let mut registry = ToolRegistry::builtin();
    let host = PluginHost::load(&fixtures(), &mut registry).unwrap();

    let model = host
        .panel_model("counter")
        .expect("the counter fixture declares a panel");
    assert_eq!(model.title, "Counter");
    assert!(
        model
            .sections
            .iter()
            .any(|section| matches!(section, PanelSection::Text(text) if text == "count: 0")),
        "the panel's text is missing: {:?}",
        model.sections
    );
    assert!(
        model.sections.iter().any(
            |section| matches!(section, PanelSection::Button { id, label } if id == "reset" && label == "Reset")
        ),
        "the panel's button is missing: {:?}",
        model.sections
    );
    assert!(
        model.sections.iter().any(
            |section| matches!(section, PanelSection::Input { id, label } if id == "note" && label == "Note")
        ),
        "the panel's input is missing: {:?}",
        model.sections
    );

    // A name no plugin has is `None`, never a panic and never another plugin's panel.
    assert!(host.panel_model("no-such-plugin").is_none());
}

#[test]
fn a_plugin_that_traps_inside_panel_is_disabled_not_the_host() {
    let mut registry = ToolRegistry::builtin();
    let host = PluginHost::load(&fixtures().join("trap-panel"), &mut registry).unwrap();

    assert_eq!(host.len(), 1, "the trapping fixture did not load");
    assert!(
        host.panel_model("trap-panel").is_none(),
        "a panel that trapped was returned anyway"
    );

    // Disabled is the plugin's state, not the host's: everything it contributed is gone, and
    // the host still answers.
    assert_eq!(host.len(), 0, "the trapping plugin stayed loaded");
    assert!(
        host.names().is_empty(),
        "a disabled plugin still lists tools"
    );
    assert!(host.panel_model("trap-panel").is_none());
    assert!(registry.specs().iter().any(|spec| spec.name == "bash"));
}
