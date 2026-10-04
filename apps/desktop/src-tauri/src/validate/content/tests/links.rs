//! What counts as a link and the one canonical key two spellings of it are compared on.

use super::{support::*, *};

/// `canonical_link(raw)` must be exactly `key`.
#[track_caller]
fn assert_key(raw: &str, key: &str) {
    assert_eq!(factual::canonical_link(raw), key, "{raw:?}");
}

// Every test below is a document that states nothing but the truth and was
// nonetheless issued a Critical. Each names the mechanism that produced it.

/// H1a — a `·`-separated STACK line is a technology list, not a link list.
/// `Socket.IO` is a bare `.io` host, `Bun.sh` a bare `.sh` host, and trimming
/// one technology out of the stack while tailoring reported the other as a
/// project link that had been "missing or altered".
#[test]
fn stack_line_library_names_are_never_read_as_project_links() {
    assert!(
        factual::urls_in("Node.js · Socket.IO · Bun.sh · Deno.dev").is_empty(),
        "library names are not links; got {:?}",
        factual::urls_in("Node.js · Socket.IO · Bun.sh · Deno.dev")
    );
    // …while a real link in any of the accepted forms still is one.
    for real in [
        "https://ledger.example.dev",
        "www.ledger.example.dev",
        "github.com/janedoe/ledger",
        "ledger.example.dev/docs",
    ] {
        assert_eq!(
            factual::urls_in(real).len(),
            1,
            "{real} must still be recognised as a link"
        );
    }

    let source = "PROJECTS\n\n\
                  **Chat Relay** · https://relay.example.dev\n\
                  Node.js · Socket.IO · Bun.sh\n\
                  A tiny websocket relay.\n";
    // Tailoring dropped one technology from the stack line. Nothing else moved.
    let trimmed = "PROJECTS\n\n\
                   **Chat Relay** · https://relay.example.dev\n\
                   Node.js · Socket.IO\n\
                   A tiny websocket relay.\n";
    silent(
        &report_against(trimmed, source),
        FACTUAL_ALTERED_PROJECT_LINK,
    );
}

/// An `urls_in` widening tried once to add a fifth "bare domain, no path,
/// lowercase" arm so the validator would see every domain the renderer's
/// `model::rich::split_urls` turns into a real hyperlink. It was reverted
/// (see `factual/links.rs`'s `URL_RE` doc) because, with no email-aware arm and no
/// lookaround in the `regex` crate, it matched the DOMAIN HALF of an email
/// address as a bare "URL" — `urls_in("jane.doe@gmail.com")` returned
/// `["gmail.com"]` — even though the renderer correctly treats the whole
/// address as one `mailto:` link and never exposes the domain separately.
/// This pins BOTH surfaces so a future widening cannot reopen the gap on
/// just one side: the validator must find no bare-URL match inside an email
/// address, and the renderer's only match for that text is the one `mailto:`
/// link — never an extra bare-domain link fragment.
#[test]
fn an_email_address_never_yields_a_phantom_domain_url() {
    for email in [
        "Jane Doe · jane.doe@gmail.com · Berlin, Germany",
        "team@openai.com",
        "jane@proton.me",
    ] {
        assert!(
            factual::urls_in(email).is_empty(),
            "an email address is not a project URL; got {:?} for {email:?}",
            factual::urls_in(email)
        );

        let spans = crate::model::rich::split_urls(email);
        let links: Vec<&crate::model::rich::Span> = spans
            .iter()
            .filter(|s| matches!(s, crate::model::rich::Span::Link { .. }))
            .collect();
        assert_eq!(
            links.len(),
            1,
            "the renderer must produce exactly one link (the mailto:) for \
             {email:?}, not a second bare-domain fragment; got {spans:?}"
        );
        assert!(
            matches!(links[0], crate::model::rich::Span::Link { url, .. } if url.starts_with("mailto:")),
            "the one link for {email:?} must be a mailto:, got {:?}",
            links[0]
        );
    }
}

