use super::*;

#[test]
fn test_client_new() {
    let client = LinkedInHttpClient::new(None);
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
    let client = LinkedInHttpClient::new(Some(session));
    assert!(client.has_session());
}

#[test]
fn test_update_session() {
    let mut client = LinkedInHttpClient::new(None);
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
    let client = LinkedInHttpClient::new(None);
    let headers = client
        .get_default_headers()
        .expect("headers build for sessionless client");
    assert!(headers.contains_key(reqwest::header::USER_AGENT));
    assert!(headers.contains_key(reqwest::header::ACCEPT));
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
    let client = LinkedInHttpClient::new(Some(session));
    let headers = client
        .get_default_headers()
        .expect("headers build with session");
    assert!(headers.contains_key(reqwest::header::COOKIE));
    assert!(headers.contains_key("X-CSRF-Token"));
}
