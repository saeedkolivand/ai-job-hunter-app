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

/// `read_bytes_capped` (called before `decode_body`) bounds only the
/// COMPRESSED wire size — DEFLATE can expand up to ~1032:1, so a small
/// compressed payload can still decompress past the cap. `decode_body` must
/// catch that on the DECOMPRESSED side too (`Read::take(cap)`), not just
/// trust the compressed-size bound already enforced upstream.
#[test]
fn decode_body_rejects_a_gzip_payload_that_decompresses_past_the_cap() {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    // 500 zero bytes compress to well under 100 bytes on the wire (DEFLATE
    // eats long runs of the same byte almost for free) — a pass here can only
    // happen if the DECOMPRESSED side is bounded, since the compressed size
    // is nowhere near the 100-byte cap under test.
    encoder.write_all(&[0u8; 500]).unwrap();
    let gzipped = encoder.finish().unwrap();
    assert!(
        gzipped.len() < 100,
        "test setup: the compressed payload must stay under the cap on its own"
    );

    let err = decode_body(gzipped, 100)
        .expect_err("a body that decompresses past the cap must be rejected");
    assert!(
        matches!(err, AppError::Validation(_)),
        "expected a size Validation error, got: {err:?}"
    );
}