/// Renderer/validator parity on a shared corpus. `model::rich::split_urls`
/// (what actually becomes a clickable link in the exported PDF/DOCX) and
/// `validate::content::factual::urls_in` (what the Critical-severity
/// `factual.altered_project_link` guard treats as a claimed link) must agree
/// on whether ordinary, non-email text is link-shaped — this is the
/// invariant the reverted fifth `URL_RE` arm broke (see the test above).
///
/// Email text is deliberately excluded from this table: the renderer legally
/// links it (as `mailto:`), a link type `urls_in` was never meant to model at
/// all (project links, not contact emails) — covered on its own above.
#[test]
fn renderer_and_validator_agree_on_a_shared_url_corpus() {
    let corpus: &[(&str, bool)] = &[
        ("https://github.com/janedoe/ledger", true),
        ("www.example.com/path", true),
        ("github.com/janedoe/ledger", true),
        ("just plain prose with no links at all", false),
        ("Bun.sh", false),
        ("bun.sh", false),
        ("Socket.IO", false),
        ("socket.io", false),
        ("Agile, CI/CD, TDD", false),
    ];
    for (text, expect_link) in corpus {
        let rendered_has_link = crate::model::rich::split_urls(text)
            .iter()
            .any(|s| matches!(s, crate::model::rich::Span::Link { .. }));
        let validator_has_url = !factual::urls_in(text).is_empty();
        assert_eq!(
            rendered_has_link, *expect_link,
            "renderer disagreed with the expected shape for {text:?}"
        );
        assert_eq!(
            validator_has_url, *expect_link,
            "validator disagreed with the expected shape for {text:?}"
        );
    }
}

/// H1b — the same link written a different way is the same link. Compared on a
/// canonical key (scheme dropped, host lowercased, trailing `/` removed,
/// markdown href unwrapped); still REPORTED verbatim when it genuinely differs.
#[test]
fn normalized_link_forms_are_not_altered_links() {
    assert_key(
        "HTTPS://GitHub.com/janedoe/ledger/",
        "github.com/janedoe/ledger",
    );
    assert_eq!(
        factual::canonical_link("github.com/janedoe/ledger"),
        factual::canonical_link("https://github.com/janedoe/ledger")
    );

    let source = "PROJECTS\n\n\
                  **Ledger CLI** · https://ledger.example.dev · https://github.com/janedoe/ledger\n\
                  Rust · SQLite\n";
    for (name, generated) in [
        (
            "scheme dropped",
            "PROJECTS\n\n\
             **Ledger CLI** · ledger.example.dev/ · github.com/janedoe/ledger\n\
             Rust · SQLite\n",
        ),
        (
            "trailing slash + host case",
            "PROJECTS\n\n\
             **Ledger CLI** · https://ledger.example.dev/ · HTTPS://GitHub.com/janedoe/ledger\n\
             Rust · SQLite\n",
        ),
        (
            "markdown links",
            "PROJECTS\n\n\
             **Ledger CLI** · [Website](https://ledger.example.dev) · \
             [GitHub](https://github.com/janedoe/ledger)\n\
             Rust · SQLite\n",
        ),
    ] {
        let report = report_against(generated, source);
        assert!(
            !codes(&report).contains(&FACTUAL_ALTERED_PROJECT_LINK),
            "{name} is the same link written differently; got {:#?}",
            report
                .issues
                .iter()
                .filter(|i| i.code == FACTUAL_ALTERED_PROJECT_LINK)
                .collect::<Vec<_>>()
        );
    }

    // A genuinely different path is still Critical, and the evidence quotes the
    // span verbatim rather than the canonical key.
    let altered = "PROJECTS\n\n\
                   **Ledger CLI** · https://ledger.example.dev · \
                   https://github.com/someone-else/ledger\n\
                   Rust · SQLite\n";
    let report = report_against(altered, source);
    let evidence = fired_evidence(&report, FACTUAL_ALTERED_PROJECT_LINK);
    assert!(
        evidence.contains(&"https://github.com/someone-else/ledger"),
        "evidence must be the verbatim span, not the comparison key; got {evidence:?}"
    );
}

