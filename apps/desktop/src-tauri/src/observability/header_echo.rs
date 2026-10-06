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
    let name = strip_edge(token).to_ascii_lowercase();
    let suffixed = ["-key", "-token", "-secret", "_key", "_token", "_secret"]
        .iter()
        .any(|s| name.ends_with(s))
        || name == "apikey";
    let quoted = token
        .trim_start_matches(['{', '[', '(', ','])
        .starts_with(['"', '\'']);
    suffixed
        || (quoted
            && matches!(
                name.as_str(),
                "key" | "token" | "secret" | "password" | "passwd" | "auth"
            ))
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

    // Bare `Bearer <credential>` with no `Authorization:` before it.
    if name == "bearer" && next.is_some_and(looks_like_secret) {
        out.push_str(&redact_token(token));
        out.push(' ');
        out.push_str(PLACEHOLDER);
        return Some(2);
    }
    None
}
