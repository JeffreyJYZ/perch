//! The provider's model listing: whatever it returns, sorted, and nothing else.
//!
//! Every test here answers the one GET from a table, so no socket is opened and no provider is
//! required. The real client is the same code path with [`perch_agent::HttpTransport`].

use std::sync::Mutex;

use perch_agent::{Error, ModelTransport, list_models_with};

/// What the table answers.
enum Answer {
    /// A 200 with this body.
    Body(&'static str),
    /// A refusal with this status.
    Status(u16),
    /// No route at all, as a transport whose host cannot be reached.
    Unreachable,
}

/// A transport that records every URL it was asked for and answers the one thing it was given.
struct Table {
    answer: Answer,
    urls: Mutex<Vec<String>>,
}

impl Table {
    fn answering(answer: Answer) -> Table {
        Table {
            answer,
            urls: Mutex::new(Vec::new()),
        }
    }

    fn url(&self, index: usize) -> String {
        let urls = self.urls.lock().unwrap();
        urls.get(index)
            .unwrap_or_else(|| panic!("no request number {index}: {urls:?}"))
            .clone()
    }
}

impl ModelTransport for Table {
    fn get(&self, url: &str, _api_key: &str) -> perch_agent::Result<Vec<u8>> {
        self.urls.lock().unwrap().push(url.to_string());
        match self.answer {
            Answer::Body(body) => Ok(body.as_bytes().to_vec()),
            Answer::Status(status) => Err(Error::Status {
                status,
                body: "no such endpoint".to_string(),
            }),
            Answer::Unreachable => Err(Error::Config("no route".to_string())),
        }
    }
}

#[test]
fn the_list_is_the_providers_sorted_and_free_models_are_not_privileged() {
    let table = Table::answering(Answer::Body(
        r#"{"data":[
            {"id":"vendor/zeta"},
            {"id":"free/alpha:free"},
            {"id":"vendor/zeta"},
            {"id":"free/beta:free"},
            {"id":"  vendor/trimmed  "},
            {"id":""}
        ]}"#,
    ));

    let ids = list_models_with(&table, "https://provider.test/v1", "sk-test").unwrap();

    assert_eq!(
        ids,
        vec![
            "free/alpha:free".to_string(),
            "free/beta:free".to_string(),
            "vendor/trimmed".to_string(),
            "vendor/zeta".to_string(),
        ],
        "sorted, deduplicated, trimmed, and a free id sits where its name sorts"
    );
}

#[test]
fn the_listing_is_the_base_url_plus_models() {
    for (base, expected) in [
        (
            "https://provider.test/v1",
            "https://provider.test/v1/models",
        ),
        (
            "https://provider.test/v1/",
            "https://provider.test/v1/models",
        ),
        (
            "https://provider.test/v1/chat/completions",
            "https://provider.test/v1/models",
        ),
    ] {
        let table = Table::answering(Answer::Body(r#"{"data":[{"id":"a/b"}]}"#));
        list_models_with(&table, base, "").unwrap();
        assert_eq!(table.url(0), expected, "the listing for {base}");
    }
}

#[test]
fn a_provider_that_names_models_instead_of_data_is_still_read() {
    let table = Table::answering(Answer::Body(
        r#"{"models":[{"name":"llama-local"},{"name":"qwen-local"}]}"#,
    ));

    let ids = list_models_with(&table, "http://127.0.0.1:11434/v1", "").unwrap();

    assert_eq!(
        ids,
        vec!["llama-local".to_string(), "qwen-local".to_string()]
    );
}

#[test]
fn a_provider_with_no_listing_is_an_error_in_words() {
    let refused = Table::answering(Answer::Status(404));
    let error = list_models_with(&refused, "https://provider.test/v1", "")
        .expect_err("a 404 is not a model list");
    assert!(
        error.to_string().contains("404"),
        "the refusal names the status: {error}"
    );

    let unreachable = Table::answering(Answer::Unreachable);
    assert!(
        list_models_with(&unreachable, "https://provider.test/v1", "").is_err(),
        "a transport with no route is an error, not an empty list"
    );

    let garbled = Table::answering(Answer::Body("this is not JSON"));
    assert!(
        list_models_with(&garbled, "https://provider.test/v1", "").is_err(),
        "a body that does not parse is an error, not an empty list"
    );
}

#[test]
fn an_address_that_is_not_http_is_refused_before_any_request() {
    for bad in ["", "not a url", "provider.test/v1", "ftp://provider.test"] {
        let table = Table::answering(Answer::Body(r#"{"data":[]}"#));
        assert!(
            list_models_with(&table, bad, "").is_err(),
            "`{bad}` is not a provider address"
        );
        assert!(
            table.urls.lock().unwrap().is_empty(),
            "`{bad}` reached the transport anyway"
        );
    }
}
