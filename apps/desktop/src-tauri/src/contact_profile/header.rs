//! The document header contact line: the single builder shared by the résumé,
//! cover letter and DOCX paths, its validation-side twin [`ContactProfile::header_urls`],
//! and the per-value sanitizers both rely on.

use std::sync::LazyLock;

use regex::Regex;

use super::{non_empty, ContactProfile};
use crate::model::rich::{tokenize_rich, RichText};

impl ContactProfile {
    /// Build the header contact line as a markdown string, localized for `lang`,
    /// in the canonical order **location | email | phone | LinkedIn | GitHub |
    /// Website | extras**. Links are emitted as `[Label](url)` and the email bare
    /// (the renderers turn it into a `mailto:` link); the existing
    /// [`tokenize_rich`] / `split_urls` machinery makes every part clickable.
    ///
    /// This is the single header builder shared by the résumé, cover letter, and
    /// DOCX paths — there is no other place a header URL is chosen. Every part
    /// is scheme-checked (link fields only; `javascript:`/`data:` never render as
    /// a clickable header link) and sanitized (control characters incl. `\n`
    /// stripped, length capped) before joining — this string is spliced verbatim
    /// into plain, `\n`-split document text (H's header-seeding path), so an
    /// embedded newline would otherwise inject an arbitrary extra line.
    ///
    /// Sanitization/capping runs on each BARE value (url/label/text) BEFORE a
    /// link field is formatted into `[Label](url)`, not on the formatted
    /// string afterward — capping the formatted string instead can truncate
    /// away the closing `)` for a long-but-legitimate URL, producing a
    /// malformed link that [`Self::header_urls`] (bare-URL-only) would never
    /// reproduce, so the genuinely-rendered link fails set membership there
    /// and false-fires `header_url_mismatch`. Capping the bare URL first, the
    /// same way in both methods, is what keeps them recording the identical
    /// post-cap string by construction. Every label that ends up INSIDE a
    /// `[Label](url)` construct goes through [`sanitize_link_label`] (drops
    /// `[`, `]`, `(`, `)` — safe for display text); every URL goes through
    /// [`sanitize_link_url`] instead (drops `[`/`]` but PERCENT-ENCODES
    /// `(`/`)` — deleting a paren from a URL corrupts it into a different
    /// destination, unlike a label). Neither is plain [`sanitize_header_part`]
    /// — location/email/phone stay on that since they're never
    /// bracket-wrapped.
    pub fn header_markdown(&self, lang: &str) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(loc) = &self.location {
            let v = loc.resolve(lang);
            if !v.trim().is_empty() {
                parts.push(sanitize_header_part(v));
            }
        }
        if let Some(email) = non_empty(&self.email) {
            parts.push(sanitize_header_part(email));
        }
        if let Some(phone) = non_empty(&self.phone) {
            parts.push(sanitize_header_part(phone));
        }
        if let Some(url) = non_empty(&self.linkedin).filter(|u| is_safe_header_url(u)) {
            parts.push(format!("[LinkedIn]({})", sanitize_link_url(url)));
        }
        if let Some(url) = non_empty(&self.github).filter(|u| is_safe_header_url(u)) {
            parts.push(format!("[GitHub]({})", sanitize_link_url(url)));
        }
        if let Some(url) = non_empty(&self.website).filter(|u| is_safe_header_url(u)) {
            parts.push(format!("[Website]({})", sanitize_link_url(url)));
        }
        for link in &self.extra_links {
            let (label, url) = (link.label.trim(), link.url.trim());
            if !label.is_empty() && !url.is_empty() && is_safe_header_url(url) {
                parts.push(format!(
                    "[{}]({})",
                    sanitize_link_label(label),
                    sanitize_link_url(url)
                ));
            }
        }
        parts
            .into_iter()
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(" | ")
    }

    /// The header contact line as [`RichText`] (link runs first-class) for the
    /// model-based résumé / DOCX backends. Empty when the profile has no contact
    /// parts, so the caller keeps the text-derived header.
    pub fn header_rich(&self, lang: &str) -> RichText {
        let md = self.header_markdown(lang);
        if md.is_empty() {
            Vec::new()
        } else {
            tokenize_rich(&md)
        }
    }

    /// Fill in a document header's name / contact line from this profile,
    /// localized for `lang` — a **fallback only**. The editor's text is the source
    /// of truth: a header that already carries a name or a contact line (parsed
    /// from the document text) is left untouched; the profile fills in whichever
    /// of the two the text-derived header is missing. No-op on both fields when
    /// the profile itself has nothing to contribute.
    ///
    /// Name fallback: when `header.name` is blank (e.g. export without generation
    /// metadata that normally fills it), `full_name` from this profile is used so
    /// a profile-edited name is never silently dropped in the rendered output.
    pub fn apply_to_header(&self, header: &mut crate::model::document::HeaderBlock, lang: &str) {
        // Fill the name from the profile when the header carries no name yet.
        // Sanitized like every other field `header_markdown` renders — a
        // control character in `full_name` is otherwise the one field that
        // reaches the header unsanitized (`contact_profile_set` accepts
        // arbitrary JSON behind a bare `z.string()`, so this is not merely a
        // browser-input-behaviour guarantee).
        if header.name.trim().is_empty() {
            if let Some(name) = non_empty(&self.full_name) {
                header.name = sanitize_header_part(name);
            }
        }

        // Fill the contact line from the profile only when the text-derived
        // header carries none — the text (what the editor shows) wins whenever
        // it already has a contact line.
        if header.contact.is_empty() {
            let rich = self.header_rich(lang);
            if !rich.is_empty() {
                header.contact = rich;
            }
        }
    }

    /// The set of header URLs this profile would render (for validation parity
    /// checks across documents — the sole input to
    /// `validate::pdf_render_issues`'s `allowed` set). Email is included as a
    /// `mailto:` link.
    ///
    /// Routed through the SAME `is_safe_header_url` filter [`Self::header_markdown`]
    /// applies, and the same PER-VALUE sanitizer — [`sanitize_link_url`] for a
    /// URL that renders inside `[Label](…)` there, `sanitize_header_part` for
    /// the bare email — so the two can never fall out of lockstep: an
    /// unsafe-scheme URL `header_markdown` drops must never appear here as
    /// "the profile's own link" (a phantom entry that would otherwise cause a
    /// spurious, non-blocking `header_url_missing`), and a URL/email carrying
    /// a control character, a link-breaking bracket, or exceeding the length
    /// cap must be compared here in the SAME post-sanitize form it actually
    /// renders in — capping the WRAPPED string instead (`mailto:{email}`,
    /// `[Label](url)`) can cap at a different point than the bare-value cap
    /// `header_markdown` applies, so the genuinely-rendered link fails set
    /// membership and `header_url_mismatch` (CRITICAL, blocking) fires on an
    /// unmodified, legitimate profile.
    ///
    /// ONE deliberate, tested exception to that lockstep (LOW, security
    /// re-review — a stale "always in lockstep" claim with no named
    /// exception is exactly the kind of drift this branch keeps re-finding):
    /// the `extra_links` loop below does NOT require a non-empty `label`,
    /// while [`Self::header_markdown`]'s does — an extra link with an empty
    /// label is listed here as "the profile's own link" even though
    /// `header_markdown` never renders it. That is intentional, not a bug:
    /// it is what makes an incomplete extra-link entry (a URL saved with no
    /// label yet) surface as a `header_url_missing` warning instead of
    /// silently vanishing from validation entirely.
    pub fn header_urls(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(email) = non_empty(&self.email) {
            out.push(format!("mailto:{}", sanitize_header_part(email)));
        }
        for url in [
            non_empty(&self.linkedin),
            non_empty(&self.github),
            non_empty(&self.website),
        ]
        .into_iter()
        .flatten()
        .filter(|u| is_safe_header_url(u))
        {
            out.push(sanitize_link_url(url));
        }
        for link in &self.extra_links {
            let url = link.url.trim();
            if !url.is_empty() && is_safe_header_url(url) {
                out.push(sanitize_link_url(url));
            }
        }
        out
    }
}

