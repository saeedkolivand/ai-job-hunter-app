use super::*;

#[test]
fn test_client_new() {
    let client = LinkedInHttpClient::new(None).expect("build LinkedIn client");
    assert!(!client.has_session());
}

#[test]
fn test_client_with_session() {
    let session = LinkedInSessionData {
        cookies: vec![],
        li_at: "test_token".to_string(),
        jsession_id: Some("jsession".to_string()),
        csrf_token: Some("csrf".to_string()),
        last_updated: 0,
    };
    let client = LinkedInHttpClient::new(Some(session)).expect("build LinkedIn client");
    assert!(client.has_session());
}

#[test]
fn test_update_session() {
    let mut client = LinkedInHttpClient::new(None).expect("build LinkedIn client");
    assert!(!client.has_session());

    let session = LinkedInSessionData {
        cookies: vec![],
        li_at: "test_token".to_string(),
        jsession_id: None,
        csrf_token: None,
        last_updated: 0,
    };
    client.update_session(session);
    assert!(client.has_session());
}

#[test]
fn test_get_default_headers() {
    let client = LinkedInHttpClient::new(None).expect("build LinkedIn client");
    let headers = client
        .get_default_headers()
        .expect("headers build for sessionless client");
    assert!(headers.contains_key(reqwest::header::USER_AGENT));
    assert!(headers.contains_key(reqwest::header::ACCEPT));
}

/// `reqwest` is built without the gzip/brotli/deflate features, so it never
/// auto-decompresses and `get_html` decodes gzip (magic `1f 8b`) by hand —
/// nothing else. Advertising `br`/`deflate` let the edge answer with a body the
/// client could not read, failing the board with "response was not valid UTF-8",
/// so the header must never claim more than the decoder supports.
#[test]
fn accept_encoding_advertises_only_what_get_html_can_decode() {
    let client = LinkedInHttpClient::new(None).expect("build LinkedIn client");
    let headers = client
        .get_default_headers()
        .expect("headers build for sessionless client");
    let accept_encoding = headers
        .get(reqwest::header::ACCEPT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .expect("Accept-Encoding is set");
    assert_eq!(accept_encoding, "gzip");
}

#[test]
fn test_get_default_headers_with_session() {
    let session = LinkedInSessionData {
        cookies: vec![],
        li_at: "test_token".to_string(),
        jsession_id: Some("jsession".to_string()),
        csrf_token: Some("csrf".to_string()),
        last_updated: 0,
    };
    let client = LinkedInHttpClient::new(Some(session)).expect("build LinkedIn client");
    let headers = client
        .get_default_headers()
        .expect("headers build with session");
    assert!(headers.contains_key(reqwest::header::COOKIE));
    assert!(headers.contains_key("X-CSRF-Token"));
}
