//! Egress inventory — the machine-readable host allowlist backing ADR-0005
//! (`docs/knowledge/decision-records/0005-network-egress-privacy-boundary.md`),
//! which enumerates 8 permitted egress *classes* in prose but, until this file,
//! had no per-host inventory a new outbound host could be checked against. A
//! 2026-07 audit found README/SECURITY literally false about egress once
//! already — this is the drift guard for a repeat.
//!
//! Sibling of `tests/architecture.rs` and deliberately the same shape: a
//! **standalone integration test** using only `std`, scanning source as TEXT
//! without linking the crate. Run: `cargo test --test egress`.
//!
//! ## What this test does NOT prove
//!
//! Every row in [`EGRESS`] is a **first hop** — the literal host a URL string
//! in source points at, nothing more:
//!
//! - `src/net/http.rs`'s shared client follows up to 10 redirects
//!   (`MAX_REDIRECTS`, `net/http.rs:44`) and its redirect guard blocks only
//!   private/loopback/link-local **IP literals** on the `Location` header — a
//!   302 to an undeclared public hostname is followed without ever being
//!   checked against this inventory.
//! - A **new scheme-less host const** (a bare `"host.example"` string used
//!   without a `scheme://` prefix, the same shape as the entries in
//!   [`SCHEMELESS`]) is invisible to EGRESS-1's extractor, which only
//!   recognizes `scheme://host` literals. It only becomes enforceable once a
//!   human notices and adds it to both [`EGRESS`] and [`SCHEMELESS`] — unlike
//!   a scheme-qualified host, it is not auto-detected as new.
//! - `format!("{base}/api/x")` where `base` is a **runtime-configurable**
//!   value (an env override, a user-typed IMAP host, a user-pasted URL) has no
//!   static host in the source literal at all and cannot be extracted.
//! - A real host nested inside a `#[cfg(test)]` **mod block** that the
//!   stripper (below) fails to detect as such would be silently skipped —
//!   see `egress_cfg_test_stripper_never_swallows_a_non_mod_attribute_target`
//!   for the guard
//!   against that specific failure mode.
//!
//! None of this is a runtime guarantee; it is a **static drift check**: does
//! the current source text declare a host that isn't in the inventory, or
//! vice versa.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

// ── Declaration files (a file move is a compile error, not a silent gap) ────
const TAURI_CONF: &str = include_str!("../tauri.conf.json");
const VITE_CONFIG: &str = include_str!("../../vite.config.mts");
const EXTENSION_MANIFEST: &str = include_str!("../../../extension/src/manifest.ts");

const DECL_FILES: &[(&str, &str)] = &[
    ("apps/desktop/src-tauri/tauri.conf.json", TAURI_CONF),
    ("apps/desktop/vite.config.mts", VITE_CONFIG),
    ("apps/extension/src/manifest.ts", EXTENSION_MANIFEST),
];

// ── The inventory ─────────────────────────────────────────────────────────

struct Egress {
    /// Lowercased authority exactly as the extractor yields it: userinfo
    /// stripped, port stripped, placeholder/wildcard labels dropped.
    host: &'static str,
    /// Third-party name that must appear in the public prose surfaces
    /// (README/SECURITY/ADR-0005). `None` = no per-host disclosure
    /// obligation; `note` must then say which generic clause covers it.
    /// Never `None` for a named third party.
    public_name: Option<&'static str>,
    /// What it is and what gates it. Free text — one host can have several
    /// roles (`www.linkedin.com` has four), so there is deliberately no
    /// `Class` enum field here.
    note: &'static str,
}

