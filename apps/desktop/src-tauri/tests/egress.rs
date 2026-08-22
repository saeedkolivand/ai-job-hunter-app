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
//! - A host supplied **only via a build-time secret** — `option_env!`/`env!`
//!   reading a value a CI workflow injects into the environment, never
//!   checked into this repo in any form — is a *different* limitation from
//!   the runtime-configurable case above: there, a real literal exists in
//!   source but is a template; here, no literal ever exists at all, in any
//!   build anyone can reproduce locally. [`UNEXTRACTABLE`] documents these by
//!   prose instead, since there is no string for EGRESS-1/2 to check.
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
    /// roles (`www.linkedin.com` has five), so there is deliberately no
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
    Egress { host: "freehire.me", public_name: Some("freehire"), note: "Keyless job board — no API key, so it is the one broad board a fresh install can search with. Selected explicitly in the catalog. scraping/boards/freehire/mod.rs." },
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
    Egress { host: "www.linkedin.com", public_name: None, note: "Five roles: login page (board_login), native board search + geo-typeahead (scraping/boards/linkedin, scraping/linkedin/api_client), the Apify actor's LinkedIn URL construction (aggregator fallback), the single-pasted-URL resolver (scrape_url), and profile import's user-pasted-URL fetch (profile_import/linkedin.rs) — the last two never contribute a literal here; the host is whatever the user pasted, redirect-resolved (see the module doc's runtime-configurable-base caveat)." },
    Egress { host: "secure.indeed.com", public_name: None, note: "Indeed login page, opened for the user to authenticate. scraping/board_login/mod.rs." },
    Egress { host: "login.xing.com", public_name: None, note: "Xing login page, opened for the user to authenticate. scraping/board_login/mod.rs." },
    Egress { host: "www.glassdoor.com", public_name: None, note: "Glassdoor login page, opened for the user to authenticate. scraping/board_login/mod.rs." },

    // ── Location autocomplete (ADR-0005 class 5) — offline-first fallback.
    Egress { host: "photon.komoot.io", public_name: Some("Photon"), note: "Location-autocomplete fallback (OpenStreetMap/ODbL) used only when the bundled offline GeoNames index has no match. commands/geocoding.rs." },

    // ── Profile import (opt-in, user-initiated).
    Egress { host: "api.github.com", public_name: Some("GitHub"), note: "\"Import from GitHub\" resume-builder flow — public, non-fork repos only. profile_import/github.rs." },

    // ── Email-confirmation watching (ADR-0005 class 7) — opt-in, default OFF.
    Egress { host: "imap.gmail.com", public_name: Some("IMAP"), note: "Default (v1 Gmail-branded) IMAP host for opt-in email-confirmation watching; DATA not a hardcoded destination — user-configurable, credential OS-keychain-backed. Scheme-less: email_watch/imap_client.rs::DEFAULT_IMAP_HOST (see SCHEMELESS)." },

    // ── Updater (ADR-0005 class 4) — first check 10s after launch, then every
    // 4h for the rest of the session (updater/mod.rs:314,317), no user data.
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

/// Egress declared by class, not by host string, because no host string
/// exists anywhere in this repo to declare: it arrives via a secret injected
/// only into the signed release build. Deliberately a SEPARATE list from
/// [`EGRESS`], not a row with a placeholder host in it — EGRESS-1 extracts
/// from source text, and EGRESS-2 re-checks every `EGRESS` row is STILL
/// present in source text; a class with no source text at all fits neither
/// direction. Folding it into `EGRESS` would force a choice between two bad
/// outcomes: a fake placeholder host that EGRESS-2 would then fail forever
/// (nothing to find, by design), or leaving the row's `host` field
/// meaningless. A third, explicitly-unchecked list says the true thing:
/// documented, not verified.
struct UnextractableEgress {
    /// Third-party name — every row here is a real SDK talking to a named
    /// vendor, so unlike [`Egress::public_name`] this is never `None`.
    public_name: &'static str,
    /// What it is, why no literal exists to extract, and where the real
    /// answer lives (repo-relative paths, so a reader can go verify by hand
    /// rather than trust this comment).
    note: &'static str,
}