/// H1c — a link with a non-ASCII character ABORTED the app. `canonical_link`
/// stripped the scheme with `&s[..8]`, a fixed BYTE offset, so any URL whose
/// 8th byte fell inside a multibyte char panicked ("byte index 8 is not a char
/// boundary"). Release builds are `panic = "abort"`: the process died mid-run,
/// before the generated document was saved. A German or French project domain is
/// all it took.
///
/// Every boundary the function cuts at gets a straddling char here — byte 8
/// (`https://`) and byte 7 (`http://`) — plus the two-, three- and four-byte
/// widths, so a future "just slice off the scheme" rewrite fails loudly.
#[test]
fn accented_project_links_are_keyed_without_panicking() {
    // Boundary 8: `é` occupies bytes 7-8 in all three of these, so the very
    // first loop iteration (`https://`, len 8) cut inside it. (The `www.` here
    // is stripped as of R14-F3 — `a_www_prefix_is_the_same_link` owns that
    // rule and its own boundary twin; what this row still pins is that the
    // accented tail survives whatever the prefix loop did.)
    assert_key("www.café-berlin.de", "café-berlin.de");
    assert_key("ab.com/éx", "ab.com/éx");
    // …and stripping the scheme that DOES match still leaves the tail intact.
    assert_key("http://éxample.com", "éxample.com");
    assert_eq!(
        factual::canonical_link("http://éxample.com"),
        factual::canonical_link("https://Éxample.com/"),
        "the same host written three ways is one key"
    );

    // Boundary 7: `é` occupies bytes 6-7, so `&s[..8]` is legal but the SECOND
    // iteration (`http://`, len 7) is the one that cut inside the char.
    assert_key("abcdefé.com/x", "abcdefé.com/x");

    // Three- and four-byte chars across the same offsets.
    assert_key("abc.de/日本語", "abc.de/日本語");
    assert_key("abc.de/🚀x", "abc.de/🚀x");
    // Shorter than either scheme — the length guard, not the boundary check.
    assert_key("é.de", "é.de");
    // Host lowercased, accents preserved; path case and accents untouched.
    assert_key(
        "HTTPS://Café.Example.DE/Ünicode/",
        "café.example.de/Ünicode",
    );

    // The three verifier-reproduced URLs, through the FULL validate_content
    // path (the way the panic actually reached a user: a Projects entry).
    let doc = "PROJECTS\n\n\
               **Café Ledger** · www.café-berlin.de · http://éxample.com · ab.com/éx\n\
               Rust · SQLite\n\
               A double-entry bookkeeping tool for freelancers.\n";
    let report = report_against(doc, doc);
    silent(&report, FACTUAL_ALTERED_PROJECT_LINK);
    let criticals = criticals_of(&report);
    assert!(
        criticals.is_empty(),
        "an accented but truthful project link must produce a clean report; got {criticals:#?}"
    );
}

/// R14-F3 — `canonical_link` dropped the scheme but not a leading `www.`, so
/// `https://www.github.com/janedoe/ledger` and `github.com/janedoe/ledger` —
/// the same resource, written the two ways résumés write it — keyed
/// differently and drew TWO `factual.altered_project_link` Criticals: the
/// source's link "missing or altered", plus the generated one "invented".
#[test]
fn a_www_prefix_is_the_same_link() {
    assert_key(
        "https://www.github.com/janedoe/ledger",
        "github.com/janedoe/ledger",
    );
    assert_eq!(
        factual::canonical_link("WWW.GitHub.com/janedoe/ledger/"),
        factual::canonical_link("https://github.com/janedoe/ledger")
    );
    // Accented hosts, on the boundary this function has already panicked on
    // once: the `www.` strip is byte-compared, never byte-sliced blind.
    assert_eq!(
        factual::canonical_link("www.café-berlin.de"),
        factual::canonical_link("café-berlin.de"),
        "the same accented host written two ways is one key"
    );
    assert_key("www.café-berlin.de", "café-berlin.de");
    assert_key("HTTPS://WWW.Café-Berlin.DE/Über/", "café-berlin.de/Über");
    assert_key("www.é.de", "é.de");

    // `www` is a LABEL, not a prefix: a host that merely starts with those
    // three letters keeps them — including the multibyte twin, where a blind
    // four-byte slice would cut inside the char and abort the process.
    assert_key("wwwé.de/x", "wwwé.de/x");
    assert_key("wwwx.example.com/x", "wwwx.example.com/x");
    // Only the FIRST label is stripped — `www.www.example.com` is a different
    // host from `www.example.com` and must stay one.
    assert_key("www.www.example.com/x", "www.example.com/x");

    // End to end: the two spellings of one link, through `validate_content`.
    let source = "PROJECTS\n\n\
                  **Ledger CLI** · https://www.github.com/janedoe/ledger\n\
                  Rust · SQLite\n";
    let generated = "PROJECTS\n\n\
                     **Ledger CLI** · github.com/janedoe/ledger\n\
                     Rust · SQLite\n";
    silent(
        &report_against(generated, source),
        FACTUAL_ALTERED_PROJECT_LINK,
    );
    // …and a genuinely different host is still Critical.
    let altered = "PROJECTS\n\n\
                   **Ledger CLI** · github.com/someone-else/ledger\n\
                   Rust · SQLite\n";
    fired(
        &report_against(altered, source),
        FACTUAL_ALTERED_PROJECT_LINK,
    );
}