#[rustfmt::skip]
const EGRESS: &[Egress] = &[
    // ── AI providers (ADR-0005 class 1) — "the AI provider you configure";
    // never enumerated by name because a new provider must work with zero
    // code changes (roadmap: future-proof extensibility).
    Egress { host: "api.anthropic.com", public_name: None, note: "AI provider you configure (Anthropic). commands/ai_provider/anthropic.rs." },
    Egress { host: "api.openai.com", public_name: None, note: "AI provider you configure (OpenAI). commands/ai_provider/openai.rs." },
    Egress { host: "generativelanguage.googleapis.com", public_name: None, note: "AI provider you configure (Gemini). commands/ai_provider/gemini.rs." },
    Egress { host: "ollama.com", public_name: None, note: "AI provider you configure (Ollama Cloud). commands/ai_provider/{ollama,ollama_cloud}.rs." },
    // CLI-agent install-help links: rendered in Settings so the user can go
    // install the CLI, never fetched by this app.
    Egress { host: "code.claude.com", public_name: None, note: "CLI install-help link (Claude Code); rendered only, never fetched. commands/ai_provider/cli_agent/claude_code.rs." },
    Egress { host: "developers.openai.com", public_name: None, note: "CLI install-help link (Codex CLI); rendered only, never fetched. commands/ai_provider/cli_agent/codex.rs." },
    Egress { host: "geminicli.com", public_name: None, note: "CLI install-help link (Gemini CLI); rendered only, never fetched. commands/ai_provider/cli_agent/gemini_cli.rs." },
    Egress { host: "antigravity.google", public_name: None, note: "CLI install-help link (Antigravity CLI); rendered only, never fetched. commands/ai_provider/cli_agent/antigravity.rs." },

    // ── Web search (ADR-0005 class 3, ADR-0023) — opt-in.
    Egress { host: "api.exa.ai", public_name: Some("Exa"), note: "Opt-in web-search backend for AI company research (ADR-0023), off unless a provider needing it is configured. commands/ai_provider/search/mod.rs." },

    // ── Job boards / aggregators / ATS platforms (ADR-0005 class 2) — the
    // app's core function, covered by the generic "job boards you scrape"
    // disclosure rather than a per-board name.
    Egress { host: "api.adzuna.com", public_name: Some("Adzuna"), note: "Aggregator primary tier (ADR-026): search + redirect-URL resolution. scraping/boards/aggregator/adzuna.rs." },
    Egress { host: "jsearch.p.rapidapi.com", public_name: Some("RapidAPI"), note: "JSearch, aggregator paid fallback tier, via RapidAPI. scraping/boards/aggregator/providers.rs." },
    Egress { host: "jooble.org", public_name: Some("Jooble"), note: "Aggregator last-resort fallback tier. scraping/boards/aggregator/providers.rs." },
    Egress { host: "api.apify.com", public_name: Some("Apify"), note: "Aggregator LinkedIn actor tier — additive, opt-in, paid. scraping/boards/aggregator/providers.rs." },
    Egress { host: "api.ashbyhq.com", public_name: None, note: "Ashby ATS board fetch. scraping/boards/ashby/mod.rs." },
    Egress { host: "jobs.ashbyhq.com", public_name: None, note: "Ashby single-pasted-URL resolver. scraping/scrape_url/mod.rs." },
    Egress { host: "api.lever.co", public_name: None, note: "Lever ATS board fetch + single-URL resolver. scraping/boards/lever/mod.rs, scraping/scrape_url/mod.rs." },
    Egress { host: "api.rippling.com", public_name: None, note: "Rippling ATS board listing API (posting URLs live on ats.rippling.com, see SCHEMELESS). scraping/boards/rippling/mod.rs." },
    Egress { host: "api.smartrecruiters.com", public_name: None, note: "SmartRecruiters ATS board fetch. scraping/boards/smartrecruiters/mod.rs." },
    Egress { host: "jobs.smartrecruiters.com", public_name: None, note: "SmartRecruiters single-pasted-URL resolver. scraping/scrape_url/mod.rs." },
    Egress { host: "apply.workable.com", public_name: None, note: "Workable ATS board fetch. scraping/boards/workable/mod.rs." },
    Egress { host: "bamboohr.com", public_name: None, note: "BambooHR ATS board fetch ({slug}.bamboohr.com). scraping/boards/bamboohr/mod.rs." },
    Egress { host: "breezy.hr", public_name: None, note: "Breezy HR ATS board fetch ({slug}.breezy.hr). scraping/boards/breezy/mod.rs." },
    Egress { host: "recruitee.com", public_name: None, note: "Recruitee ATS board fetch ({slug}.recruitee.com). scraping/boards/recruitee/mod.rs." },
    Egress { host: "pinpointhq.com", public_name: None, note: "Pinpoint ATS board fetch ({slug}.pinpointhq.com). scraping/boards/pinpoint/mod.rs." },
    Egress { host: "boards-api.greenhouse.io", public_name: None, note: "Greenhouse ATS board API fetch + single-URL resolver. scraping/boards/greenhouse/mod.rs, scraping/scrape_url/mod.rs." },
    Egress { host: "www.comeet.co", public_name: None, note: "Comeet ATS board fetch. scraping/boards/comeet/mod.rs." },
    Egress { host: "myworkdayjobs.com", public_name: None, note: "Workday single-pasted-URL resolver ({tenant}.{company}.myworkdayjobs.com). scraping/scrape_url/mod.rs." },
    Egress { host: "berlinstartupjobs.com", public_name: None, note: "Berlin Startup Jobs board fetch (own WordPress RSS permalink). scraping/boards/berlinstartupjobs/mod.rs." },
    Egress { host: "germantechjobs.de", public_name: None, note: "German Tech Jobs board fetch. scraping/boards/germantechjobs/mod.rs." },
    Egress { host: "jobicy.com", public_name: None, note: "Jobicy board fetch; own posting-page URL required by Jobicy's ToS attribution. scraping/boards/jobicy/mod.rs." },
    Egress { host: "www.arbeitnow.com", public_name: None, note: "Arbeitnow board fetch. scraping/boards/arbeitnow/mod.rs." },
    Egress { host: "rest.arbeitsagentur.de", public_name: None, note: "German Federal Employment Agency board REST API fetch. scraping/boards/arbeitsagentur/mod.rs." },
    Egress { host: "www.arbeitsagentur.de", public_name: None, note: "German Federal Employment Agency board site (alongside the REST API host). scraping/boards/arbeitsagentur/mod.rs." },
    Egress { host: "weworkremotely.com", public_name: None, note: "We Work Remotely board fetch. scraping/boards/wwr/mod.rs." },
    Egress { host: "remoteok.com", public_name: None, note: "RemoteOK board fetch. scraping/boards/remoteok/mod.rs." },
    Egress { host: "remotive.com", public_name: None, note: "Remotive board fetch. scraping/boards/remotive/mod.rs." },
    Egress { host: "www.themuse.com", public_name: None, note: "The Muse board fetch. scraping/boards/themuse/mod.rs." },
    Egress { host: "news.ycombinator.com", public_name: None, note: "Y Combinator (\"Who's Hiring\") board site, alongside the Firebase API host. scraping/boards/ycombinator/mod.rs." },
    Egress { host: "hacker-news.firebaseio.com", public_name: None, note: "Y Combinator (\"Who's Hiring\") board Firebase API fetch. scraping/boards/ycombinator/mod.rs." },
    Egress { host: "www.linkedin.com", public_name: None, note: "Four roles: login page (board_login), native board search + geo-typeahead (scraping/boards/linkedin, scraping/linkedin/api_client), the Apify actor's LinkedIn URL construction (aggregator fallback), and the single-pasted-URL resolver (scrape_url)." },
    Egress { host: "secure.indeed.com", public_name: None, note: "Indeed login page, opened for the user to authenticate. scraping/board_login/mod.rs." },
    Egress { host: "login.xing.com", public_name: None, note: "Xing login page, opened for the user to authenticate. scraping/board_login/mod.rs." },
    Egress { host: "www.glassdoor.com", public_name: None, note: "Glassdoor login page, opened for the user to authenticate. scraping/board_login/mod.rs." },

    // ── Location autocomplete (ADR-0005 class 5) — offline-first fallback.
    Egress { host: "photon.komoot.io", public_name: Some("Photon"), note: "Location-autocomplete fallback (OpenStreetMap/ODbL) used only when the bundled offline GeoNames index has no match. commands/geocoding.rs." },

    // ── Profile import (opt-in, user-initiated).
    Egress { host: "api.github.com", public_name: Some("GitHub"), note: "\"Import from GitHub\" resume-builder flow — public, non-fork repos only. profile_import/github.rs." },

    // ── Email-confirmation watching (ADR-0005 class 7) — opt-in, default OFF.
    Egress { host: "imap.gmail.com", public_name: Some("IMAP"), note: "Default (v1 Gmail-branded) IMAP host for opt-in email-confirmation watching; DATA not a hardcoded destination — user-configurable, credential OS-keychain-backed. Scheme-less: email_watch/imap_client.rs::DEFAULT_IMAP_HOST (see SCHEMELESS)." },

    // ── Updater (ADR-0005 class 4) — on-launch version check, no user data.
    Egress { host: "github.com", public_name: Some("GitHub"), note: "Updater version-check endpoint, configured in tauri.conf.json's plugins.updater.endpoints; also the Help-menu doc/issues/changelog links opened in the user's browser, never fetched by net::http. lib.rs, updater/mod.rs, commands/menu.rs." },

    // ── Optional enrichment (ADR-0005 class 6) — opt-in, default OFF, from
    // the declaration files (tauri.conf.json's CSP), not Rust src.
    Egress { host: "logo.clearbit.com", public_name: Some("Clearbit"), note: "Opt-in company-logo enrichment, default OFF. CSP img-src only (renderer-fetched, not net::http). tauri.conf.json." },
    Egress { host: "autocomplete.clearbit.com", public_name: Some("Clearbit"), note: "Opt-in company-logo enrichment's name-autocomplete lookup, default OFF, behind the same preference. CSP connect-src only. tauri.conf.json." },

    // ── Loopback (never a public host).
    Egress { host: "127.0.0.1", public_name: None, note: "Loopback only: local Ollama's default endpoint (platform::config::ollama_host) and the extension-bridge WS relay (extension_bridge/native_host.rs, tauri.conf.json, vite.config.mts, apps/extension/src/manifest.ts)." },

    // ── Build tooling, never fetched at runtime.
    Egress { host: "schema.tauri.app", public_name: None, note: "Build-time $schema reference in tauri.conf.json; never fetched by the running app." },

    // ── SCHEMELESS entries (see the SCHEMELESS const below for why EGRESS-1
    // can never see these — they never appear with a scheme:// prefix).
    Egress { host: "jobs.personio.de", public_name: None, note: "Personio ATS board fetch host, bare (no scheme) HOSTS const entry. scraping/boards/personio/mod.rs:11." },
    Egress { host: "jobs.personio.com", public_name: None, note: "Personio ATS board fallback host, same bare HOSTS const. scraping/boards/personio/mod.rs:11." },
    Egress { host: "ats.rippling.com", public_name: None, note: "Rippling job-posting URL host; validated as a bare literal in real code (scheme-qualified only inside the file's own #[cfg(test)] fixtures). scraping/ats_ref.rs, scraping/boards/rippling/mod.rs::is_valid_rippling_job_url." },
    Egress { host: "boards.greenhouse.io", public_name: None, note: "Greenhouse public careers-page host, recognized as a bare literal for discovered-link classification. scraping/ats_ref.rs::greenhouse_slug." },
    Egress { host: "job-boards.greenhouse.io", public_name: None, note: "Greenhouse public careers-page host (newer domain), same bare-literal recognizer. scraping/ats_ref.rs::greenhouse_slug." },
    Egress { host: "boards.eu.greenhouse.io", public_name: None, note: "Greenhouse EU-region careers-page host, same bare-literal recognizer. scraping/ats_ref.rs::greenhouse_slug." },
];

