//! Header-style and pretty-JSON credential echoes for
//! [`redact_tokens`](super::redact_tokens): `x-goog-api-key: <v>`,
//! `x-api-key: <v>`, any `*-key:` / `*-token:` name, `authorization: <scheme> <v>`,
//! a bare `Bearer <v>`, and the same names as JSON fields with a space after
//! the colon (`{"x-goog-api-key": "AIza…"}`, `{"Authorization": "Bearer gsk_…"}`).
//!
//! Provider error bodies echo the request, so these are the shapes a leaked key
//! actually arrives in. Whitespace-token granularity like the rest of the
//! redactor: one pass, constant work per token.

use super::redact_token;

const PLACEHOLDER: &str = "<credential-redacted>";

/// Auth-scheme words that sit between `authorization:` and the credential.
fn is_scheme(token: &str) -> bool {
    matches!(
        strip_edge(token).to_ascii_lowercase().as_str(),
        "bearer" | "basic" | "token" | "key"
    )
}

/// `token` without surrounding JSON/prose punctuation.
fn strip_edge(token: &str) -> &str {
    token.trim_matches(|c: char| {
        matches!(
            c,
            '"' | '\'' | '`' | '{' | '}' | '[' | ']' | '(' | ')' | ',' | ';' | ':'
        )
    })
}

/// Whether `token` (the `name:` half of `name: value`) names a credential field.
/// Plain prose words (`token:`, `key:`) only count when JSON-quoted — "invalid
/// token: expired" must survive; `"token": "…"` cannot be prose.
fn is_credential_name(token: &str) -> bool {
    if !token.ends_with(':') {
        return false;
    }
    let quoted = token
        .trim_start_matches(['{', '[', '(', ','])
        .starts_with(['"', '\'']);
    is_key_name(&strip_edge(token).to_ascii_lowercase(), quoted)
}

/// `name` (lowercase) is a credential field name; `quoted` = it was JSON-quoted.
fn is_key_name(name: &str, quoted: bool) -> bool {
    let suffixed = ["-key", "-token", "-secret", "_key", "_token", "_secret"]
        .iter()
        .any(|s| name.ends_with(s))
        || name == "apikey";
    suffixed
        || (quoted
            && matches!(
                name,
                "key" | "token" | "secret" | "password" | "passwd" | "auth"
            ))
}

/// Compact forms where the name and its value share one whitespace token:
/// `{"Authorization":"Bearer`, `x-api-key:K`, `"api_key":"K"`, `Authorization=Bearer`.
/// Scans each `:`/`=` once, reading the name run behind it; URLs are left to
/// `redact_token`'s own `<url-redacted>`.
fn redact_embedded(tokens: &[&str], i: usize, out: &mut String) -> Option<usize> {
    let token = tokens[i];
    if token.contains("://") {
        return None;
    }
    let b = token.as_bytes();
    for (p, &c) in b.iter().enumerate() {
        if c != b':' && c != b'=' {
            continue;
        }
        let mut e = p;
        if e > 0 && matches!(b[e - 1], b'"' | b'\'') {
            e -= 1;
        }
        let mut s = e;
        while s > 0 && (b[s - 1].is_ascii_alphanumeric() || matches!(b[s - 1], b'_' | b'-')) {
            s -= 1;
        }
        if s == e {
            continue;
        }
        let name = token[s..e].to_ascii_lowercase();
        let quoted = s > 0 && matches!(b[s - 1], b'"' | b'\'');
        let auth = name.ends_with("authorization");
        if !auth && !is_key_name(&name, quoted) {
            continue;
        }
        let vstart = p
            + 1
            + b[p + 1..]
                .iter()
                .take_while(|c| matches!(c, b'"' | b'\'' | b'{' | b'['))
                .count();
        let vend = vstart
            + b[vstart..]
                .iter()
                .rposition(|c| !matches!(c, b'"' | b'\'' | b'}' | b']' | b',' | b';'))
                .map_or(0, |n| n + 1);
        if vstart >= vend {
            continue;
        }
        let value = &token[vstart..vend];
        if is_scheme(value) {
            tokens.get(i + 1)?;
            out.push_str(token);
            out.push(' ');
            out.push_str(PLACEHOLDER);
            return Some(2);
        }
        if auth && (e == p || value.len() < 8) {
            continue;
        }
        out.push_str(&token[..vstart]);
        out.push_str(PLACEHOLDER);
        out.push_str(&token[vend..]);
        return Some(1);
    }
    None
}

/// A bare credential-shaped word after `Bearer`: long, token charset, and not a
/// plain lowercase/Capitalised English word ("Bearer authentication failed").
fn looks_like_secret(token: &str) -> bool {
    let v = strip_edge(token);
    v.len() >= 16
        && v.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~' | '+' | '/' | '=')
        })
        && (v
            .chars()
            .any(|c| c.is_ascii_digit() || matches!(c, '-' | '_' | '.'))
            || v.chars().skip(1).any(|c| c.is_ascii_uppercase()))
}

/// If `tokens[i]` starts a header/JSON credential echo, append its redacted form
/// to `out` and return how many tokens it consumed; `None` leaves `i` to the
/// caller's other rules.
pub(super) fn redact_header_echo(tokens: &[&str], i: usize, out: &mut String) -> Option<usize> {
    let token = tokens[i];
    let next = tokens.get(i + 1).copied();
    let name = strip_edge(token).to_ascii_lowercase();

    // `authorization: <scheme> <value>` — gated on the scheme word so an
    // unrelated "Authorization failed" survives.
    if name.ends_with("authorization") {
        let (scheme, _) = next.zip(tokens.get(i + 2))?;
        if !is_scheme(scheme) {
            return None;
        }
        out.push_str(&redact_token(token));
        out.push(' ');
        out.push_str(&redact_token(scheme));
        out.push(' ');
        out.push_str(PLACEHOLDER);
        return Some(3);
    }

    if is_credential_name(token) {
        let value = next?;
        out.push_str(&redact_token(token));
        out.push(' ');
        // `x-api-key: Bearer <v>`: the scheme word is part of the echo.
        if is_scheme(value) && tokens.get(i + 2).is_some() {
            out.push_str(&redact_token(value));
            out.push(' ');
            out.push_str(PLACEHOLDER);
            return Some(3);
        }
        out.push_str(PLACEHOLDER);
        return Some(2);
    }

    if let Some(used) = redact_embedded(tokens, i, out) {
        return Some(used);
    }

    // Bare `Bearer <credential>` with no `Authorization:` before it.
    if name == "bearer" && next.is_some_and(looks_like_secret) {
        out.push_str(&redact_token(token));
        out.push(' ');
        out.push_str(PLACEHOLDER);
        return Some(2);
    }
    None
}
