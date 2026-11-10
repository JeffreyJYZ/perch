use super::*;

const DIGEST: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

fn valid() -> Value {
    serde_json::json!({
        "version": "0.1.0-alpha.4",
        "url": "https://release.perch.test/Perch-0.1.0-alpha.4.zip",
        "notes": "Fixes the wheel over the transcript.",
        "min_os": 14,
        "sha256": DIGEST,
    })
}

fn with(field: &str, value: Value) -> String {
    let mut document = valid();
    document
        .as_object_mut()
        .expect("an object")
        .insert(field.to_string(), value);
    document.to_string()
}

#[test]
fn a_complete_manifest_parses() {
    let manifest = Manifest::parse(&valid().to_string()).expect("parses");
    assert_eq!(manifest.version.to_string(), "0.1.0-alpha.4");
    assert_eq!(manifest.min_os, 14);
    assert_eq!(manifest.sha256, DIGEST);
}

#[test]
fn every_field_is_required_and_the_error_names_it() {
    for field in ["version", "url", "notes", "min_os", "sha256"] {
        let mut document = valid();
        document.as_object_mut().expect("an object").remove(field);
        let error = Manifest::parse(&document.to_string()).expect_err(field);
        assert_eq!(
            error.to_string(),
            format!("manifest field `{field}` is missing"),
            "{field}"
        );
    }
}

#[test]
fn a_version_that_does_not_parse_names_the_version_field() {
    let error = Manifest::parse(&with("version", "0.1".into())).expect_err("0.1 is not a version");
    assert!(
        matches!(
            &error,
            Error::Invalid {
                field: "version",
                ..
            }
        ),
        "{error}"
    );
}

#[test]
fn a_quoted_min_os_is_refused_by_name() {
    let error =
        Manifest::parse(&with("min_os", "14".into())).expect_err("a string is not a number");
    assert!(
        matches!(
            &error,
            Error::Invalid {
                field: "min_os",
                ..
            }
        ),
        "{error}"
    );
    assert!(error.to_string().contains("positive integer"), "{error}");
}

#[test]
fn a_checksum_that_is_not_64_hex_digits_names_the_field() {
    let error = Manifest::parse(&with("sha256", "deadbeef".into())).expect_err("too short");
    assert!(
        matches!(
            &error,
            Error::Invalid {
                field: "sha256",
                ..
            }
        ),
        "{error}"
    );
}

#[test]
fn an_uppercase_checksum_is_normalised() {
    let text = with("sha256", DIGEST.to_uppercase().into());
    let manifest = Manifest::parse(&text).expect("parses");
    assert_eq!(manifest.sha256, DIGEST);
}

#[test]
fn a_document_that_is_not_an_object_is_refused() {
    assert!(Manifest::parse("[1, 2]").is_err());
    assert!(Manifest::parse("not json").is_err());
}

#[test]
fn an_unknown_key_is_ignored() {
    let text = with("channel", "alpha".into());
    assert!(Manifest::parse(&text).is_ok());
}
