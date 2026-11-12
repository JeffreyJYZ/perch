//! The settings file's own tests: the fields it holds, and the file it writes.

use super::*;

#[test]
fn rules_live_beside_the_settings_a_person_edits() {
    let dir = Path::new("/data/perch");
    assert_eq!(rules_path(dir), Path::new("/data/perch/permissions.jsonl"));
    assert_eq!(rules_path(dir).parent(), path_in(dir).parent());
}

#[test]
fn a_font_size_is_optional_and_round_trips_only_when_it_is_set() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(path_in(dir.path()), "theme = \"dark\"\n").unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    assert_eq!(
        settings.font_size, None,
        "a file written before the field existed draws today's scale"
    );

    settings.save(dir.path()).unwrap();
    let text = std::fs::read_to_string(path_in(dir.path())).unwrap();
    assert!(
        !text.contains("font_size"),
        "a size nobody picked is not written:\n{text}"
    );

    let mut settings = Settings::load(dir.path()).unwrap();
    settings.font_size = Some(14.0);
    settings.save(dir.path()).unwrap();
    assert_eq!(
        Settings::load(dir.path()).unwrap().font_size,
        Some(14.0),
        "and a picked one survives a save"
    );
}

#[test]
fn the_update_check_is_empty_until_a_person_names_a_manifest() {
    assert_eq!(
        Settings::default().update_check,
        "",
        "the default must not reach the network"
    );
}

#[test]
fn a_file_without_the_update_key_still_loads() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(path_in(dir.path()), "theme = \"light\"\n").unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    assert_eq!(settings.theme, "light");
    assert_eq!(settings.update_check, "");
}

#[test]
fn a_update_check_url_written_by_hand_is_read_and_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let url = "https://release.perch.test/latest.json";
    std::fs::write(path_in(dir.path()), format!("update_check = \"{url}\"\n")).unwrap();

    let settings = Settings::load(dir.path()).unwrap();
    assert_eq!(
        settings.update_check, url,
        "the field is read like the others"
    );

    settings.save(dir.path()).unwrap();
    assert_eq!(
        Settings::load(dir.path()).unwrap().update_check,
        url,
        "a save keeps the URL it did not edit"
    );
}

#[test]
fn a_shell_command_is_written_only_when_a_person_states_one() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        Settings::default().shell,
        "",
        "the default is this machine's own shell, not a command"
    );

    Settings::default().save(dir.path()).unwrap();
    let text = std::fs::read_to_string(path_in(dir.path())).unwrap();
    assert!(
        !text.contains("shell"),
        "a file nobody touched does not mention the field:\n{text}"
    );

    std::fs::write(path_in(dir.path()), "shell = \"sh -l\"\n").unwrap();
    let settings = Settings::load(dir.path()).unwrap();
    assert_eq!(settings.shell, "sh -l", "the line is read as it was typed");

    settings.save(dir.path()).unwrap();
    assert_eq!(
        Settings::load(dir.path()).unwrap().shell,
        "sh -l",
        "and a save keeps it"
    );
}

#[test]
fn the_written_file_holds_a_provider_list_and_reads_back() {
    let dir = tempfile::tempdir().unwrap();
    let settings = Settings::default();
    settings.save(dir.path()).unwrap();

    let text = std::fs::read_to_string(path_in(dir.path())).unwrap();
    assert!(text.contains("[[providers]]"), "{text}");
    assert!(text.contains("active_provider"), "{text}");

    let again = Settings::load(dir.path()).unwrap();
    assert_eq!(again.providers.len(), 1);
    assert_eq!(again.active_provider, DEFAULT_PROFILE);
}

#[test]
fn the_keybinds_table_is_written_only_when_a_chord_is_set() {
    let dir = tempfile::tempdir().unwrap();
    Settings::default().save(dir.path()).unwrap();
    let text = std::fs::read_to_string(path_in(dir.path())).unwrap();
    assert!(
        !text.contains("keybinds"),
        "an untouched table is not written at all:\n{text}"
    );

    let mut changed = Settings::load(dir.path()).unwrap();
    changed
        .keybinds
        .insert("settings".to_string(), "mod+j".to_string());
    changed.save(dir.path()).unwrap();
    let text = std::fs::read_to_string(path_in(dir.path())).unwrap();
    assert!(text.contains("[keybinds]"), "{text}");
    assert!(text.contains("settings = \"mod+j\""), "{text}");
    assert_eq!(
        Settings::load(dir.path())
            .unwrap()
            .keybinds
            .get("settings")
            .map(String::as_str),
        Some("mod+j"),
        "and it reads back as it was written"
    );
}
