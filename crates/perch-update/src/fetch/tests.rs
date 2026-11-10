use super::*;

/// SHA-256 of the bytes "hello", computed away from this crate.
const HELLO_DIGEST: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
const MANIFEST_URL: &str = "https://release.perch.test/latest.json";
const ARCHIVE_URL: &str = "https://release.perch.test/Perch.zip";

struct Fake {
    routes: Vec<(&'static str, Vec<u8>)>,
}

impl Fake {
    fn new(routes: &[(&'static str, &str)]) -> Self {
        Self {
            routes: routes
                .iter()
                .map(|(route, body)| (*route, body.as_bytes().to_vec()))
                .collect(),
        }
    }
}

impl Transport for Fake {
    fn get(&self, url: &str) -> Result<Vec<u8>> {
        self.routes
            .iter()
            .find(|(route, _)| *route == url)
            .map(|(_, body)| body.clone())
            .ok_or_else(|| Error::Transport {
                reason: format!("no route for {url}"),
            })
    }
}

fn manifest_body(sha256: &str) -> String {
    serde_json::json!({
        "version": "0.1.0-alpha.4",
        "url": ARCHIVE_URL,
        "notes": "Notes.",
        "min_os": 14,
        "sha256": sha256,
    })
    .to_string()
}

#[test]
fn a_verified_download_is_written_to_the_callers_path() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dest = temp.path().join("nested").join("Perch.zip");
    let transport = Fake::new(&[(ARCHIVE_URL, "hello")]);
    let manifest = Manifest::parse(&manifest_body(HELLO_DIGEST)).expect("parses");

    stage(&transport, &manifest, &dest).expect("stages");

    assert_eq!(fs::read(&dest).expect("written"), b"hello");
}

#[test]
fn a_download_that_does_not_match_the_manifest_is_deleted_and_named() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dest = temp.path().join("Perch.zip");
    let transport = Fake::new(&[(ARCHIVE_URL, "perch")]);
    let manifest = Manifest::parse(&manifest_body(HELLO_DIGEST)).expect("parses");

    let error = stage(&transport, &manifest, &dest).expect_err("perch is not hello");

    assert!(!dest.exists(), "a mismatch must not stay on disk");
    match error {
        Error::Checksum {
            expected, actual, ..
        } => {
            assert_eq!(expected, HELLO_DIGEST);
            assert_ne!(actual, HELLO_DIGEST);
        }
        other => panic!("expected a checksum error, got {other}"),
    }
}

#[test]
fn a_failed_download_leaves_nothing_behind() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dest = temp.path().join("Perch.zip");
    let transport = Fake::new(&[]);
    let manifest = Manifest::parse(&manifest_body(HELLO_DIGEST)).expect("parses");

    let error = stage(&transport, &manifest, &dest).expect_err("no route");

    assert!(matches!(error, Error::Transport { .. }), "{error}");
    assert!(!dest.exists());
}

#[test]
fn download_follows_the_manifest_to_the_archive_it_names() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dest = temp.path().join("Perch.zip");
    let transport = Fake::new(&[
        (MANIFEST_URL, &manifest_body(HELLO_DIGEST)),
        (ARCHIVE_URL, "hello"),
    ]);

    let manifest = download(&transport, MANIFEST_URL, &dest).expect("downloads");

    assert_eq!(manifest.version.to_string(), "0.1.0-alpha.4");
    assert_eq!(fs::read(&dest).expect("written"), b"hello");
}

#[test]
fn download_writes_nothing_when_the_manifest_is_unreadable() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dest = temp.path().join("Perch.zip");
    let transport = Fake::new(&[(MANIFEST_URL, "{\"version\": \"0.1.0\"}")]);

    let error = download(&transport, MANIFEST_URL, &dest).expect_err("missing fields");

    assert!(
        matches!(error, Error::MissingField { field: "url" }),
        "{error}"
    );
    assert!(!dest.exists());
}

#[test]
fn the_request_timeout_is_thirty_seconds() {
    assert_eq!(TIMEOUT, Duration::from_secs(30));
    assert!(
        Http::new().is_ok(),
        "the client has to build to be used at all"
    );
}