/// Hosts that only ever appear WITHOUT a `scheme://` prefix in real code (a
/// bare comparison/DNS-label const, not a `scheme://host` literal) —
/// EGRESS-1's scheme-anchored extractor can never see these, so EGRESS-2
/// checks them by searching for the quoted literal (`"host"`) in
/// comment-stripped, non-test source instead of extracted-set membership.
const SCHEMELESS: &[&str] = &[
    "jobs.personio.de",
    "jobs.personio.com",
    "ats.rippling.com",
    "boards.greenhouse.io",
    "job-boards.greenhouse.io",
    "boards.eu.greenhouse.io",
    "imap.gmail.com",
];

/// Declared dynamic-host sites: a `scheme://` literal whose entire authority
/// is placeholder/format-arg text (no real label survives extraction), keyed
/// on `(path, matched literal)` rather than line number so an edit above the
/// site doesn't churn this table.
const DYNAMIC_SITES: &[(&str, &str)] = &[
    ("model/rich.rs", "https://{url}"),
    ("scraping/board_login/mod.rs", "https://{host}"),
    ("scraping/boards/personio/mod.rs", "https://{}.{}"),
    ("scraping/scrape_url/mod.rs", "https://{host}"),
    ("scraping/scrape_url/mod.rs", "https://{}"),
];

