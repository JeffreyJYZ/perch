//! A corrupt rules file is an error the caller sees, never an empty list that reads as "allow".

use perch_agent::Rules;

use crate::harness::at;

#[test]
fn a_corrupt_file_is_an_error_and_an_empty_one_is_simply_empty() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(at(tmp.path()).rules().is_empty(), "absent is not corrupt");

    let file = tmp.path().join("permissions.jsonl");
    std::fs::write(
        &file,
        "{\"tool\":\"bash\",\"decision\":\"allow\"}\nnot json\n",
    )
    .unwrap();

    let error = Rules::load(&file).expect_err("a corrupt file must not load");
    let text = error.to_string();
    assert!(text.contains("permissions.jsonl"), "{text}");
    assert!(text.contains("line 2"), "the bad line is named: {text}");
}
