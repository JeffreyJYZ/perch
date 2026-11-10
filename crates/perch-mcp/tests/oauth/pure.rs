//! The parts of the flow that need no socket: PKCE, the account name and the callback parse.

use perch_mcp::oauth::{self, Callback};

#[test]
fn the_pure_parts_match_the_spec() {
    assert_eq!(
        oauth::pkce_s256("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
    assert_eq!(oauth::account_for("fixture"), "mcp:fixture");
    assert_eq!(
        oauth::parse_callback("GET /callback?code=abc123&state=xyz HTTP/1.1\r\n").unwrap(),
        Callback {
            code: "abc123".to_string(),
            state: "xyz".to_string(),
        }
    );
    // A refusal is an error, not an empty code handed to the token endpoint.
    assert!(
        oauth::parse_callback("GET /callback?error=access_denied&state=xyz HTTP/1.1\r\n").is_err()
    );
    assert!(oauth::parse_callback("GET /callback?code=abc123 HTTP/1.1\r\n").is_err());
}
