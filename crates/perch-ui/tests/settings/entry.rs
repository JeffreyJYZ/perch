//! The two ways into the settings: the ⌘, press and the settings tab, which open the same pane.

use perch_ui::Shell;
use perch_ui::shell::{Intent, Waiting, composer_key, intent};
use rune::Tx;

use crate::harness::{click, cmd_comma, layout};

#[test]
fn the_settings_tab_and_cmd_comma_open_the_same_pane() {
    assert_eq!(intent(&cmd_comma(), Waiting::Nothing), Intent::Settings);

    // The key, applied by the one handler the composer and the window share.
    let mut typed = Shell::idle(Vec::new(), 0);
    assert!(
        composer_key(&mut typed, &cmd_comma(), &mut Tx::default()),
        "⌘, is consumed by the window"
    );
    assert!(typed.settings_open(), "⌘, opens the settings");

    // The tab, pressed where the window drew it.
    let mut runtime = layout(Shell::idle(Vec::new(), 0));
    click(&mut runtime, "t-settings");
    assert!(runtime.state.settings_open(), "the tab opens the settings");

    assert_eq!(
        runtime.state.settings_panel().map(|panel| panel.theme()),
        typed.settings_panel().map(|panel| panel.theme()),
        "both ways reach the same panel"
    );

    // And the same key again is the same press: ⌘, closes what it opened. The tab is a tab, so a
    // second press on it leaves the pane open rather than flipping it shut.
    assert!(composer_key(&mut typed, &cmd_comma(), &mut Tx::default()));
    assert!(!typed.settings_open(), "⌘, closes the pane it opened");
    click(&mut runtime, "t-settings");
    assert!(
        runtime.state.settings_open(),
        "a second press on the settings tab leaves it showing"
    );
}
