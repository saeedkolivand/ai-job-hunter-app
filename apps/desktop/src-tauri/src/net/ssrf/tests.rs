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
fn base_url_blocks_link_local_and_metadata_in_every_form() {
    for u in [
        "http://169.254.170.2/v2/credentials",
        "http://169.254.0.1/",
        "http://169.254.169.254:80/",
        "http://2852039166/",          // decimal 169.254.169.254
        "http://0xa9fea9fe/",          // hex
        "http://0251.0376.0251.0376/", // octal
        "http://[fe80::1]/",
        "http://[febf::1]/",
        "http://[fd00:ec2::254]/",
        "http://[FD00:EC2:0:0:0:0:0:254]/",
        "http://[::ffff:169.254.169.254]/",
        "http://[::ffff:a9fe:a9fe]/",
        "http://[::169.254.169.254]/",
        "http://[::ffff:169.254.170.2]/",
        "http://100.100.100.200/",
        "http://[::ffff:100.100.100.200]/",
        "http://[2002:a9fe:a9fe::]/",
        "http://[64:ff9b::a9fe:a9fe]/",
        "http://[64:ff9b:1::a9fe:a9fe]/",
        "http://[::ffff:0:a9fe:a9fe]/",
    ] {
        assert!(
            validate_provider_base_url(u).is_err(),
            "{u} must be rejected"
        );
    }
}

#[test]
fn base_url_mapped_private_and_other_ula_stay_allowed() {
    // Local gateways: mapped LAN/loopback and non-IMDS ULA are not link-local.
    for u in [
        "http://[::ffff:10.0.0.1]:8080/",
        "http://[::ffff:127.0.0.1]:11434/",
        "http://[fd12:3456::1]/",
        "http://[fd00:ec2::255]/",
        "http://[::1]:11434/",
    ] {
        assert!(validate_provider_base_url(u).is_ok(), "{u} must be allowed");
    }
}

#[test]
fn ipv4_compatible_private_is_unsafe() {
    assert!(!is_safe_public_host("::10.0.0.1"));
    assert!(!is_safe_public_host("::127.0.0.1"));
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

#[test]
fn trailing_dot_and_alias_loopback_names_are_not_public() {
    for h in [
        "localhost.",
        "localhost..",
        "x.localhost..",
        "127.0.0.1..",
        "LOCALHOST.",
        "foo.localhost.",
        "x.local.",
        "localhost.localdomain",
        "ip6-localhost",
        "ip6-loopback.",
    ] {
        assert!(!is_safe_public_host(h), "{h}");
    }
    assert!(is_safe_public_host("example.com."));
}

#[test]
fn wrapped_private_ipv4_is_unsafe_but_dns64_public_is_safe() {
    for h in [
        "2002:7f00:1::",       // 6to4 127.0.0.1
        "64:ff9b::a00:1",      // NAT64 10.0.0.1
        "64:ff9b:1::c0a8:101", // local-use NAT64 192.168.1.1
        "::ffff:0:a00:1",      // SIIT 10.0.0.1
    ] {
        assert!(!is_safe_public_host(h), "{h}");
    }
    assert!(is_safe_public_host("64:ff9b::808:808"));
    assert!(is_safe_public_host("2002:808:808::"));
}