#[rustfmt::skip]
const UNEXTRACTABLE: &[UnextractableEgress] = &[
    // ── Crash reporting (ADR-0005 class 8) — default ON, gated on shown
    // consent (Settings::transmits = enabled && consent_shown).
    UnextractableEgress {
        public_name: "Sentry",
        note: "Crash-report ingest host, carried inside the DSN. The DSN is a GitHub Actions \
               secret (AJH_SENTRY_DSN) injected ONLY by .github/workflows/release.yml's signed \
               release job, read at compile time via build.rs into \
               option_env!(\"AJH_SENTRY_DSN\") (crash_reporting/mod.rs:108) — every local build, \
               every contributor clone, and every non-release CI check compiles with DSN = None \
               and cannot transmit at all, so the host is not merely hard to extract, it does \
               not exist in any reproducible build of this repo. Further gated by opt-out \
               consent (Settings::transmits, crash_reporting/mod.rs) and the wire-gate transport \
               that re-checks consent and drops unredactable envelopes per-send \
               (crash_reporting/transport.rs).",
    },
];

/// Hosts that only ever appear WITHOUT a `scheme://` prefix in real code (a
/// bare comparison/DNS-label const, not a `scheme://host` literal) —
/// EGRESS-1's scheme-anchored extractor can never see these, so EGRESS-2
/// checks them by searching for the quoted literal (`"host"`) in
/// comment-stripped, non-test source instead of extracted-set membership.
///
/// `owning_files` scopes that search to the specific file(s) each host is
/// actually declared in, rather than the whole tree — an un-scoped `contains`
/// over every file joined together is vacuously satisfied by ANY bare
/// occurrence anywhere, including one that has nothing to do with egress.
/// Concretely: `scraping/trust/mod.rs`'s 31-hostname `ATS_ALLOWLIST` (a
/// suffix-match trust-policy list, not an egress declaration) happens to
/// contain 3 of these 7 literals as ordinary data — deleting the real
/// declaring site while that allowlist entry stays would leave an un-scoped
/// check green.
struct Schemeless {
    host: &'static str,
    /// Every file (relative to `src/`) whose real (non-test) code legitimately
    /// declares this bare literal. Present in any one of these is enough.
    owning_files: &'static [&'static str],
}

#[rustfmt::skip]
const SCHEMELESS: &[Schemeless] = &[
    Schemeless { host: "jobs.personio.de", owning_files: &["scraping/boards/personio/mod.rs"] },
    Schemeless { host: "jobs.personio.com", owning_files: &["scraping/boards/personio/mod.rs"] },
    Schemeless { host: "ats.rippling.com", owning_files: &["scraping/ats_ref.rs", "scraping/boards/rippling/mod.rs"] },
    Schemeless { host: "boards.greenhouse.io", owning_files: &["scraping/ats_ref.rs"] },
    Schemeless { host: "job-boards.greenhouse.io", owning_files: &["scraping/ats_ref.rs"] },
    Schemeless { host: "boards.eu.greenhouse.io", owning_files: &["scraping/ats_ref.rs"] },
    Schemeless { host: "imap.gmail.com", owning_files: &["email_watch/imap_client.rs"] },
];