/// Scheme allowlist for the four fields that get wrapped in `[Label](url)` —
/// `linkedin`/`github`/`website`/`extra_links` — `http(s)` ONLY.
/// `javascript:`/`data:` (and anything else) never render as a clickable
/// header link, however lenient upstream URL classification/import is.
///
/// MEDIUM (security re-review): `mailto:` used to be allowed here too, but
/// this allowlist ONLY ever gates a value headed into `[Label](url)` — and
/// `model::rich::MD_LINK_RE` (the downstream matcher that turns that markdown
/// back into a real, clickable [`TextRun`](crate::model::rich::TextRun) link)
/// only recognizes an `https?://` URL group, never `mailto:`. A
/// `mailto:`-valued Website/LinkedIn/GitHub/extra-link rendered as literal,
/// unlinked `[Website](mailto:…)` markdown text instead of a clickable link
/// — `EMAIL_RE` would still auto-link the bare address INSIDE that literal
/// text, but the surrounding `[Website](mailto:` / `)` bytes stayed visible.
/// The dedicated `email` field is the correct, and only, place a `mailto:`
/// target belongs — it renders bare and is auto-linked by `EMAIL_RE`, never
/// through this allowlist or the `[Label](url)` construct at all.
fn is_safe_header_url(url: &str) -> bool {
    let lower = url.trim().to_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// True for a Unicode Format (`Cf`) character — invisible, non-rendering
/// characters `char::is_control()` (category `Cc`) does NOT cover. LOW
/// (security re-review): a bidi override — `RIGHT-TO-LEFT OVERRIDE` (U+202E)
/// is the canonical example — embedded in a header value can visually
/// REVERSE the surrounding rendered text, making a label or URL read
/// differently than its actual byte content; every sanitizer below filters
/// this alongside `is_control()`, not just newlines/control characters.
/// Backed by the `regex` crate's Unicode general-category support
/// (`\p{Cf}`) rather than a new dependency — `regex` is already a direct
/// dependency used throughout this crate.
fn is_format_char(c: char) -> bool {
    static CF_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\p{Cf}").unwrap());
    let mut buf = [0u8; 4];
    CF_RE.is_match(c.encode_utf8(&mut buf))
}

/// Strip control characters (newlines above all) + Unicode Format characters
/// (see [`is_format_char`]) and cap length on one already-formatted
/// `header_markdown` part. A raw `\n` in a profile field would otherwise
/// inject an arbitrary extra physical line — including a well-formed section
/// heading — once the header line is spliced into plain, `\n`-split
/// document text.
fn sanitize_header_part(s: &str) -> String {
    const MAX_LEN: usize = 200;
    s.chars()
        .filter(|c| !c.is_control() && !is_format_char(*c))
        .take(MAX_LEN)
        .collect()
}

/// [`sanitize_header_part`], plus drops `[`, `]`, `(`, `)` — for a LABEL
/// (display text, never a real navigable target) that gets spliced into a
/// `[Label](url)` markdown construct. Those four characters could otherwise
/// close the link early or open a second one; [`is_safe_header_url`] only
/// checks the scheme prefix, so an `https://`-prefixed value can still carry
/// one past it. A prior security round judged the live exploit surface
/// already closed (import-derived labels come from [`url_label`], which
/// cannot produce a bracket) — this is defense-in-depth, not a hole being
/// patched. Deleting these characters is fine for a label (display text with
/// no semantic content to preserve); it is NOT fine for a URL — see
/// [`sanitize_link_url`], which this is NOT used for.
fn sanitize_link_label(s: &str) -> String {
    const MAX_LEN: usize = 200;
    s.chars()
        .filter(|c| !c.is_control() && !is_format_char(*c) && !matches!(c, '[' | ']' | '(' | ')'))
        .take(MAX_LEN)
        .collect()
}

/// [`sanitize_header_part`], plus drops `[`/`]` and PERCENT-ENCODES `(`/`)`
/// (`%28`/`%29`) — for a URL value that gets spliced into a `[Label](url)`
/// markdown construct. MEDIUM (security re-review): a URL sanitizer must
/// never DELETE `(`/`)` the way [`sanitize_link_label`] does for a label —
/// parens are common in legitimate URLs (a Wikipedia-style path segment, some
/// callback/redirect URLs with an encoded payload) and deleting them silently
/// corrupts the URL into a DIFFERENT destination, not a broken one.
/// Percent-encoding is semantically transparent (a compliant client decodes
/// `%28`/`%29` back to the exact same URL) while still removing the literal
/// byte that could close the markdown link construct early — the actual
/// property this sanitizer exists for. `[`/`]` stay dropped: a URL
/// legitimately containing them is vanishingly rare (an IPv6 host literal,
/// which never appears in a résumé profile link) and worth nothing real,
/// unlike `(`/`)`.
fn sanitize_link_url(s: &str) -> String {
    const MAX_LEN: usize = 200;
    // Cap the RAW value BEFORE percent-encoding, not after. Encoding EXPANDS
    // (`(` → `%28`, `)` → `%29`, 1 byte becomes 3) — capping the expanded
    // string can truncate mid-escape, leaving a mangled `%2` (or bare `%`)
    // tail: not valid percent-encoding, and not the sanitizer's own output
    // contract. Capping first guarantees every emitted escape is whole; the
    // final encoded string can run slightly past 200 chars in a paren-heavy
    // URL, which is an acceptable, deliberate trade for never emitting a
    // broken escape.
    let cleaned: String = s
        .chars()
        .filter(|c| !c.is_control() && !is_format_char(*c) && !matches!(c, '[' | ']'))
        .take(MAX_LEN)
        .collect();
    cleaned.replace('(', "%28").replace(')', "%29")
}

#[cfg(test)]
mod tests;
