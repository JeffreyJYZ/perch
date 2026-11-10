//! The host's renderer for a plugin panel.
//!
//! A panel model is data, so its content is proved by flattening it into lines: text and rows
//! become lines, buttons and inputs stay controls the host draws. The view test then reads the
//! accessibility tree of the whole window, which is how the app's own tests prove what a region
//! says without pixels — a panel is only drawn where the column draws it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use perch_plugins::{PanelEntry, PanelModel, PanelSection, PluginHost};
use perch_store::{SessionEvent, SessionMeta};
use perch_tools::ToolRegistry;
use perch_ui::shell::{Live, Panels, Shell, Waiting};
use perch_ui::{Transcript, panel_lines};
use rune::{Env, Runtime};

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

/// A window whose app holds one panel, so the renderer is proved without a wasm fixture. The
/// model is what the column draws; nothing else about this app is real.
struct Drawn {
    model: PanelModel,
}

impl Panels for Drawn {
    fn panels(&self) -> Vec<PanelEntry> {
        vec![PanelEntry {
            plugin: "demo".to_string(),
            model: self.model.clone(),
        }]
    }

    fn panel_action(
        &mut self,
        plugin: &str,
        control: &str,
        _value: Option<String>,
    ) -> std::result::Result<(), String> {
        Err(format!("{plugin}/{control} reached no plugin"))
    }
}

impl Live for Drawn {
    fn pump(&mut self, _: Duration) -> bool {
        false
    }
    fn transcript(&self) -> Option<&Transcript> {
        None
    }
    fn waiting(&self) -> Waiting {
        Waiting::Nothing
    }
    fn busy(&self) -> bool {
        false
    }
    fn prompt(&mut self, _: String) -> std::result::Result<(), String> {
        Ok(())
    }
    fn cancel(&mut self) {}
    fn approve(&mut self) {}
    fn deny(&mut self) {}
    fn pick(&mut self, _: usize) {}
    fn switch(&mut self, _: usize) {}
}

/// The accessibility tree of the window, which carries every word and control a panel draws.
fn rendered(model: PanelModel) -> String {
    let meta = SessionMeta {
        id: "s".to_string(),
        title: "s".to_string(),
        worktree: PathBuf::from("/work"),
        branch: "dev".to_string(),
        model: String::new(),
        created: 0,
    };
    let mut runtime = Runtime::new(
        Shell::new(vec![meta], 0).with_live(Drawn { model }),
        perch_ui::shell::view,
        (1280.0, 820.0),
        Env::new(true, 2.0),
    );
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
    // The button is a named role the host attaches a press to, keyed by the id it reports. The
    // field is a real editor inside the panel, so what a person types is what a submit carries.
    assert!(
        tree.contains("panel-button-rerun"),
        "no button for `rerun`:\n{tree}"
    );
    assert!(
        tree.contains("panel-input-field-note"),
        "no input field for the `note` id:\n{tree}"
    );

    // The counter fixture's real panel renders too, which is the path the shell takes.
    let (host, _) = host();
    let tree = rendered(host.panel_model("counter").unwrap());
    assert!(tree.contains("count: 0"), "{tree}");
    assert!(tree.contains("Bump"), "{tree}");
}
