//! A press on a panel control, and what it costs when the guest cannot answer.
//!
//! A panel is data, so a control is only a name until the plugin is told about the press. These
//! tests read the model after the press, which is the same read the column draws from: a press
//! that changed nothing is visible as a model that did not move, and one that arrived is visible
//! as a line that did.

use std::path::{Path, PathBuf};

use perch_plugins::{PanelSection, PluginHost};
use perch_tools::ToolRegistry;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// Every text section of `plugin`'s panel, in order, so a change is a line rather than a diff.
fn lines(host: &PluginHost, plugin: &str) -> Vec<String> {
    host.panel_model(plugin)
        .expect("the fixture declares a panel")
        .sections
        .into_iter()
        .filter_map(|section| match section {
            PanelSection::Text(text) => Some(text),
            _ => None,
        })
        .collect()
}

#[test]
fn a_button_press_reaches_the_guest_and_its_panel_changes() {
    let mut registry = ToolRegistry::builtin();
    let host = PluginHost::load(&fixtures(), &mut registry).unwrap();
    assert_eq!(lines(&host, "counter"), ["count: 0"]);

    // A button reports no value: there is nothing to type into it.
    host.panel_action("counter", "bump", None)
        .expect("the button press reached the plugin");
    assert_eq!(
        lines(&host, "counter"),
        ["count: 1"],
        "the press did not change the panel"
    );

    host.panel_action("counter", "reset", None)
        .expect("the reset press reached the plugin");
    assert_eq!(lines(&host, "counter"), ["count: 0"]);
}

#[test]
fn an_input_submit_carries_its_text() {
    let mut registry = ToolRegistry::builtin();
    let host = PluginHost::load(&fixtures(), &mut registry).unwrap();

    host.panel_action("counter", "note", Some("hello"))
        .expect("the input submit reached the plugin");
    assert_eq!(
        lines(&host, "counter"),
        ["count: 0", "note: hello"],
        "the text a person submitted is not what the plugin saw"
    );

    // The same control again replaces what it held, rather than appending to it.
    host.panel_action("counter", "note", Some("bye")).unwrap();
    assert_eq!(lines(&host, "counter"), ["count: 0", "note: bye"]);
}

#[test]
fn a_press_for_a_plugin_that_is_gone_is_stated() {
    let mut registry = ToolRegistry::builtin();
    let host = PluginHost::load(&fixtures(), &mut registry).unwrap();

    let refused = host
        .panel_action("no-such-plugin", "bump", None)
        .expect_err("a press for no plugin was accepted");
    let words = refused.to_string();
    assert!(
        words.contains("no-such-plugin") && words.contains("not loaded"),
        "the refusal does not say what was missing: {words}"
    );

    // Nothing else was touched: the plugins that are loaded still answer.
    assert_eq!(host.len(), 1);
    assert!(host.panel_model("counter").is_some());
}

#[test]
fn a_press_for_a_control_the_plugin_never_declared_is_stated() {
    let mut registry = ToolRegistry::builtin();
    let host = PluginHost::load(&fixtures(), &mut registry).unwrap();

    let refused = host
        .panel_action("counter", "no-such-control", None)
        .expect_err("a press for an undeclared control was forwarded");
    let words = refused.to_string();
    assert!(
        words.contains("counter") && words.contains("declares no control `no-such-control`"),
        "the refusal does not name the control: {words}"
    );

    // A refusal is not a trap: the plugin is still loaded and its tools are still offered.
    assert_eq!(host.len(), 1, "a bad id disabled the plugin");
    assert_eq!(lines(&host, "counter"), ["count: 0"]);
    assert!(host.names().contains(&"echo".to_string()));
}

#[test]
fn a_press_on_a_plugin_disabled_by_a_trap_is_stated_rather_than_forwarded() {
    let mut registry = ToolRegistry::builtin();
    let host = PluginHost::load(&fixtures().join("trap-panel"), &mut registry).unwrap();

    // Its panel traps, so it is disabled before any control could be drawn.
    assert!(host.panel_model("trap-panel").is_none());
    let refused = host
        .panel_action("trap-panel", "fine", None)
        .expect_err("a press reached a disabled plugin");
    assert!(
        refused.to_string().contains("disabled"),
        "the refusal does not say the plugin is gone: {refused}"
    );
}

#[test]
fn a_press_that_traps_disables_the_plugin_like_a_trapping_tool() {
    let mut registry = ToolRegistry::builtin();
    let host = PluginHost::load(&fixtures().join("trap-action"), &mut registry).unwrap();
    assert_eq!(host.len(), 1, "the trapping fixture did not load");
    assert!(
        host.panel_model("trap-action").is_some(),
        "the fixture's panel must draw, or there is no press to make"
    );

    let refused = host
        .panel_action("trap-action", "trap", None)
        .expect_err("a trapping action answered as if it succeeded");
    let words = refused.to_string();
    assert!(
        words.contains("was disabled while answering `trap`"),
        "the refusal does not say the plugin went: {words}"
    );

    // Disabled is the plugin's state, not the host's: everything it contributed is gone.
    assert_eq!(host.len(), 0, "the trapping plugin stayed loaded");
    assert!(
        host.names().is_empty(),
        "a disabled plugin still lists tools"
    );
    assert!(host.panel_model("trap-action").is_none());
    assert_eq!(host.release(&mut registry), 1, "its tool was not released");

    // Everything else still answers.
    assert!(registry.specs().iter().any(|spec| spec.name == "bash"));
}
