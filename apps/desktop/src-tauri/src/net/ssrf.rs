//! SSRF address classifier for the centralized HTTP layer.
//!
//! Pure IP/host predicates used to reject loopback, RFC-1918, link-local
//! (incl. cloud-metadata `169.254.169.254`), CGNAT, ULA, and `*.local` /
//! `localhost` targets. Lives in `net` (L0) so both the extension-bridge host
//! guard (L3) and the IP-pinned guarded fetch ([`crate::net::http::get_guarded`])
//! can share one classifier without an upward dependency.
//!
//! Also home to [`validate_provider_base_url`] — the AI-provider egress guard,
//! which is deliberately NOT a wholesale IP filter (local gateways are legit).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::error::{AppError, AppResult};

/// Lowercase, strip brackets and ONE trailing dot: `url` keeps the root dot on
/// domains, so `localhost.` must classify like `localhost`.
fn normalize_host(host: &str) -> String {
    let h = host.trim().trim_matches(|c| c == '[' || c == ']');
    h.strip_suffix('.').unwrap_or(h).to_ascii_lowercase()
}

/// IPv4 address embedded in an IPv6 one: mapped/compatible, SIIT
/// (`::ffff:0:a.b.c.d`), 6to4 (`2002::/16`), NAT64 (`64:ff9b::/96`,
/// `64:ff9b:1::/48`). Only the embedded address decides, so DNS64 to public
/// IPv4 stays reachable.
fn embedded_ipv4(v6: Ipv6Addr) -> Option<Ipv4Addr> {
    let s = v6.segments();
    let v4 = |hi: u16, lo: u16| Ipv4Addr::from(((u32::from(hi)) << 16) | u32::from(lo));
    if let Some(v) = v6.to_ipv4() {
        Some(v)
    } else if s[..4] == [0; 4] && s[4] == 0xffff && s[5] == 0 {
        Some(v4(s[6], s[7]))
    } else if s[0] == 0x2002 {
        Some(v4(s[1], s[2]))
    } else if s[0] == 0x64 && s[1] == 0xff9b && (s[2..6] == [0; 4] || s[2] == 1) {
        Some(v4(s[6], s[7]))
    } else {
        None
    }
}

/// The host-only SSRF guard. `host` is a bare hostname or IP literal (no
/// scheme/port). Rejects: `localhost`, any `*.localhost`, `local`, any
/// `*.local`, loopback (`127.0.0.0/8`, `::1`), RFC-1918 private (`10/8`,
/// `172.16/12`, `192.168/16`), link-local (`169.254/16`, `fe80::/10`), CGNAT
/// (`100.64.0.0/10`), ULA (`fc00::/7`), unspecified (`0.0.0.0`, `::`),
/// broadcast/multicast, and IPv4-mapped-private. A non-IP, non-blocked
/// hostname is treated as public.
pub fn is_safe_public_host(host: &str) -> bool {
    let lower = normalize_host(host);
    let host = lower.as_str();
    if host.is_empty() {
        return false;
    }

    // Hostname blocklist.
    if matches!(
        host,
        "localhost" | "localhost.localdomain" | "ip6-localhost" | "ip6-loopback"
    ) || host.ends_with(".localhost")
    {
        return false;
    }
    if host == "local" || host.ends_with(".local") {
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
    // Wrapped IPv4 (mapped/compatible/SIIT/6to4/NAT64): defer to the IPv4 rules
    // so a wrapped loopback/private address can't slip through.
    if let Some(v4) = embedded_ipv4(ip) {
        return is_safe_ipv4(v4);
    }
    true
}

/// AWS IMDS over IPv6.
const AWS_IMDS_IPV6: Ipv6Addr = Ipv6Addr::new(0xfd00, 0x0ec2, 0, 0, 0, 0, 0, 0x0254);

/// Alibaba Cloud metadata service.
const ALIBABA_METADATA_IPV4: Ipv4Addr = Ipv4Addr::new(100, 100, 100, 200);

fn is_metadata_ipv4(v4: Ipv4Addr) -> bool {
    v4.is_link_local() || v4 == ALIBABA_METADATA_IPV4
}

/// Link-local / cloud-metadata addresses: all of `169.254.0.0/16` (incl. the
/// ECS task-metadata `169.254.170.2`), `fe80::/10`, `fd00:ec2::254` and
/// Alibaba's `100.100.100.200`. IPv4-mapped (`::ffff:a.b.c.d`) and
/// IPv4-compatible forms are unwrapped and re-checked. Unlike [`is_safe_ip`]
/// this does NOT cover loopback/LAN/ULA; local gateways legitimately live there.
pub fn is_link_local_or_metadata_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_metadata_ipv4(v4),
        IpAddr::V6(v6) => {
            embedded_ipv4(v6).is_some_and(is_metadata_ipv4)
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                || v6 == AWS_IMDS_IPV6
        }
    }
}