/// Declared dynamic-host sites: a `scheme://` literal whose entire authority
/// is placeholder/format-arg text (no real label survives extraction), keyed
/// on `(path, matched literal)` rather than line number so an edit above the
/// site doesn't churn this table.
const DYNAMIC_SITES: &[(&str, &str)] = &[
    // Both of these BUILD A STRING and never fetch it: `rich.rs` makes a bare
    // URL clickable in the rendered document, and `extraction/pdf.rs` writes a
    // scheme onto a host the CV spelled out without one, for the extractor's
    // own markdown reference list. Neither reaches the network.
    ("model/rich.rs", "https://{url}"),
    ("extraction/pdf.rs", "https://{token}"),
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

/// Does `lines[i]` open a `#[cfg(test)]` … `mod name { … }` region? Returns
/// the line index the `mod … {` line itself sits on. `None` for a
/// `#[cfg(test)]` whose attribute chain lands on anything else (`use`,
/// `const`, `enum`, `fn`, or a `#[path = "…"] mod name;` external-file
/// reference) — those hold no inline test code to leak, and the naive "strip
/// to the next standalone `}`" approach would delete real declarations.
/// Dozens of column-0 `#[cfg(test)]` attributes in this tree sit on something
/// other than a `mod … {` block (e.g. `validate/content/mod.rs:66` on a
/// `use`, `commands/ai_provider/stream.rs:528` on `enum StreamSink {`,
/// `commands/ai_provider/anthropic.rs:1394` on a `#[path] mod tests;`
/// external-file reference) — `egress_cfg_test_stripper_never_swallows_a_non_mod_attribute_target`
/// pins exactly those three as regression cases.
///
/// Only matches at column 0 (`lines[i] == "#[cfg(test)]"` exactly, no
/// indentation, no trailing content) — an indented or otherwise-decorated
/// `#[cfg(test)] mod tests { … }` (nested inside another mod, for example) is
/// silently left un-stripped. Fail-safe today by inspection (every indented
/// `#[cfg(test)]` in this tree sits on a fn or field, never a `mod` block),
/// but if that ever changes, the failure mode is confusing rather than
/// silent: a real host inside the un-stripped fixture would surface as an
/// EGRESS-1 failure whose fix-it message tells a developer to add a TEST
/// fixture host to the real inventory.
fn cfg_test_mod_open(lines: &[&str], i: usize) -> Option<usize> {
    if lines[i] != "#[cfg(test)]" {
        return None;
    }
    // Skip forward past any directly-following attribute lines (e.g.
    // `#[path = "…"]`) to find what the attribute chain lands on.
    let mut j = i + 1;
    while j < lines.len() && lines[j].trim_start().starts_with("#[") {
        j += 1;
    }
    if j >= lines.len() {
        return None;
    }
    let t = lines[j].trim_start();
    let t_end = t.trim_end();
    // Strip an optional `pub`/`pub(...)` visibility prefix, THEN require the
    // next token to be exactly `mod ` — a bare `t.starts_with("pub(")` check
    // (without stripping to the token after it) also matches a
    // `#[cfg(test)]`-annotated test-only HELPER function/impl such as
    // `pub(super) fn row_counts() -> (usize, usize) {`
    // (commands/geocoding/geonames.rs), which is real code, not a module
    // block, and must not be stripped.
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
    (after_vis.starts_with("mod ") && t_end.ends_with('{')).then_some(j)
}

/// Brace-count forward from `open_line` (a `mod … {` line) to find where the
/// naive, string-unaware counter decides the block closes. Naive: counts
/// every `{`/`}` BYTE including ones inside string literals — usually
/// harmless, since a real `mod … { … }` block's own Rust-syntax braces
/// balance regardless, but a string literal that itself contains an
/// UNBALANCED `{`/`}` (a JSON fixture testing corrupt input, or RTF's own
/// `{`/`}` control-word syntax) throws the running count off, either closing
/// the region too early (mid-line, inside a later string) or never closing it
/// at all (running to EOF). See
/// `egress_stripped_regions_close_on_a_column_zero_brace_or_a_known_exception`
/// for the invariant this naivety is checked against, and its three known
/// exceptions.
///
/// Returns the line index the scan stopped on — `lines.len()` if depth never
/// reached <= 0 before the file ended.
fn brace_close_line(lines: &[&str], open_line: usize) -> usize {
    let mut depth = 0i32;
    let mut k = open_line;
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
    k
}

/// Strip every `#[cfg(test)]` … `mod name { … }` region [`cfg_test_mod_open`]
/// finds (real inline test code with its own scheme-qualified fixture URLs)
/// from `content`, using [`brace_close_line`] to find each region's end.
///
/// Returns the content with those regions removed, plus how many regions were
/// stripped (used only as a loose sanity signal, never an exact pin).
fn strip_cfg_test_mod_blocks(content: &str) -> (String, usize) {
    let lines: Vec<&str> = content.lines().collect();
    let mut out: Vec<&str> = Vec::with_capacity(lines.len());
    let mut stripped = 0usize;
    let mut i = 0usize;
    while i < lines.len() {
        if let Some(open_line) = cfg_test_mod_open(&lines, i) {
            let close_line = brace_close_line(&lines, open_line);
            stripped += 1;
            i = close_line + 1;
            continue;
        }
        out.push(lines[i]);
        i += 1;
    }
    (out.join("\n"), stripped)
}

/// For every `#[cfg(test)] mod … { … }` region [`cfg_test_mod_open`] finds in
/// `content`, whether [`brace_close_line`] closed it on a column-0 `}` — the
/// shape every real `mod … { … }` block closes with under this repo's
/// rustfmt config (a top-level item's closing brace is never indented).
/// `false` covers both known failure shapes of the naive counter: a mid-line
/// close (a brace inside a string decremented the count early, so real code
/// AFTER that point was not stripped and got scanned as if it were
/// production source) and a run to EOF (a brace inside a string was never
/// balanced, so the count never returned to zero and the "region" swallowed
/// the rest of the file). Used only by
/// `egress_stripped_regions_close_on_a_column_zero_brace_or_a_known_exception`
/// below — a diagnostic twin of the stripper, not part of the extraction
/// pipeline, so nothing about what gets stripped changes by its existing.
fn region_closes_on_column_zero_brace(content: &str) -> Vec<bool> {
    let lines: Vec<&str> = content.lines().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < lines.len() {
        if let Some(open_line) = cfg_test_mod_open(&lines, i) {
            let close_line = brace_close_line(&lines, open_line);
            let clean = close_line < lines.len() && lines[close_line].starts_with('}');
            out.push(clean);
            i = close_line + 1;
            continue;
        }
        i += 1;
    }
    out
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
///
/// Matched case-insensitively — schemes are case-insensitive per RFC 3986
/// §3.1, and browsers/curl/reqwest all accept `HTTPS://…`, so a literal
/// spelled that way must be exactly as visible here as a lowercase one.
/// Compared on bytes rather than `str` slices so this can never panic on a
/// char-boundary: a matching ASCII byte run can only ever land on codepoint
/// boundaries (a UTF-8 continuation byte is never a valid ASCII byte value).
fn scheme_start(content: &str, pos: usize) -> Option<usize> {
    let bytes = content.as_bytes();
    if pos >= 1 && bytes[pos - 1] == b'*' {
        let start = pos - 1;
        if start == 0 || !bytes[start - 1].is_ascii_alphanumeric() {
            return Some(start);
        }
    }
    for w in SCHEME_WORDS {
        let wl = w.len();
        if pos >= wl && bytes[pos - wl..pos].eq_ignore_ascii_case(w.as_bytes()) {
            let start = pos - wl;
            if start == 0 || !bytes[start - 1].is_ascii_alphanumeric() {
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

#[cfg(test)]
mod scan_one_case_sensitivity {
    use super::scan_one;

    /// RFC 3986 §3.1 schemes are case-insensitive, and every real HTTP client
    /// (browsers, curl, reqwest) accepts `HTTPS://…` — a literal spelled that
    /// way must be exactly as visible to the extractor as a lowercase one, or
    /// a real host behind it silently evades EGRESS-1.
    #[test]
    fn recognizes_an_uppercase_scheme() {
        let mut hosts = super::HostSites::new();
        let mut dynamic = super::DynamicSites::new();
        scan_one(
            "probe.rs",
            "let u = \"HTTPS://api.acme-corp.io/x\";",
            &mut hosts,
            &mut dynamic,
        );
        assert!(
            hosts.contains_key("api.acme-corp.io"),
            "an uppercase HTTPS:// scheme must be recognized; got {hosts:?}"
        );
    }

    /// Mixed case must be caught too, not just all-uppercase.
    #[test]
    fn recognizes_a_mixed_case_scheme() {
        let mut hosts = super::HostSites::new();
        let mut dynamic = super::DynamicSites::new();
        scan_one(
            "probe.rs",
            "let u = \"HttpS://api.acme-corp.io/x\";",
            &mut hosts,
            &mut dynamic,
        );
        assert!(hosts.contains_key("api.acme-corp.io"), "got {hosts:?}");
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
        "\nTo fix: add an `Egress { host, public_name, note }` row for each host above to \
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
    // bare-literal search, keyed by file — NOT one blob joined across the
    // whole tree (vacuously satisfied by ANY bare occurrence anywhere, e.g.
    // the 31-hostname `ATS_ALLOWLIST` policy list at
    // scraping/trust/mod.rs:76-108, a suffix-match allowlist, not an egress
    // declaration, that happens to contain 3 of the 7 SCHEMELESS literals as
    // ordinary data) and NOT a raw `contains` over unstripped file text
    // (vacuously satisfied by a stale doc-comment mention). Each
    // [`Schemeless`] row names its own `owning_files`; the search below is
    // scoped to exactly those.
    let schemeless_by_file: BTreeMap<String, String> = rust_sources()
        .into_iter()
        .map(|f| {
            let (no_mods, _) = strip_cfg_test_mod_blocks(&f.content);
            (f.rel, strip_comment_lines(&no_mods))
        })
        .collect();

    let mut dead: Vec<&str> = Vec::new();
    for e in EGRESS {
        let present = if let Some(sl) = SCHEMELESS.iter().find(|s| s.host == e.host) {
            sl.owning_files.iter().any(|file| {
                schemeless_by_file
                    .get(*file)
                    .is_some_and(|content| content.contains(&format!("\"{}\"", e.host)))
            })
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

/// A CSP/manifest source that is a bare scheme (`https:` with no `//…`), a
/// scheme-wildcard host (`*://*/*`, `<all_urls>`), OR a **fixed-scheme**
/// wildcard host (`https://*`, `https://*/*`) defeats the whole point of an
/// enumerated inventory — pinning the scheme doesn't narrow which host may
/// answer, so all three are equally ANY-host. Deliberately separate from the
/// extractor (which only recognizes `scheme://host` literals as *declarable*
/// hosts) rather than folding this into the authority-extraction rule — this
/// function's job is to REJECT a wildcard, not record it as a dynamic site:
/// `EGRESS-3`'s `DYNAMIC_SITES` is for a legitimate single templated URL
/// (`https://{host}`, filled with one real value at runtime), not a CSP glob
/// that admits every host in existence.
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
            } else {
                // Fixed-scheme wildcard host: the same "no real label
                // survives `*`-filtering" test EGRESS-1's extractor uses to
                // recognize a dynamic site, reused here to recognize an
                // any-host CSP/MV3 source instead (`https://*`,
                // `https://*/*`, but not `https://*.example.com/*`, which
                // narrows to one real domain).
                let auth_start = after + 2;
                let auth_end = authority_end(content.as_bytes(), auth_start);
                let raw_auth = &content[auth_start..auth_end];
                if matches!(process_authority(raw_auth), Some(None)) {
                    hits.push(format!("wildcard host pattern `{scheme}//{raw_auth}`"));
                }
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
        // Comment-strip first, mirroring extract_from_decl_files's pipeline —
        // otherwise a comment merely mentioning a bare scheme (e.g. `// see
        // http: handling`) fails this gate with no real CSP/manifest source
        // behind it.
        let clean = strip_comment_lines(content);
        for hit in find_bare_scheme_or_wildcard(&clean) {
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

    /// The gap the wildcard-scheme-only check (`*://*/*`) missed: a FIXED
    /// scheme with a wildcard host is just as much "any host" — CSP's own
    /// syntax for it.
    #[test]
    fn flags_a_fixed_scheme_csp_wildcard_host() {
        let hits = find_bare_scheme_or_wildcard("connect-src 'self' https://*");
        assert_eq!(
            hits.len(),
            1,
            "a fixed-scheme any-host CSP source must be flagged; got {hits:?}"
        );
    }

    /// Same gap, MV3's `host_permissions` spelling.
    #[test]
    fn flags_a_fixed_scheme_mv3_wildcard_host() {
        let hits = find_bare_scheme_or_wildcard(r#"host_permissions: ["https://*/*"]"#);
        assert_eq!(hits.len(), 1, "got {hits:?}");
    }

    /// A wildcard SUBDOMAIN narrows to one real domain — not an any-host
    /// source, and must not be flagged (it's a normal declarable host, caught
    /// by the ordinary extractor instead).
    #[test]
    fn does_not_flag_a_narrowed_wildcard_subdomain() {
        let hits = find_bare_scheme_or_wildcard("connect-src 'self' https://*.example.com/*");
        assert!(
            hits.is_empty(),
            "a wildcard SUBDOMAIN of a real domain must not be flagged; got {hits:?}"
        );
    }

    /// Regression guard for the EGRESS-4 cry-wolf gap: unlike EGRESS-1/2
    /// (`extract_from_decl_files`), the test that calls this function used to
    /// scan the RAW, unstripped declaration-file text, so a comment merely
    /// mentioning a bare scheme (`// see http: handling`) failed the gate
    /// with no real CSP/manifest source behind it. Mutation-checked: with the
    /// call site's `strip_comment_lines` reverted, the first assertion below
    /// (against the STRIPPED text) goes red because the raw scanner still
    /// finds the comment; restoring it goes green again. The second assertion
    /// pins that the raw scanner genuinely does still flag the same text
    /// un-stripped — proving the fix is the stripping step at the call site,
    /// not a change that made `find_bare_scheme_or_wildcard` itself blind to
    /// real hits.
    #[test]
    fn ignores_a_bare_scheme_mentioned_only_in_a_comment_line() {
        let raw = "// see http: handling\nconnect-src 'self' https://api.example.org";

        let clean = super::strip_comment_lines(raw);
        let hits = find_bare_scheme_or_wildcard(&clean);
        assert!(
            hits.is_empty(),
            "a bare scheme mentioned only inside a comment line must not trip EGRESS-4 once \
             comments are stripped (mirroring the EGRESS-1/2 pipeline); got {hits:?}"
        );

        let raw_hits = find_bare_scheme_or_wildcard(raw);
        assert!(
            !raw_hits.is_empty(),
            "sanity check failed: the raw (unstripped) text should still be flagged directly by \
             the scanner — if this is empty too, the comment-false-positive guard above is \
             untested"
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

/// Pins the real invariant the naive brace counter should be judged against —
/// every stripped `#[cfg(test)] mod … { … }` region closes on a column-0
/// `}`, never mid-line and never at EOF — rather than the false claim this
/// module's doc comment used to carry ("every test fixture in this tree with
/// embedded braces is itself balanced, verified empirically"). It was not: 3
/// of 95 regions in this tree mis-resolve, each from an unbalanced-brace
/// STRING LITERAL the naive counter cannot see past, listed here by name
/// rather than silently excluded from the count:
///
///   - `crash_reporting/mod.rs` — `"{ not json"` (a deliberately-invalid JSON
///     fixture) has one embedded `{` and no matching `}`, offsetting the
///     trailing `mod tests { … }` block's count by +1 for the rest of the
///     file, so it runs to EOF.
///   - `extraction/rtf.rs` — RTF's own `{`/`}` control-word syntax inside
///     string fixtures (e.g. `"{\\rtf1\\ansi …}"`) offsets the count, so its
///     `mod tests { … }` block also runs to EOF.
///   - `pipeline/json.rs` — `r#"{"note":"a \" then }","a":1}"#` has a `}`
///     INSIDE the quoted string value, which the naive counter reads as a
///     real closing brace and stops on, two statements before the block's
///     real end.
///
/// None of these hides a real host TODAY — verified by hand against the
/// extracted set, and structurally impossible for the first two, since a
/// region that "closes" at EOF has nothing after it to hide. `pipeline/json.rs`
/// is the one case where a real host COULD have been hidden in principle (the
/// truncation leaves real lines unstripped, and those lines can and do
/// contain scheme-qualified fixture URLs elsewhere in that same test module)
/// — none of them are non-reserved hosts today, but this is exactly the
/// failure mode the module doc's "What this test does NOT prove" section
/// already names as a limitation.
///
/// A NEW file failing this check is not automatically safe the same way —
/// investigate before adding it to `KNOWN_EXCEPTIONS`.
#[test]
fn egress_stripped_regions_close_on_a_column_zero_brace_or_a_known_exception() {
    const KNOWN_EXCEPTIONS: &[&str] = &[
        "crash_reporting/mod.rs",
        "extraction/rtf.rs",
        "pipeline/json.rs",
    ];
    let mut bad = Vec::new();
    for f in rust_sources() {
        if !f.content.contains("#[cfg(test)]") {
            continue;
        }
        let all_clean = region_closes_on_column_zero_brace(&f.content)
            .into_iter()
            .all(|ok| ok);
        if !all_clean && !KNOWN_EXCEPTIONS.contains(&f.rel.as_str()) {
            bad.push(f.rel);
        }
    }
    assert!(
        bad.is_empty(),
        "\na #[cfg(test)] mod region in a NEW file does not close on a column-0 `}}` (mid-line \
         close or ran to EOF) — investigate whether a real host could be hidden past the \
         truncation/overrun (mid-line close is the dangerous direction: it leaves real code \
         un-stripped), then either fix the fixture's brace balance or add the file to \
         KNOWN_EXCEPTIONS in apps/desktop/src-tauri/tests/egress.rs with a reason: {bad:?}\n"
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

/// [`UNEXTRACTABLE`]'s equivalent of `egress_rows_are_documented` — every row
/// must have a real vendor name and a non-empty note, the same audit-trail
/// contract as [`EGRESS`].
#[test]
fn egress_unextractable_rows_are_documented() {
    let mut bad = Vec::new();
    for e in UNEXTRACTABLE {
        if e.public_name.trim().is_empty() {
            bad.push("(unnamed row): empty public_name".to_string());
        }
        if e.note.trim().is_empty() {
            bad.push(format!("{}: empty note", e.public_name));
        }
    }
    assert!(
        bad.is_empty(),
        "UNEXTRACTABLE rows missing documentation: {bad:?}"
    );
}

/// Regression guard for the HIGH this row exists to fix: ADR-0005 class 8
/// (crash reporting) had zero inventory coverage despite being the only
/// default-ON automatic egress in the product. Deliberately independent of
/// [`egress_unextractable_rows_are_documented`] (a shape check that would
/// stay green even if every row were deleted) — this asserts the SPECIFIC
/// row exists, so deleting it is what actually goes red. Mutation-checked:
/// deleting the Sentry row from `UNEXTRACTABLE` turns this test red while
/// every other egress test (including `egress_2_every_declared_host_is_still_in_source`)
/// stays exactly as it was — proof `UNEXTRACTABLE` cannot influence EGRESS-2,
/// since EGRESS-2 iterates `EGRESS` only and never reads this const.
#[test]
fn egress_declares_the_sentry_crash_reporting_class() {
    assert!(
        UNEXTRACTABLE.iter().any(|e| e.public_name == "Sentry"),
        "ADR-0005 class 8 (crash reporting) must have a declared-but-unextractable row in \
         UNEXTRACTABLE — see the module doc's build-time-secret bullet for why it cannot be a \
         normal EGRESS row"
    );
}
