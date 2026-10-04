use super::*;

#[test]
fn rejects_loopback_and_localhost() {
    assert!(!is_safe_public_host("localhost"));
    assert!(!is_safe_public_host("LOCALHOST"));
    assert!(!is_safe_public_host("foo.localhost"));
    assert!(!is_safe_public_host("127.0.0.1"));
    assert!(!is_safe_public_host("127.1.2.3"));
    assert!(!is_safe_public_host("::1"));
}

#[test]
fn rejects_dot_local() {
    assert!(!is_safe_public_host("printer.local"));
    assert!(!is_safe_public_host("local"));
}

#[test]
fn rejects_private_ranges() {
    assert!(!is_safe_public_host("10.0.0.5"));
    assert!(!is_safe_public_host("10.255.255.255"));
    assert!(!is_safe_public_host("172.16.0.1"));
    assert!(!is_safe_public_host("172.31.255.255"));
    assert!(!is_safe_public_host("192.168.1.1"));
}

#[test]
fn rejects_link_local_and_cgnat() {
    assert!(!is_safe_public_host("169.254.169.254")); // cloud metadata!
    assert!(!is_safe_public_host("100.64.0.1")); // CGNAT
    assert!(!is_safe_public_host("fe80::1"));
}

#[test]
fn rejects_unspecified_and_ula() {
    assert!(!is_safe_public_host("0.0.0.0"));
    assert!(!is_safe_public_host("::"));
    assert!(!is_safe_public_host("fc00::1"));
    assert!(!is_safe_public_host("fd12:3456::1"));
}

#[test]
fn rejects_ipv4_mapped_private() {
    assert!(!is_safe_public_host("::ffff:127.0.0.1"));
    assert!(!is_safe_public_host("::ffff:10.0.0.1"));
}

#[test]
fn allows_public_hosts() {
    assert!(is_safe_public_host("boards.greenhouse.io"));
    assert!(is_safe_public_host("jobs.lever.co"));
    assert!(is_safe_public_host("1.1.1.1"));
    assert!(is_safe_public_host("8.8.8.8"));
    assert!(is_safe_public_host("2606:4700:4700::1111"));
}

#[test]
fn is_safe_ip_accepts_public_literals() {
    assert!(is_safe_ip("1.1.1.1".parse().unwrap()));
    assert!(is_safe_ip("8.8.8.8".parse().unwrap()));
    assert!(is_safe_ip("2606:4700:4700::1111".parse().unwrap()));
}

// ── validate_provider_base_url: provenance guard, NOT an IP filter ─────────

#[test]
fn base_url_rejects_non_http_schemes() {
    for u in ["ftp://host/v1", "file:///etc/passwd", "ws://host/v1"] {
        assert!(
            validate_provider_base_url(u).is_err(),
            "{u} must be rejected (scheme)"
        );
    }
}

#[test]
fn base_url_rejects_garbage() {
    assert!(validate_provider_base_url("not a url").is_err());
    assert!(validate_provider_base_url("").is_err());
}

#[test]
fn base_url_blocks_cloud_metadata() {
    assert!(validate_provider_base_url("http://169.254.169.254/").is_err());
    assert!(
        validate_provider_base_url("http://169.254.169.254/latest/meta-data/").is_err(),
        "the metadata host must be blocked regardless of path"
    );
}

#[test]
fn base_url_allows_local_gateways_and_public() {
    // The load-bearing exception: loopback/LAN gateways are legitimate AI
    // endpoints and must NOT be filtered — only the metadata addr + bad schemes.
    for u in [
        "http://127.0.0.1:11434",       // Ollama
        "http://localhost:1234/v1",     // LM Studio
        "http://192.168.1.50:8000/v1",  // on-prem LAN vLLM
        "https://openrouter.ai/api/v1", // public gateway
        "https://api.openai.com/v1",
    ] {
        assert!(
            validate_provider_base_url(u).is_ok(),
            "{u} must be allowed (provenance guard, not IP filter)"
        );
    }
}