// ── Source-tree access (mirrors tests/architecture.rs's `sources()`) ────────

struct RustSource {
    /// Path relative to `src/`, always forward-slashed.
    rel: String,
    content: String,
}

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// True for a file whose path component (in practice, always the filename —
/// no `test`/`tests` DIRECTORY exists in this tree) is exactly `test.rs` /
/// `tests.rs`, or whose stem ends in `_test`/`_tests`. Matched as a path
/// component / filename suffix, **never** a bare `ends_with("test.rs")` —
/// that would also eat a future `latest.rs`/`contest.rs`.
fn is_test_file(rel: &str) -> bool {
    if rel.split('/').any(|c| c == "test.rs" || c == "tests.rs") {
        return true;
    }
    let filename = rel.rsplit('/').next().unwrap_or(rel);
    let stem = filename.strip_suffix(".rs").unwrap_or(filename);
    stem.ends_with("_test") || stem.ends_with("_tests")
}

/// True for lines that are purely a comment (`//`, `///`, `//!`, block-comment
/// body) — identical rule to `tests/architecture.rs::is_comment_line`,
/// duplicated here because integration-test binaries in `tests/` don't share
/// modules with each other in this crate.
fn is_comment_line(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("//") || t.starts_with('*') || t.starts_with("/*")
}

fn collect(dir: &Path, root: &Path, out: &mut Vec<RustSource>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, root, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if is_test_file(&rel) {
                continue;
            }
            let content = fs::read_to_string(&path).unwrap_or_default();
            out.push(RustSource { rel, content });
        }
    }
}

fn rust_sources() -> Vec<RustSource> {
    let root = src_root();
    let mut out = Vec::new();
    collect(&root, &root, &mut out);
    assert!(
        !out.is_empty(),
        "no scanned .rs files found under {}",
        root.display()
    );
    out
}

// ── #[cfg(test)] mod-block stripper ─────────────────────────────────────────

