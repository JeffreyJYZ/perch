//! The two ways in: the ⌘, press and the gear, which open and close the same panel.

use perch_ui::Shell;
use perch_ui::shell::{Intent, Waiting, composer_key, intent};
use rune::Tx;

use crate::harness::{cmd_comma, layout};

#[test]
fn the_gear_and_cmd_comma_open_and_close_the_same_panel() {
    assert_eq!(intent(&cmd_comma(), Waiting::Nothing), Intent::Settings);

    // The key, applied by the one handler the composer and the window share.
    let mut typed = Shell::idle(Vec::new(), 0);
    assert!(
        composer_key(&mut typed, &cmd_comma(), &mut Tx::default()),
        "⌘, is consumed by the window"
    );
    assert!(typed.settings_open(), "⌘, opens the settings");

    // The gear, pressed where the window drew it.
    let mut runtime = layout(Shell::idle(Vec::new(), 0));
    let gear = runtime
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some("settings-gear"))
        .expect("the tab row draws the gear");
    assert_eq!(gear.role, "button");
    assert_eq!(gear.label.as_deref(), Some("Settings"));
    let at = (
        gear.rect[0] + gear.rect[2] * 0.5,
        gear.rect[1] + gear.rect[3] * 0.5,
    );
    runtime.pointer_down(at.0, at.1);
    runtime.pointer_up(at.0, at.1);
    runtime.frame(0.0);
    assert!(runtime.state.settings_open(), "the gear opens the settings");

    assert_eq!(
        runtime.state.settings_panel().map(|panel| panel.theme()),
        typed.settings_panel().map(|panel| panel.theme()),
        "both ways reach the same panel"
    );

    // And the same press again is the same press: both close it.
    assert!(composer_key(&mut typed, &cmd_comma(), &mut Tx::default()));
    assert!(!typed.settings_open(), "⌘, closes the panel it opened");
    let gear = runtime
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some("settings-gear"))
        .expect("the gear is still drawn");
    let at = (
        gear.rect[0] + gear.rect[2] * 0.5,
        gear.rect[1] + gear.rect[3] * 0.5,
    );
    runtime.pointer_down(at.0, at.1);
    runtime.pointer_up(at.0, at.1);
    runtime.frame(0.0);
    assert!(!runtime.state.settings_open(), "the gear closes it too");
}
