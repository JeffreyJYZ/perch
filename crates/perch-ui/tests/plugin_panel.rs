//! The host's renderer for a plugin panel.
//!
//! A panel model is data, so its content is proved by flattening it into lines: text and rows
//! become lines, buttons and inputs stay controls the host draws. The view test then reads the
//! accessibility tree, which is how the app's own tests prove what a region says without pixels.

use std::path::{Path, PathBuf};

use perch_plugins::{PanelModel, PanelSection, PluginHost};
use perch_store::SessionEvent;
use perch_tools::ToolRegistry;
use perch_ui::{panel_lines, panel_view};
use rune::{Cx, El, Env, Runtime};

/// The plugin fixtures live in the sibling crate; a test's working directory is not reliable.
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../perch-plugins/fixtures")
}

fn host() -> (PluginHost, ToolRegistry) {
    let mut registry = ToolRegistry::builtin();
    let host = PluginHost::load(&fixtures(), &mut registry).unwrap();
    (host, registry)
}

#[test]
fn a_panel_model_decodes_into_host_rendered_lines() {
    let (host, _) = host();
    let model = host.panel_model("counter").unwrap();
    assert_eq!(model.title, "Counter");
    assert_eq!(panel_lines(&model), vec!["count: 0".to_string()]);
}

#[test]
fn a_session_event_reaches_the_plugin_and_changes_its_panel() {
    let (mut host, _) = host();
    host.dispatch_event(&SessionEvent::TurnEnd {
        reason: "done".into(),
        tokens_in: 1,
        tokens_out: 2,
    });
    assert_eq!(
        panel_lines(&host.panel_model("counter").unwrap()),
        vec!["count: 1".to_string()]
    );
}

#[test]
fn panel_lines_flatten_text_and_rows_and_leave_controls_to_the_host() {
    let model = PanelModel {
        title: "Build".to_string(),
        sections: vec![
            PanelSection::Text("branch: main\nstatus: green".to_string()),
            PanelSection::Rows(vec!["job a".to_string(), "job b".to_string()]),
            PanelSection::Button {
                id: "rerun".to_string(),
                label: "Rerun".to_string(),
            },
            PanelSection::Input {
                id: "note".to_string(),
                label: "Note".to_string(),
            },
        ],
    };

    assert_eq!(
        panel_lines(&model),
        vec![
            "branch: main".to_string(),
            "status: green".to_string(),
            "job a".to_string(),
            "job b".to_string(),
        ],
        "text and rows are lines; a button and an input are controls, not text"
    );
}

fn view(model: &PanelModel, _: &mut Cx<PanelModel>) -> El {
    panel_view(model)
}

/// The accessibility tree of the panel, which carries every word and control it draws.
fn rendered(model: PanelModel) -> String {
    let mut runtime = Runtime::new(model, view, (360.0, 600.0), Env::new(true, 2.0));
    runtime.frame(0.0);
    runtime.accessibility_json().to_string()
}

#[test]
fn the_view_draws_the_title_text_rows_and_named_controls() {
    let model = PanelModel {
        title: "Build".to_string(),
        sections: vec![
            PanelSection::Text("branch: main".to_string()),
            PanelSection::Rows(vec!["job a".to_string(), "job b".to_string()]),
            PanelSection::Button {
                id: "rerun".to_string(),
                label: "Rerun".to_string(),
            },
            PanelSection::Input {
                id: "note".to_string(),
                label: "Note".to_string(),
            },
        ],
    };

    let tree = rendered(model);
    for word in ["Build", "branch: main", "job a", "job b", "Rerun", "Note"] {
        assert!(tree.contains(word), "the panel is missing {word}:\n{tree}");
    }
    // The button is a named role the host can attach a press to by id. The field is Rune's
    // keyed textbox container: a pure view cannot own the editor, so the shell puts the
    // editable inside this keyed element and Rune routes presses to it.
    assert!(tree.contains("button"), "no button role:\n{tree}");
    assert!(
        tree.contains("panel-input-field-note"),
        "no input field for the `note` id:\n{tree}"
    );

    // The counter fixture's real panel renders too, which is the path the shell will take.
    let (host, _) = host();
    let tree = rendered(host.panel_model("counter").unwrap());
    assert!(tree.contains("count: 0"), "{tree}");
}