/// Strip column-0 `#[cfg(test)]` … `mod name { … }` blocks (real inline test
/// code with its own scheme-qualified fixture URLs) from `content`. Does
/// **not** touch a `#[cfg(test)]` that sits on anything else (`use`, `const`,
/// `enum`, `fn`, or a `#[path = "…"] mod name;` external-file reference) —
/// those hold no inline test code to leak, and the naive "strip to the next
/// standalone `}`" approach would delete real declarations. Dozens of
/// column-0 `#[cfg(test)]` attributes in this tree sit on something other
/// than a `mod … {` block (e.g. `validate/content/mod.rs:66` on a `use`,
/// `commands/ai_provider/stream.rs:528` on `enum StreamSink {`,
/// `commands/ai_provider/anthropic.rs:1394` on a `#[path] mod tests;`
/// external-file reference) — `egress_cfg_test_stripper_never_swallows_a_non_mod_attribute_target`
/// pins exactly those three as regression cases, rather than pinning a total
/// region count (which would redden every time anyone anywhere adds an
/// unrelated `#[cfg(test)] mod tests { … }` — a routine, desirable change).
///
/// Returns the content with those regions removed, plus how many regions were
/// stripped (used only as a loose sanity signal, never an exact pin).
fn strip_cfg_test_mod_blocks(content: &str) -> (String, usize) {
    let lines: Vec<&str> = content.lines().collect();
    let mut out: Vec<&str> = Vec::with_capacity(lines.len());
    let mut stripped = 0usize;
    let mut i = 0usize;
    while i < lines.len() {
        if lines[i] == "#[cfg(test)]" {
            // Skip forward past any directly-following attribute lines (e.g.
            // `#[path = "…"]`) to find what the attribute chain lands on.
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim_start().starts_with("#[") {
                j += 1;
            }
            let opens_mod = j < lines.len() && {
                let t = lines[j].trim_start();
                let t_end = t.trim_end();
                // Strip an optional `pub`/`pub(...)` visibility prefix, THEN
                // require the next token to be exactly `mod ` — a bare
                // `t.starts_with("pub(")` check (without stripping to the
                // token after it) also matches a `#[cfg(test)]`-annotated
                // test-only HELPER function/impl such as
                // `pub(super) fn row_counts() -> (usize, usize) {`
                // (commands/geocoding/geonames.rs), which is real code, not a
                // module block, and must not be stripped.
                let after_vis = if let Some(rest) = t.strip_prefix("pub(") {
                    match rest.find(')') {
                        Some(close) => rest[close + 1..].trim_start(),
                        None => t,
                    }
                } else if let Some(rest) = t.strip_prefix("pub ") {
                    rest.trim_start()
                } else {
                    t
                };
                after_vis.starts_with("mod ") && t_end.ends_with('{')
            };
            if opens_mod {
                // Brace-count from j to find the matching close. Naive: counts
                // every `{`/`}` BYTE including ones inside string literals —
                // safe here because every test fixture in this tree with
                // embedded braces (JSON strings) is itself balanced, verified
                // empirically: every stripped region resolves with zero URL
                // leaks past its boundary (no test-only fixture host — e.g.
                // `https://ats.rippling.com`, `https://jobs.lever.co` — ever
                // appears in the extracted set).
                let mut depth = 0i32;
                let mut k = j;
                let mut started = false;
                while k < lines.len() {
                    for ch in lines[k].chars() {
                        if ch == '{' {
                            depth += 1;
                            started = true;
                        } else if ch == '}' {
                            depth -= 1;
                        }
                    }
                    if started && depth <= 0 {
                        break;
                    }
                    k += 1;
                }
                stripped += 1;
                i = k + 1;
                continue;
            }
        }
        out.push(lines[i]);
        i += 1;
    }
    (out.join("\n"), stripped)
}

fn strip_comment_lines(content: &str) -> String {
    content
        .lines()
        .filter(|l| !is_comment_line(l))
        .collect::<Vec<_>>()
        .join("\n")
}

// ── Scheme/authority extraction (rule steps 4-9 in the task spec) ──────────

const SCHEME_WORDS: &[&str] = &["https", "http", "wss", "ws"];
const STOP_BYTES: &[u8] = b"/?#\"'`)],;<> \t\r\n";

/// If `content[..pos]` (where `pos` is the byte index of `://`) ends with a
/// recognized scheme word bounded by a non-alphanumeric (or start-of-string),
/// return the scheme's start byte index. The `*` alternative catches MV3
/// match patterns (`*://*.example.com/*`).
fn scheme_start(content: &str, pos: usize) -> Option<usize> {
    let before = &content[..pos];
    if before.ends_with('*') {
        let start = pos - 1;
        if start == 0 || !content.as_bytes()[start - 1].is_ascii_alphanumeric() {
            return Some(start);
        }
    }
    for w in SCHEME_WORDS {
        if before.ends_with(w) {
            let start = pos - w.len();
            if start == 0 || !content.as_bytes()[start - 1].is_ascii_alphanumeric() {
                return Some(start);
            }
        }
    }
    None
}

fn authority_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() && !STOP_BYTES.contains(&bytes[i]) {
        i += 1;
    }
    i
}

/// RFC-2606/6761 reserved names — unconditionally dropped wherever they
/// appear (exact match or as a subdomain).
fn is_reserved(host: &str) -> bool {
    const EXACT: &[&str] = &[
        "localhost",
        "example.com",
        "example.org",
        "example.net",
        "example",
        "test",
        "invalid",
        "local",
    ];
    const SUFFIXES: &[&str] = &[
        ".example.com",
        ".example.org",
        ".example.net",
        ".example",
        ".test",
        ".invalid",
        ".local",
        ".localhost",
    ];
    EXACT.contains(&host) || SUFFIXES.iter().any(|s| host.ends_with(s))
}

