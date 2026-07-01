//! SSRF address classifier for the centralized HTTP layer.
//!
//! Pure IP/host predicates used to reject loopback, RFC-1918, link-local
//! (incl. cloud-metadata `169.254.169.254`), CGNAT, ULA, and `*.local` /
//! `localhost` targets. Lives in `net` (L0) so both the extension-bridge host
//! guard (L3) and the IP-pinned guarded fetch ([`crate::net::http::get_guarded`])
//! can share one classifier without an upward dependency.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// The host-only SSRF guard. `host` is a bare hostname or IP literal (no
/// scheme/port). Rejects: `localhost`, any `*.localhost`, `local`, any
/// `*.local`, loopback (`127.0.0.0/8`, `::1`), RFC-1918 private (`10/8`,
/// `172.16/12`, `192.168/16`), link-local (`169.254/16`, `fe80::/10`), CGNAT
/// (`100.64.0.0/10`), ULA (`fc00::/7`), unspecified (`0.0.0.0`, `::`),
/// broadcast/multicast, and IPv4-mapped-private. A non-IP, non-blocked
/// hostname is treated as public.
pub fn is_safe_public_host(host: &str) -> bool {
    let host = host.trim().trim_matches(|c| c == '[' || c == ']');
    if host.is_empty() {
        return false;
    }
    let lower = host.to_ascii_lowercase();

    // Hostname blocklist (case-insensitive).
    if lower == "localhost" || lower.ends_with(".localhost") {
        return false;
    }
    if lower == "local" || lower.ends_with(".local") {
        return false;
    }

    // IP-literal checks.
    if let Ok(ip) = host.parse::<IpAddr>() {
        return is_safe_ip(ip);
    }

    // A non-IP, non-blocked hostname is treated as public.
    true
}

/// Whether an IP literal is a safe public address (not loopback/private/
/// link-local/unspecified/multicast/broadcast).
pub fn is_safe_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_safe_ipv4(v4),
        IpAddr::V6(v6) => is_safe_ipv6(v6),
    }
}

pub fn is_safe_ipv4(ip: Ipv4Addr) -> bool {
    if ip.is_loopback()        // 127.0.0.0/8
        || ip.is_private()     // 10/8, 172.16/12, 192.168/16
        || ip.is_link_local()  // 169.254.0.0/16
        || ip.is_unspecified() // 0.0.0.0
        || ip.is_broadcast()   // 255.255.255.255
        || ip.is_multicast()
    {
        return false;
    }
    // Carrier-grade NAT 100.64.0.0/10 — also internal; reject.
    let [a, b, ..] = ip.octets();
    if a == 100 && (64..=127).contains(&b) {
        return false;
    }
    true
}

pub fn is_safe_ipv6(ip: Ipv6Addr) -> bool {
    if ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() {
        return false;
    }
    let seg0 = ip.segments()[0];
    // Link-local fe80::/10.
    if (seg0 & 0xffc0) == 0xfe80 {
        return false;
    }
    // Unique-local fc00::/7 (private).
    if (seg0 & 0xfe00) == 0xfc00 {
        return false;
    }
    // IPv4-mapped (::ffff:a.b.c.d) — defer to the IPv4 rules so a mapped
    // loopback/private address can't slip through.
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_safe_ipv4(v4);
    }
    true
}

#[cfg(test)]
mod tests {
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
}
