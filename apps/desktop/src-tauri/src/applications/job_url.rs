//! Job-URL identity: the normalizer that turns a posting link into the stable
//! dedup key an [`super::Application`] is keyed by, plus its scheme guard.
//!
//! Split out of [`super`] verbatim (no behaviour change) to keep the store body
//! under the architecture LOC cap (`tests/architecture.rs` R8).
//! [`normalize_job_url`] is re-exported from `super`, so every existing caller
//! (`crate::applications::normalize_job_url`) is unaffected.

/// Extract an explicit URL scheme (the `scheme:` prefix per RFC 3986§3.1) if
/// one is present, lowercased. A scheme is `ALPHA *( ALPHA / DIGIT / "+" / "-" /
/// "." )` immediately followed by `:`, and it MUST appear before any `/`, `?`, or
/// `#` — so `javascript:alert(1)` and `data:text/html,…` are schemes, but a
/// scheme-less `host/path?x=a:b` (colon in the path/query) is not. Used to reject
/// dangerous schemes; returns `None` for scheme-less input.
fn explicit_scheme(input: &str) -> Option<String> {
    // Only the authority-less head, before the first path/query/fragment delimiter,
    // can carry a scheme. This keeps a `:` inside a path or query from looking like one.
    let head = input.split(['/', '?', '#']).next().unwrap_or(input);
    let (candidate, _) = head.split_once(':')?;
    if candidate.is_empty() {
        return None;
    }
    let mut chars = candidate.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        return None;
    }
    Some(candidate.to_ascii_lowercase())
}

/// Normalize a job URL into a stable dedup key: lowercase host, strip a leading
/// `www.`, drop the fragment (`#…`), retain only per-host *identifying* query
/// params (e.g. Indeed `jk`) while dropping every other query param (utm_*, ref,
/// tracking), and trim a trailing `/`. The scheme is preserved (lowercased). Empty
/// input returns empty.
///
/// Security chokepoint: an input carrying an explicit scheme other than
/// `http`/`https` (e.g. `javascript:`, `data:`, `file:`, `vbscript:`, `blob:`) is
/// neutralized to an empty string — i.e. "no url" — so an import-borne or
/// manually-entered payload can never be stored as an openable link. Scheme-less
/// input and `http(s)` keep their exact prior normalization.
///
/// No existing centralized URL normalizer was found in `net`/`scraping` (only
/// host-only helpers like `contact_profile::host_of`), so this is the single
/// owner for Application url identity.
pub fn normalize_job_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // Reject dangerous explicit schemes at the single backend chokepoint. Only
    // `http`/`https` may round-trip; any other explicit scheme yields "no url".
    if let Some(scheme) = explicit_scheme(trimmed) {
        if scheme != "http" && scheme != "https" {
            return String::new();
        }
    }
    let lower = trimmed.to_lowercase();
    let (scheme, rest) = match lower.split_once("://") {
        Some((s, r)) => (Some(s.to_string()), r.to_string()),
        None => (None, lower.clone()),
    };
    // Drop the fragment (`#…`) unconditionally, then split the query off the path so
    // per-host identifying params can be selectively retained below.
    let no_frag = rest.split('#').next().unwrap_or(&rest);
    let (path_part, query) = match no_frag.split_once('?') {
        Some((p, q)) => (p, q),
        None => (no_frag, ""),
    };
    let (host, path) = match path_part.split_once('/') {
        Some((h, p)) => (h.to_string(), Some(p.to_string())),
        None => (path_part.to_string(), None),
    };
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    // Keep ONLY the host's identifying query params (utm_*, ref, … are dropped);
    // hosts with no allowlist entry drop the whole query, exactly as before.
    let retained_query = retain_identifying_params(&host, query);
    let mut out = String::new();
    if let Some(s) = scheme {
        out.push_str(&s);
        out.push_str("://");
    }
    out.push_str(&host);
    if let Some(p) = path {
        let p = p.trim_end_matches('/');
        if !p.is_empty() {
            out.push('/');
            out.push_str(p);
        }
    }
    if !retained_query.is_empty() {
        out.push('?');
        out.push_str(&retained_query);
    }
    out
}

/// Per-host allowlist of *identifying* query params that must survive normalization
/// (every other query param — utm_*, ref, tracking — is dropped, and hosts absent
/// here drop the entire query, so a host whose id lives in the QUERY collapses to
/// one key for every job). Keep in sync with every canonical URL builder that does
/// that: Indeed (`/viewjob?jk=<id>`) and Hacker News (`/item?id=<id>`, built by
/// `boards::ycombinator` when a job story has no external url). Path-based ids
/// (LinkedIn et al.) need no entry.
fn identifying_query_params(host: &str) -> &'static [&'static str] {
    if host == "indeed.com" || host.ends_with(".indeed.com") {
        &["jk"]
    } else if host == "news.ycombinator.com" {
        &["id"]
    } else {
        &[]
    }
}

/// Rebuild the query string keeping only `identifying_query_params(host)`, emitted
/// in the allowlist's own fixed order so the input param ordering can never change
/// the dedup key. A param with an empty value is skipped. Returns "" when nothing is
/// retained (the common, path-based case).
fn retain_identifying_params(host: &str, query: &str) -> String {
    let allow = identifying_query_params(host);
    if allow.is_empty() || query.is_empty() {
        return String::new();
    }
    allow
        .iter()
        .filter_map(|key| {
            query.split('&').find_map(|pair| {
                let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
                (k == *key && !v.is_empty()).then(|| format!("{key}={v}"))
            })
        })
        .collect::<Vec<_>>()
        .join("&")
}