/// `None` = bare `scheme://` literal (empty authority) — contributes nothing.
/// `Some(None)` = a dynamic-host site (no label survived placeholder-stripping).
/// `Some(Some(host))` = a real, non-reserved host.
fn process_authority(raw: &str) -> Option<Option<String>> {
    let after_userinfo = raw.rsplit('@').next().unwrap_or(raw);
    let before_port = after_userinfo.split(':').next().unwrap_or(after_userinfo);
    let lowered = before_port.to_lowercase();
    if lowered.is_empty() {
        return None;
    }
    let kept: Vec<&str> = lowered
        .split('.')
        .filter(|label| !label.contains('{') && !label.contains('}') && !label.contains('*'))
        .collect();
    if kept.is_empty() {
        Some(None)
    } else {
        let host = kept.join(".");
        if is_reserved(&host) {
            None
        } else {
            Some(Some(host))
        }
    }
}

/// host -> first `(path, matched literal)` seen for that host — used to build
/// EGRESS-1's "first seen at" error message.
type HostSites = BTreeMap<String, (String, String)>;
/// `(path, matched literal)` for every dynamic-host site occurrence.
type DynamicSites = Vec<(String, String)>;

/// Scan `content` (already comment/test-mod stripped) for `scheme://authority`
/// literals, folding results into `hosts` (host -> first `(path, matched
/// literal)` seen, for error messages) and `dynamic` (every dynamic-host site
/// occurrence, path + matched literal).
fn scan_one(rel: &str, content: &str, hosts: &mut HostSites, dynamic: &mut DynamicSites) {
    let bytes = content.as_bytes();
    let mut idx = 0usize;
    while idx < content.len() {
        let Some(off) = content[idx..].find("://") else {
            break;
        };
        let pos = idx + off;
        if let Some(s_start) = scheme_start(content, pos) {
            let scheme = &content[s_start..pos];
            let auth_start = pos + 3;
            let auth_end = authority_end(bytes, auth_start);
            let raw_auth = std::str::from_utf8(&bytes[auth_start..auth_end]).unwrap_or("");
            match process_authority(raw_auth) {
                None => {} // rule 8 (or reserved): contributes nothing
                Some(None) => {
                    dynamic.push((rel.to_string(), format!("{scheme}://{raw_auth}")));
                }
                Some(Some(host)) => {
                    hosts
                        .entry(host)
                        .or_insert_with(|| (rel.to_string(), format!("{scheme}://{raw_auth}")));
                }
            }
            idx = auth_end.max(pos + 3);
        } else {
            idx = pos + 3;
        }
    }
}

/// Run the full pipeline (strip cfg(test) mod blocks, strip comments, scan)
/// over the scanned Rust source tree. Returns `(hosts, dynamic_sites,
/// total_stripped_regions)`.
fn extract_from_rust_sources() -> (HostSites, DynamicSites, usize) {
    let mut hosts = BTreeMap::new();
    let mut dynamic = Vec::new();
    let mut total_stripped = 0usize;
    for f in rust_sources() {
        let (no_test_mods, stripped) = strip_cfg_test_mod_blocks(&f.content);
        total_stripped += stripped;
        let clean = strip_comment_lines(&no_test_mods);
        scan_one(&f.rel, &clean, &mut hosts, &mut dynamic);
    }
    (hosts, dynamic, total_stripped)
}

/// Same pipeline over the three declaration files (comment-line stripped for
/// consistency; harmless for JSON, and manifest.ts's own comments carry no
/// scheme-qualified hosts today).
fn extract_from_decl_files(hosts: &mut HostSites, dynamic: &mut DynamicSites) {
    for (path, content) in DECL_FILES {
        let clean = strip_comment_lines(content);
        scan_one(path, &clean, hosts, dynamic);
    }
}

fn egress_host_set() -> BTreeSet<&'static str> {
    EGRESS.iter().map(|e| e.host).collect()
}

// ── EGRESS-1: every extracted host is declared ──────────────────────────────

#[test]
fn egress_1_every_extracted_host_is_declared() {
    let (mut hosts, mut dynamic, _stripped) = extract_from_rust_sources();
    extract_from_decl_files(&mut hosts, &mut dynamic);

    let declared = egress_host_set();
    let mut undeclared: Vec<(String, String, String)> = hosts
        .into_iter()
        .filter(|(h, _)| !declared.contains(h.as_str()))
        .map(|(h, (rel, snippet))| (h, rel, snippet))
        .collect();
    undeclared.sort();

    if undeclared.is_empty() {
        return;
    }
    let mut msg =
        String::from("\nEGRESS-1 FAILED — a new outbound host was found that is not declared:\n");
    for (host, rel, snippet) in &undeclared {
        msg.push_str(&format!("  {host}  (first seen at {rel}: `{snippet}`)\n"));
    }
    msg.push_str(
        "\nTo fix: add an `Egress {{ host, public_name, note }}` row for each host above to \
         the `EGRESS` const in apps/desktop/src-tauri/tests/egress.rs. If it is a NEW \
         third-party service not already covered by an existing ADR-0005 egress class, also \
         update the enumeration in README.md, SECURITY.md, and \
         docs/knowledge/decision-records/0005-network-egress-privacy-boundary.md.\n",
    );
    panic!("{msg}");
}

// ── EGRESS-2: every declared host is still present in source ───────────────