/// [`is_link_local_or_metadata_ip`] for a bare host string (brackets allowed);
/// non-IP hostnames are `false`.
pub fn is_link_local_or_metadata_host(host: &str) -> bool {
    normalize_host(host)
        .parse::<IpAddr>()
        .is_ok_and(is_link_local_or_metadata_ip)
}

/// Whether a host is an unspecified address (`0.0.0.0`, `::`, or a mapped /
/// compatible form); these connect to the local machine on most stacks.
pub fn is_unspecified_host(host: &str) -> bool {
    match host
        .trim_matches(|c| c == '[' || c == ']')
        .parse::<IpAddr>()
    {
        Ok(IpAddr::V4(v4)) => v4.is_unspecified(),
        Ok(IpAddr::V6(v6)) => {
            v6.is_unspecified() || embedded_ipv4(v6).is_some_and(|v4| v4.is_unspecified())
        }
        Err(_) => false,
    }
}

/// Validate a user-supplied AI-provider base URL for the *generation egress*
/// path (the endpoint that receives the stored API key + full prompt).
///
/// Unlike [`is_safe_public_host`], this deliberately does **not** reject
/// loopback / LAN addresses: local gateways — Ollama (`http://127.0.0.1:11434`),
/// LM Studio, an on-prem vLLM — are legitimate OpenAI-compatible endpoints. It is
/// a *provenance* guard against the key-exfiltration SSRF (an XSS'd renderer
/// pointing the base URL at an attacker), not a wholesale IP filter. It blocks
/// only:
///  1. a non-`http(s)` scheme, and
///  2. link-local / cloud-metadata hosts ([`is_link_local_or_metadata_ip`]) in
///     any notation (`url` normalizes hex/octal/decimal IPv4 to dotted-decimal;
///     IPv6 and IPv4-mapped forms are unwrapped). Loopback, LAN and other ULA
///     stay allowed. Defense-in-depth; the real boundary is provenance (see
///     `crate::ai_config`).
///
/// Uses the same `reqwest::Url` parser as [`crate::net::http::get_guarded`] —
/// never hand-rolled URL parsing.
pub fn validate_provider_base_url(base_url: &str) -> AppResult<()> {
    let url = reqwest::Url::parse(base_url)
        .map_err(|e| AppError::Validation(format!("Invalid provider base URL: {e}")))?;
    match url.scheme() {
        "http" | "https" => {}
        other => {
            return Err(AppError::Validation(format!(
                "Provider base URL scheme '{other}' is not allowed; use http or https."
            )))
        }
    }
    match url.host_str() {
        Some(host) => {
            let blocked = is_link_local_or_metadata_host(host);
            if blocked {
                return Err(AppError::Validation(
                    "Provider base URL targets a link-local/cloud-metadata address and is not allowed."
                        .to_string(),
                ));
            }
        }
        None => {
            return Err(AppError::Validation(
                "Provider base URL has no host.".to_string(),
            ))
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