#[test]
fn egress_2_every_declared_host_is_still_in_source() {
    let (mut hosts, mut dynamic, _stripped) = extract_from_rust_sources();
    extract_from_decl_files(&mut hosts, &mut dynamic);

    // Comment-stripped, cfg(test)-mod-stripped Rust text for the SCHEMELESS
    // bare-literal search — NOT a raw `contains` over unstripped file text,
    // which would be vacuously satisfied by the 31-hostname `ATS_ALLOWLIST`
    // policy list at scraping/trust/mod.rs:76-108 (a suffix-match allowlist,
    // not an egress declaration) or by a stale doc-comment mention.
    let schemeless_text: String = rust_sources()
        .iter()
        .map(|f| {
            let (no_mods, _) = strip_cfg_test_mod_blocks(&f.content);
            strip_comment_lines(&no_mods)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut dead: Vec<&str> = Vec::new();
    for e in EGRESS {
        let present = if SCHEMELESS.contains(&e.host) {
            schemeless_text.contains(&format!("\"{}\"", e.host))
        } else {
            hosts.contains_key(e.host)
        };
        if !present {
            dead.push(e.host);
        }
    }
    assert!(
        dead.is_empty(),
        "\nEGRESS-2 FAILED — these EGRESS entries are no longer present in source (remove \
         their rows from EGRESS in apps/desktop/src-tauri/tests/egress.rs, or update the row \
         if the host moved): {dead:?}\n"
    );
}

// ── EGRESS-3: dynamic-host sites are declared ───────────────────────────────

#[test]
fn egress_3_dynamic_host_sites_are_declared() {
    let (_hosts, dynamic, _stripped) = extract_from_rust_sources();
    let declared: BTreeSet<(&str, &str)> = DYNAMIC_SITES.iter().map(|&(p, l)| (p, l)).collect();

    let undeclared: BTreeSet<(String, String)> = dynamic
        .iter()
        .filter(|(p, l)| !declared.contains(&(p.as_str(), l.as_str())))
        .cloned()
        .collect();
    assert!(
        undeclared.is_empty(),
        "\nEGRESS-3 FAILED — undeclared dynamic-host site(s) (a scheme:// literal whose \
         entire authority is a format placeholder, e.g. `https://{{host}}`): \
         {undeclared:?}\nAdd each `(path, matched literal)` pair to DYNAMIC_SITES in \
         apps/desktop/src-tauri/tests/egress.rs.\n"
    );

    // Dead-entry half: every declared site must still be extracted, keyed on
    // (path, literal) — not (path, line), which would churn on any edit
    // above the site.
    let extracted: BTreeSet<(&str, &str)> = dynamic
        .iter()
        .map(|(p, l)| (p.as_str(), l.as_str()))
        .collect();
    let stale: Vec<(&str, &str)> = DYNAMIC_SITES
        .iter()
        .copied()
        .filter(|site| !extracted.contains(site))
        .collect();
    assert!(
        stale.is_empty(),
        "\nEGRESS-3 FAILED — these DYNAMIC_SITES entries no longer match any extracted \
         dynamic-host site (remove or update them in apps/desktop/src-tauri/tests/egress.rs): \
         {stale:?}\n"
    );
}

// ── EGRESS-4: no bare scheme / wildcard host in the declaration files ──────

/// A CSP/manifest source that is a bare scheme (`https:` with no `//…`) or an
/// explicit wildcard host (`*://*/*`, `<all_urls>`) defeats the whole point
/// of an enumerated inventory — it allows ANY host. Deliberately separate
/// from the extractor (which only recognizes `scheme://host` literals) rather
/// than folding this into the authority-extraction rule.
fn find_bare_scheme_or_wildcard(content: &str) -> Vec<String> {
    let mut hits = Vec::new();
    if content.contains("*://*/*") {
        hits.push("wildcard host pattern `*://*/*`".to_string());
    }
    if content.contains("<all_urls>") {
        hits.push("wildcard host pattern `<all_urls>`".to_string());
    }
    for scheme in ["https:", "http:", "ws:", "wss:"] {
        let mut start = 0usize;
        while let Some(off) = content[start..].find(scheme) {
            let pos = start + off;
            let after = pos + scheme.len();
            if !content[after..].starts_with("//") {
                hits.push(format!("bare scheme `{scheme}` (no `//` following)"));
            }
            start = after;
        }
    }
    hits
}

#[test]
fn egress_4_declaration_files_have_no_bare_scheme_or_wildcard_source() {
    let mut v = Vec::new();
    for (path, content) in DECL_FILES {
        for hit in find_bare_scheme_or_wildcard(content) {
            v.push(format!("{path}: {hit}"));
        }
    }
    assert!(
        v.is_empty(),
        "\nEGRESS-4 FAILED — a CSP/manifest source is a bare scheme or wildcard host, which \
         allows ANY host and defeats this whole inventory: {v:?}\nUse a specific \
         `scheme://host` entry instead.\n"
    );
}

#[cfg(test)]
mod egress_4_detection_logic {
    use super::find_bare_scheme_or_wildcard;

    #[test]
    fn flags_a_bare_https_scheme_source() {
        let hits = find_bare_scheme_or_wildcard("connect-src 'self' https:");
        assert_eq!(
            hits.len(),
            1,
            "a bare `https:` CSP source must be flagged; got {hits:?}"
        );
    }

    #[test]
    fn flags_a_wildcard_match_pattern() {
        let hits = find_bare_scheme_or_wildcard(r#"host_permissions: ["*://*/*"]"#);
        assert_eq!(hits.len(), 1, "got {hits:?}");
    }

    #[test]
    fn flags_all_urls() {
        let hits = find_bare_scheme_or_wildcard(r#"permissions: ["<all_urls>"]"#);
        assert_eq!(hits.len(), 1, "got {hits:?}");
    }

    #[test]
    fn does_not_flag_a_normal_scheme_qualified_host() {
        let hits = find_bare_scheme_or_wildcard(
            "connect-src 'self' https://api.example.com ws://127.0.0.1:1",
        );
        assert!(
            hits.is_empty(),
            "a normal scheme://host source must not be flagged; got {hits:?}"
        );
    }
}

// ── Supporting guards ────────────────────────────────────────────────────

/// Regression guard for the exact failure mode `strip_cfg_test_mod_blocks`
/// exists to avoid: a `#[cfg(test)]` that sits on something OTHER than a
/// `mod … {` block must be left completely alone. Deliberately NOT a pinned
/// total region count — that would redden every time anyone anywhere in the
/// tree adds an unrelated `#[cfg(test)] mod tests { … }` (a routine,
/// desirable change; this repo has explicitly rejected checks that cry wolf
/// on maintenance activity). Instead this pins the three known-tricky
/// survivors the extraction rule was measured against: an attribute on a
/// `use` (`validate/content/mod.rs`), on an `enum`
/// (`commands/ai_provider/stream.rs`), and on a `#[path] mod name;`
/// external-file reference (`commands/ai_provider/anthropic.rs`). Each line
/// must be found, byte-for-byte, in the stripped output. This is what
/// mutation-tests the stripper: swap it for a naive "strip anything after
/// `#[cfg(test)]` to the next standalone `}`" implementation and this test
/// goes red, because a naive stripper does NOT distinguish these targets
/// from a real `mod … {` block.
#[test]
fn egress_cfg_test_stripper_never_swallows_a_non_mod_attribute_target() {
    let cases: &[(&str, &str)] = &[
        (
            "validate/content/mod.rs",
            "use self::language::{is_language_mismatch, looks_like_prose, PROSE_LOWERCASE_WORD_RATIO};",
        ),
        (
            "validate/content/mod.rs",
            "pub use self::language::document_language_mismatch;",
        ),
        ("commands/ai_provider/stream.rs", "enum StreamSink {"),
        (
            "commands/ai_provider/anthropic.rs",
            "#[path = \"anthropic_tests.rs\"]",
        ),
        ("commands/ai_provider/anthropic.rs", "mod tests;"),
    ];
    let sources: BTreeMap<String, String> = rust_sources()
        .into_iter()
        .map(|f| (f.rel, f.content))
        .collect();
    let mut missing = Vec::new();
    for (rel, needle) in cases {
        let Some(content) = sources.get(*rel) else {
            missing.push(format!("{rel}: file not found among scanned sources"));
            continue;
        };
        let (stripped, _) = strip_cfg_test_mod_blocks(content);
        if !stripped.contains(needle) {
            missing.push(format!(
                "{rel}: expected surviving line not found: `{needle}`"
            ));
        }
    }
    assert!(
        missing.is_empty(),
        "\nthe #[cfg(test)] mod-block stripper swallowed real code that sits on a non-`mod` \
         attribute target (a `use`/`enum`/`#[path] mod …;`) — these lines must survive \
         stripping verbatim: {missing:?}\n"
    );
}

/// Every row must justify itself: a non-empty note (so this stays a real
/// audit trail, not decoration), and per `Egress::public_name`'s own doc
/// comment, a non-empty name when one is given.
#[test]
fn egress_rows_are_documented() {
    let mut bad = Vec::new();
    for e in EGRESS {
        if e.note.trim().is_empty() {
            bad.push(format!("{}: empty note", e.host));
        }
        if let Some(name) = e.public_name
            && name.trim().is_empty()
        {
            bad.push(format!("{}: empty public_name", e.host));
        }
    }
    assert!(bad.is_empty(), "EGRESS rows missing documentation: {bad:?}");
}

/// Every host in EGRESS is unique — a duplicate row is either dead weight or
/// (worse) two contradictory notes for the same host.
#[test]
fn egress_hosts_are_unique() {
    let mut seen = BTreeSet::new();
    let mut dupes = Vec::new();
    for e in EGRESS {
        if !seen.insert(e.host) {
            dupes.push(e.host);
        }
    }
    assert!(dupes.is_empty(), "duplicate EGRESS host row(s): {dupes:?}");
}
