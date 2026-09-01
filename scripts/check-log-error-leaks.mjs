// Drift guard: no bare interpolation of a caught error — captured (`{e}`,
// `{err}`, `{error}`) or positional (`"...{}...", e`) — inside a
// `log::{warn,error,info,debug}!` call under apps/desktop/src-tauri/src.
//
// ── The defect this exists to stop ──────────────────────────────────────────
//
// AGENTS.md's path-privacy rule says an absolute path / username / home dir
// must never appear anywhere, logs explicitly included. The crate violated it
// via a recurring shape:
//
//   log::warn!("[setup] ... failed: {e}")
//
// When `e` is (or wraps) a `rusqlite::Error::InvalidPath`, a filesystem
// `std::io::Error`, or a `reqwest::Error`, its `Display` can embed the
// offending absolute path or full request URL — so this writes the user's
// home directory, or a credential-bearing query string, straight into a log
// file that support bundles ship verbatim. `apps/desktop/src-tauri/src/
// applications/mod.rs` (see the comment on `ApplicationStore::open`) is the
// canonical fix: `.code()` for a fixed, path-free category, or
// `observability::sanitize_reason(&e.to_string())` when the rest of the
// message is worth keeping.
//
// ── What this checks ─────────────────────────────────────────────────────────
//
// Every `log::{warn,error,info,debug}!(...)` call that interpolates a bound
// error identifier — captured in the format string (`{e}`, `{err}`,
// `{error}`, Rust 2021 format-string capture) or passed as a bare positional
// argument (`"...{}...", e`). Bounded to those three binding names because
// they are the only ones this codebase's ~264 log call sites ever use for a
// caught error — see `ERROR_BINDING_NAMES` for the measurement. A caught
// error re-bound to some OTHER name before logging is still a blind spot; no
// static scan without real type information can rule that out, and
// introducing one here would itself be a conspicuous, reviewable departure
// from every existing call site.
//
// NOT "no site may ever print an error" — an error whose `Display`
// structurally cannot carry a path/URL/host/credential (a pure JSON/PDF
// parse failure, a fixed-string domain error) loses real diagnostic value for
// nothing if it is forced through `.code()`. This enforces that the question
// was ANSWERED: every surviving site is declared in ALLOWLIST with a
// one-line reason, tagged `safe` (provably cannot leak) or `debt` (a real
// leak candidate this pass did not fix — see each entry). A newly introduced
// site fails until someone writes that sentence, mirroring
// check-event-subscriptions.mjs.
//
// Both directions are checked, so the list cannot rot: an undeclared site
// fails, and so does an ALLOWLIST entry that no longer corresponds to a real
// site (fixed, moved, or deleted) — see `violations()`.
//
// `scraping/**` was originally left as 45 `debt` entries (scraping-applier's
// domain, AGENTS.md / CLAUDE.md domain routing, out of the first pass's
// primary-path scope). scraping-applier-author triaged all 45 by tracing each
// `{e}`'s concrete type: 12 were genuine leaks (a `reqwest::Error` NOT routed
// through `scraping::http`'s `.without_url()` chokepoint, a keyring error, or
// a filesystem path op) and are now fixed with
// `observability::sanitize_reason(&e.to_string())`; the remaining 33 are
// declared `safe` — either the error crosses `scraping::http::fetch_text`/
// `fetch_json` (the fleet chokepoint that already strips the request URL
// before wrapping in `AppError::Network`, so the Adzuna/JSearch/Jooble/Apify
// API key in the query string never reaches the log), or it is a pure
// in-memory parse/task failure that never had a path/URL to begin with. Zero
// `debt` entries remain; the tag stays available (see `status` below) for a
// future pass that needs to record a real leak it isn't fixing yet.
//
// A later review (of #1036) found the detector only matched the literal
// `{e}`, so `{err}` and a bare positional `e` bypassed it entirely — three
// live, undeclared sites: `postings/mod.rs:382` and
// `platform/linux_appimage.rs:170` (`{err}`), and `autopilot_helpers/mod.rs:151`
// (positional `err`). `findLeaks` was widened to catch all three shapes.
// `postings/mod.rs:382` and `platform/linux_appimage.rs:170` were traced and
// declared `safe` — an in-memory serde_json parse and a `Command::exec()`
// `io::Error` whose Display never embeds the program path, respectively.
//
// `autopilot_helpers/mod.rs:151` was NOT declared safe by tracing: a first
// pass argued it bottoms out at the fetch_text/fetch_json chokepoint because
// every registered scraper reports `ScraperMode::Http` — but `ScraperMode` is
// declared transport, not enforced transport, and `LinkedInScraper` (also
// `Http`) reaches the network via `LinkedInHttpClient`'s own
// `reqwest::Client::send()`, bypassing the chokepoint. The exemption was
// therefore certifying a LIVE leak: a first-page LinkedIn search failure
// carries the user's own free-text keywords and location in the URL that
// `From<reqwest::Error> for AppError` never strips. That three-hop argument
// was unsound, so the site is sanitized directly with
// `observability::sanitize_reason` instead and carries no ALLOWLIST entry —
// see the call site.
//
// A follow-up review of that same PR found the positional check still only
// matched a BARE identifier, missing the equivalent one method call away —
// `log::warn!("...: {}", e.to_string())`. `findPositionalErrorArg` was
// widened to also flag `<binding>.to_string()`/`.to_owned()`. Measured
// against every real log call site before widening: zero currently use
// either shape positionally (every `.to_string()` near a `log::` call is
// already the sanctioned `sanitize_reason(&e.to_string())` wrapper, which
// still does not match — see the comment on `positionalErrorRe`), so this
// arm is a tripwire, not a fix for a live leak.
//
// A pre-PR review (2026-08-19) found the ALLOWLIST's `"<path>:<line>"` key is
// its only identity, so ANY unrelated edit that inserts a line above a
// declared site breaks the guard both ways at once (undeclared at the new
// line, stale at the old one) — it happened to `documents/mod.rs`'s
// `charge_provider_daily` site the same day it was written. An entry can
// opt into content-anchored matching by adding `sig` (copied from the `sig`
// `findLeaks` reports for that site — its own format-string literal,
// whitespace-collapsed), which survives a line shift as long as it stays the
// ONLY declared entry with that (file, sig) pair — see `violations`. That
// first pass deliberately kept `sig` opt-in rather than migrating every
// existing entry at once, reasoning a mass-migration was a bigger, separately
// reviewable change from fixing the one break in hand.
//
// That deferral turned out to be wrong: the same shape re-broke three more
// times on this branch alone, twice on `extension_bridge/mod.rs` and once on
// `extension_bridge/register.rs` — exactly the entries the first pass hadn't
// migrated. `sig` is now backfilled onto every ALLOWLIST entry, including the
// `httpChokepointSafe`/`inMemoryParseSafe` factories (both now take `sig` as
// a parameter and thread it into the returned entry, one factory call per
// site so each site still keeps its own independent declaration). Generated
// mechanically from `findLeaks()`'s own `sig` output rather than
// hand-transcribed, so it cannot drift from the real source text it was
// copied from. `sig` stays a per-entry opt-in field rather than a hard
// requirement enforced by `violations()` — the line-exact-only path is real,
// tested behavior (see check-log-error-leaks.test.mjs's `violations` suite),
// not dead code to delete — but every NEW entry should still add one; a bare
// `{ status, reason }` entry works but is one more line insertion away from
// this exact recurrence.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const SRC_REL = 'apps/desktop/src-tauri/src';

/**
 * Every surviving `{e}` site, keyed `"<path relative to src/>:<line>"` (the
 * line the literal `{e}` text sits on — for a multi-line format string that
 * is not necessarily the `log::` call's own line). The line in the key is a
 * human-readable pointer, not the sole identity: every entry below also
 * carries `sig` (see the file header), which is what actually survives the
 * site moving to a different line.
 *
 * `status: 'safe'` — the error's `Display` structurally cannot carry a
 *   path/URL/host/credential; `.code()`/`sanitize_reason` would only lose
 *   diagnostic value. `status: 'debt'` — a likely-real leak, out of this
 *   pass's scope, tracked for a follow-up fix (see the file header).
 */
const ALLOWLIST = {
  // ── safe: provably cannot carry a path/URL/host/credential ────────────────
  'autopilot/mod.rs:817': {
    status: 'safe',
    reason:
      "serde_json::from_value type-mismatch parsing the app's own Autopilot " +
      'record on the load path — a pure parse failure, never a path/URL.',
    sig: '[autopilot] dropping unparseable record: {e}',
  },
  'autopilot_helpers/mod.rs:399': {
    status: 'safe',
    reason:
      "limits::Limiter::charge_provider_daily's AppError::RateLimited message is a " +
      'fixed, author-written template naming only the provider id and a static ' +
      'daily-ceiling number — see limits/mod.rs.',
    sig: '[autopilot] AI notes stopped at daily ceiling: {e}',
  },
  'documents/embedding.rs:153': {
    status: 'safe',
    reason: 'Same charge_provider_daily fixed-template message as autopilot_helpers/mod.rs:395.',
    // `sig` opts this entry into content-anchored matching (see `violations`)
    // so the NEXT unrelated edit that shifts this line doesn't repeat the
    // 2026-08-19 break (a 23-line insertion above it moved 1153 -> 1176 and
    // failed both directions: undeclared-at-1176, stale-at-1153). Note `sig`
    // only survives a LINE shift within the same file, not a file move — this
    // entry's key itself moved to documents/embedding.rs the same day, when
    // the site was extracted out of documents/mod.rs for R8's LOC cap.
    // Copied verbatim from the call site's own format-string literal.
    sig: '[embed] round-trip refused by the daily ceiling: {e}',
  },
  'validate/mod.rs:401': {
    status: 'safe',
    reason:
      'lopdf::Document::load_mem parses IN-MEMORY bytes, not a file path — its errors ' +
      'are fixed format-parse messages ("trailer not found", …), never a path.',
    sig: 'export: lopdf failed to parse rendered PDF bytes: {e}',
  },
  'profile_import/linkedin.rs:134': {
    status: 'safe',
    reason:
      "net::http::read_text_capped's error path already calls reqwest::Error::" +
      'without_url() before wrapping in AppError::Network (see accumulate_capped in ' +
      'net/http.rs) — the URL is stripped upstream of this call site.',
    sig: '[profile_import] linkedin body read failed: has_session={has_session} error={e}',
  },
  'postings/mod.rs:447': {
    status: 'safe',
    reason:
      "err: &serde_json::Error is back_up_corrupt_file's parameter — the from_str parse " +
      'failure of interactions.json content already read into memory (map_mut). Same ' +
      'in-memory-parse shape as inMemoryParseSafe() below, never a path/URL/host/credential.',
    sig: '[postings] interactions.json failed to parse ({err}); \\ backed_up={renamed} backup_name={}',
  },
  'postings/mod.rs:489': {
    status: 'safe',
    reason:
      "serde_json::to_string_pretty is a SERIALIZE error on the app's own " +
      'InteractionRecord — a type-system failure, not an echo of file content.',
    sig: '[postings] save skipped: could not serialize {} interaction(s): {e}',
  },
  'notifications/mod.rs:231': {
    status: 'safe',
    reason: 'Same serde_json serialize-error shape as postings/mod.rs:427 (AppNotification).',
    sig: 'failed to serialize notifications: {e}',
  },
  'commands/autopilot/rerank.rs:278': {
    status: 'safe',
    reason: 'Same charge_provider_daily fixed-template message as autopilot_helpers/mod.rs:395.',
    sig: '[autopilot] semantic re-rank stopped at daily ceiling: {e}',
  },
  'commands/profile_import.rs:20': {
    status: 'safe',
    reason:
      "import_from_url's only branch (linkedin::import) already collapses every " +
      "failure to a fixed AppError::Network string ('could not reach linkedin' / " +
      "'could not read the linkedin response') before returning — no URL/host " +
      'reaches this call site.',
    sig: '[profile_import] import failed: host={host} error={e}',
  },
  'extension_bridge/register.rs:138': {
    status: 'safe',
    reason:
      'std::env::current_exe() takes no input path to leak — a failure here is a ' +
      'generic OS resource-resolution error.',
    sig: '[native_host] current_exe() failed (non-fatal): {e}',
  },
  'extension_bridge/native_host.rs:50': {
    status: 'safe',
    reason:
      'tokio::runtime::Builder::build() failure is a generic OS-thread/resource error; ' +
      'building a runtime touches no filesystem path.',
    sig: '[native_host] failed to build runtime: {e}',
  },
  'extension_bridge/mod.rs:510': {
    status: 'safe',
    reason:
      'TcpListener::accept() failure is a local socket-resource error (e.g. EMFILE); it ' +
      'carries no peer address or path — the peer is a separate, unread `_peer` binding.',
    sig: '[extension_bridge] accept error (continuing): {e}',
  },
  'platform/linux_appimage.rs:170': {
    status: 'safe',
    reason:
      'err is the std::io::Error Command::exec() returns when execve() fails — its ' +
      'Display is only the OS errno message (e.g. "No such file or directory (os error ' +
      '2)"); Rust does not embed the failed program path in that error, so `exe`\'s ' +
      'absolute path never reaches this log line.',
    sig: '[startup] AppImage Wayland LD_PRELOAD re-exec failed, continuing without preload: {err}',
  },

  // ── scraping/**: fetch_text/fetch_json chokepoint already strips the URL ──
  // Every site below calls (directly, or one anyhow-wrapped hop away via the
  // Adzuna/JSearch/Jooble/Apify providers) `scraping::http::fetch_text` /
  // `fetch_json`. Its transport-failure branch calls `reqwest::Error::
  // without_url()` before wrapping in `AppError::Network` (see `fetch_text`'s
  // `Err(e) =>` arm in scraping/http/mod.rs), and every other error variant it
  // can return (`AppError::Provider("HTTP <status>")`,
  // `AppError::Validation("Response too large")`,
  // `AppError::Parse(<generic schema-drift string>)`, `AppError::Cancelled`)
  // is a fixed, path/URL-free message. So the request URL — which for Adzuna/
  // JSearch/Jooble/Apify carries an API key in the query string — never
  // reaches these log lines even on failure. Traced individually against the
  // current source, not pattern-matched.
  'scraping/boards/greenhouse/mod.rs:110': httpChokepointSafe(
    "[greenhouse] fetch failed for '{}': {e}"
  ),
  'scraping/boards/breezy/mod.rs:298': httpChokepointSafe("[breezy] fetch failed for '{}': {e}"),
  'scraping/boards/ycombinator/mod.rs:109': httpChokepointSafe(
    '[ycombinator] item {id} failed: {e}; skipping'
  ),
  'scraping/boards/aggregator/adzuna.rs:429': httpChokepointSafe(
    '[aggregator] adzuna broaden retry failed, keeping narrow result: {e}'
  ),
  'scraping/boards/aggregator/adzuna.rs:562': httpChokepointSafe(
    '[aggregator] adzuna page {page} failed, keeping {} result(s) already collected: {e}'
  ),
  'scraping/boards/arbeitnow/mod.rs:96': httpChokepointSafe(
    '[arbeitnow] page {page} failed: {e}; returning {} collected'
  ),
  'scraping/boards/bamboohr/mod.rs:244': httpChokepointSafe(
    "[bamboohr] fetch failed for '{}': {e}"
  ),
  'scraping/boards/arbeitsagentur/mod.rs:164': httpChokepointSafe(
    '[arbeitsagentur] page {page} failed: {e}; returning {} collected'
  ),
  'scraping/boards/aggregator/mod.rs:318': httpChokepointSafe(
    '[aggregator] adzuna error, attempting jsearch fallback: {e}'
  ),
  'scraping/boards/aggregator/mod.rs:352': httpChokepointSafe(
    '[aggregator] jsearch error, attempting jooble fallback: {e}'
  ),
  'scraping/boards/aggregator/mod.rs:385': httpChokepointSafe(
    '[aggregator] jooble fallback failed: {e}'
  ),
  'scraping/boards/aggregator/mod.rs:623': httpChokepointSafe(
    '[aggregator] apify_linkedin error (additive, ignored): {e}'
  ),
  'scraping/boards/ashby/mod.rs:157': httpChokepointSafe("[ashby] fetch failed for '{}': {e}"),
  'scraping/boards/workable/mod.rs:304': httpChokepointSafe(
    "[workable] fetch failed for '{}': {e}"
  ),
  'scraping/boards/lever/mod.rs:154': httpChokepointSafe("[lever] fetch failed for '{}': {e}"),
  'scraping/boards/pinpoint/mod.rs:192': httpChokepointSafe(
    "[pinpoint] fetch failed for '{}': {e}"
  ),
  'scraping/boards/smartrecruiters/mod.rs:237': httpChokepointSafe(
    "[smartrecruiters] list fetch failed for '{}': {e}"
  ),
  'scraping/boards/smartrecruiters/mod.rs:275': httpChokepointSafe(
    '[smartrecruiters] detail fetch failed for posting {} ({detail_url}): {e}; skipping'
  ),
  'scraping/boards/themuse/mod.rs:193': httpChokepointSafe(
    '[themuse] page {page} failed: {e}; returning {} collected'
  ),
  'scraping/boards/recruitee/mod.rs:157': httpChokepointSafe(
    "[recruitee] fetch failed for '{}': {e}"
  ),
  'scraping/boards/rippling/mod.rs:230': httpChokepointSafe(
    "[rippling] fetch failed for '{}': {e}"
  ),
  'scraping/boards/personio/mod.rs:208': httpChokepointSafe(
    "[personio] fetch failed for '{}' via {}: {e}"
  ),

  // ── scraping/**: pure in-memory parse of already-fetched data ─────────────
  // `serde_json::from_value`/`from_str` on a JSON value/body already read into
  // memory — Display is a schema-mismatch message ("missing field `x`",
  // "invalid type: …"), structurally incapable of carrying a path/URL/host/
  // credential.
  'scraping/boards/breezy/mod.rs:102': inMemoryParseSafe('[breezy] skipping malformed row: {e}'),
  'scraping/boards/jobicy/mod.rs:66': inMemoryParseSafe('[jobicy] skipping malformed row: {e}'),
  'scraping/boards/jobicy/mod.rs:171': inMemoryParseSafe(
    '[jobicy] response parse failure (HTTP {status_code}): {e}'
  ),
  'scraping/boards/comeet/mod.rs:76': inMemoryParseSafe('[comeet] skipping malformed row: {e}'),
  'scraping/boards/rippling/mod.rs:89': inMemoryParseSafe('[rippling] skipping malformed row: {e}'),
  'scraping/boards/workable/mod.rs:118': inMemoryParseSafe(
    '[workable] skipping malformed row: {e}'
  ),
  'scraping/http/mod.rs:332': inMemoryParseSafe(
    '[scraping::http] fetch_json parse failure for {safe_url} ({e}); body_len={}'
  ),

  'scraping/http/mod.rs:442': {
    status: 'safe',
    reason:
      "htmd::convert parses an already-fetched, in-memory HTML string (html_to_markdown's " +
      'own fallback path) — its Display is a structural markdown-conversion failure, never ' +
      'a path/URL/host/credential.',
    sig: '[scraping::http] htmd conversion failed ({e}); falling back to html_to_text',
  },
  'scraping/engine/mod.rs:1198': {
    status: 'safe',
    reason:
      'tokio::task::JoinError from the record_health spawn_blocking handle (the task ' +
      'panicked or was cancelled) — its Display names only the task id and panic/cancel ' +
      'state, never a path/URL/host/credential. Distinct from the sibling arm 3 lines ' +
      'above, which IS a path-carrying AppError from the store and already uses .code() ' +
      'for exactly that reason.',
    sig: '[scrape] board-health record task failed: {e}',
  },
};

/** Factory for the fetch_text/fetch_json-chokepoint shape — see the comment
 * above the first entry that uses it for the full rationale; kept as one
 * factory so the reasoning is written once and every site stays independently
 * declared (the undeclared/stale checks below still cover each individually).
 * `sig` is the call site's own format-string literal (copied from `findLeaks`'
 * output for it — see `normalizeSig`), threaded straight through so every
 * factory-built entry gets content-anchored matching too, same as a plain
 * entry that declares `sig` inline. */
function httpChokepointSafe(sig) {
  return {
    status: 'safe',
    reason:
      'Every error here originates from scraping::http::fetch_text/fetch_json (or, for ' +
      'the Adzuna/JSearch/Jooble/Apify providers, a thin anyhow wrapper one hop away from ' +
      "the same call) — the fleet's HTTP chokepoint for named-board scrapers, whose " +
      'transport-failure branch already calls reqwest::Error::without_url() before ' +
      'wrapping in AppError::Network, and whose other error variants (HTTP-status, ' +
      'body-too-large, generic schema-drift, cancelled) are fixed path/URL-free messages.',
    sig,
  };
}

/** Factory for the pure in-memory-parse shape — see the comment above the
 * first entry that uses it. `sig` — see `httpChokepointSafe` above. */
function inMemoryParseSafe(sig) {
  return {
    status: 'safe',
    reason:
      'serde_json parsing an already-fetched, in-memory JSON value/body — a pure schema-' +
      'mismatch failure, never a path, URL, host, or credential.',
    sig,
  };
}

/** Minimum reason length for an entry to count as an explanation, not a stub. */
const MIN_REASON_CHARS = 20;

/** Recursively list `.rs` files under `dir`. */
function rustFiles(dir) {
  const out = [];
  for (const name of readdirSync(dir)) {
    const full = join(dir, name);
    if (statSync(full).isDirectory()) {
      out.push(...rustFiles(full));
      continue;
    }
    if (name.endsWith('.rs')) out.push(full);
  }
  return out;
}

/** Normalize a relative path to POSIX separators, so keys are stable on Windows. */
const toPosix = (rel) => rel.split(sep).join('/');

/** 1-based line number of byte offset `at` in `text`. */
function lineOf(text, at) {
  let line = 1;
  for (let i = 0; i < at; i++) {
    if (text[i] === '\n') line++;
  }
  return line;
}

/**
 * The identifiers this codebase's log call sites use for a caught/bound
 * error when interpolating it directly — captured (`{e}`/`{err}`/`{error}`)
 * or positional (`"...{}...", e`), the latter optionally `&`/`*`-prefixed.
 *
 * Bounded to these three rather than "any identifier": a scan of every
 * `{ident}` capture across all ~264 log call sites in the crate found `e`
 * (45 sites) and `err` (2 sites) as the only ones naming an error — every
 * other captured identifier (`host`, `port`, `board_id`, `reason`, `page`, …)
 * is an ordinary data field. Matching on any identifier would flag every one
 * of those too, drowning the real leak candidates in noise on normal log
 * content. `error` is included for the same convention `err` was, even
 * though nothing currently spells it that way.
 */
const ERROR_BINDING_NAMES = ['e', 'err', 'error'];
const capturedErrorRe = new RegExp(`\\{(?:${ERROR_BINDING_NAMES.join('|')})\\}`);
/**
 * A bare error binding (`e`, `&err`, …), or the same binding stringified —
 * `e.to_string()` / `err.to_owned()` — one method call away from the exact
 * same leak. A later review (of #1036) found the positional check only
 * matched the bare identifier, so `log::warn!("...: {}", e.to_string())`
 * bypassed it entirely — anchored to the WHOLE top-level argument (see
 * `findPositionalErrorArg`), so a wrapper like
 * `sanitize_reason(&e.to_string())` — whose argument text is
 * `sanitize_reason(&e.to_string())`, not `e.to_string()` — still does not
 * match. Measured against the crate's real log call sites before adding
 * this: zero currently use `.to_string()`/`.to_owned()` positionally (every
 * `.to_string()` near a `log::` call is already inside `sanitize_reason`),
 * so this arm is a tripwire for a shape nobody has hit yet, not a fix for a
 * live leak.
 */
const positionalErrorRe = new RegExp(
  `^[&*]*(?:${ERROR_BINDING_NAMES.join('|')})(?:\\.(?:to_string|to_owned)\\(\\))?$`
);

/**
 * From `start` — the index in `text` right after a log macro's leading
 * format-string literal, still inside the macro call's own opening paren —
 * returns the raw text of the remaining arguments up to that call's matching
 * closing paren. Respects nested parens and string/char literals, so a `)`
 * inside a method call (`e.to_string()`) or a nested string does not end the
 * scan early. Returns `null` if the source runs out first (malformed or
 * truncated input — should not happen against real source).
 */
function restOfCall(text, start) {
  let depth = 1; // already inside the macro call's opening '('
  let i = start;
  while (i < text.length) {
    const c = text[i];
    if (c === '"' || c === "'") {
      const quote = c;
      i++;
      while (i < text.length && text[i] !== quote) {
        if (text[i] === '\\') i++;
        i++;
      }
      i++;
      continue;
    }
    if (c === '(') depth++;
    else if (c === ')') {
      depth--;
      if (depth === 0) return text.slice(start, i);
    }
    i++;
  }
  return null;
}

/**
 * Splits `argsText` on top-level commas — a comma inside a nested call
 * (`sanitize_reason(&e.to_string())`) or a string does not split — returning
 * each raw segment together with the offset (into `argsText`) it starts at.
 */
function splitTopLevelArgs(argsText) {
  const args = [];
  let depth = 0;
  let start = 0;
  let i = 0;
  while (i < argsText.length) {
    const c = argsText[i];
    if (c === '"' || c === "'") {
      const quote = c;
      i++;
      while (i < argsText.length && argsText[i] !== quote) {
        if (argsText[i] === '\\') i++;
        i++;
      }
      i++;
      continue;
    }
    if (c === '(' || c === '[' || c === '{') depth++;
    else if (c === ')' || c === ']' || c === '}') depth--;
    if (c === ',' && depth === 0) {
      args.push({ raw: argsText.slice(start, i), start });
      start = i + 1;
    }
    i++;
  }
  if (start < argsText.length) args.push({ raw: argsText.slice(start), start });
  return args;
}

/**
 * The offset (into `argsText`) of the first top-level argument that is a
 * bare error-binding identifier (`e`, `err`, `error`, `&err`, …) or that same
 * binding stringified (`e.to_string()`, `&err.to_owned()`) — the two shapes
 * that carry the SAME leak. Any other method call (`e.code()`, `e.kind()`,
 * `sanitize_reason(&e.to_string())`) never matches: each is a distinct
 * top-level argument in its own right (`sanitize_reason(&e.to_string())`,
 * not `e.to_string()`), which is what keeps the sanctioned safe forms out of
 * this check without naming them specially. `null` when no argument
 * qualifies.
 */
function findPositionalErrorArg(argsText) {
  for (const { raw, start } of splitTopLevelArgs(argsText)) {
    if (!positionalErrorRe.test(raw.trim())) continue;
    return start + (raw.length - raw.trimStart().length);
  }
  return null;
}

/**
 * A stable content signature for a log call's format-string literal —
 * whitespace-collapsed so a `\`-continued literal (see the multi-line test
 * case) reads the same regardless of exact line wrapping. This is what makes
 * `sig`-based matching (see `violations`) survive a line-number shift: an
 * unrelated edit above a declared site changes its LINE but never its own
 * message text, so the signature is unchanged. Purely a display/matching
 * convenience — not a security boundary, so a naive whitespace collapse
 * (rather than actually decoding Rust escapes) is precise enough.
 */
function normalizeSig(literal) {
  return literal.replace(/\s+/g, ' ').trim();
}

/**
 * Every `log::{warn,error,info,debug}!(...)` call site, under `srcDir`, that
 * interpolates a bound error identifier — captured in the format string
 * (`{e}`, `{err}`, `{error}`) or passed as a bare positional argument
 * (`"...{}...", e`). See `ERROR_BINDING_NAMES` for why detection is bounded
 * to those three names rather than every identifier.
 *
 * Returns `{ key, file, line, sig }[]`, `key` being
 * `"<path relative to src/>:<line>"` — the ALLOWLIST's own key shape. `line`
 * is the line the identifier itself sits on (for a multi-line format string,
 * or an argument list wrapped onto its own line, that is not necessarily the
 * `log::` call's own line). `sig` is the call's own format-string literal,
 * normalized — see `normalizeSig` and, for how it's used, `violations`.
 */
export function findLeaks(srcDir = join(REPO_ROOT, SRC_REL)) {
  const found = [];
  const callRe = /log::(?:warn|error|info|debug)!\(\s*"((?:[^"\\]|\\[\s\S])*)"/g;
  for (const file of rustFiles(srcDir)) {
    const text = readFileSync(file, 'utf8');
    const rel = toPosix(relative(srcDir, file));
    callRe.lastIndex = 0;
    let m;
    while ((m = callRe.exec(text))) {
      const sig = normalizeSig(m[1]);
      const captured = capturedErrorRe.exec(m[0]);
      if (captured) {
        const line = lineOf(text, m.index + captured.index);
        found.push({ key: `${rel}:${line}`, file: rel, line, sig });
        continue;
      }

      const rest = restOfCall(text, m.index + m[0].length);
      if (rest === null) continue;
      const argOffset = findPositionalErrorArg(rest);
      if (argOffset === null) continue;
      const line = lineOf(text, m.index + m[0].length + argOffset);
      found.push({ key: `${rel}:${line}`, file: rel, line, sig });
    }
  }
  return found;
}

/**
 * Every violation, as human-readable lines. Empty means the invariant holds.
 *
 * Returned rather than printed so the check is testable without capturing
 * stdout or trapping `process.exit`.
 */
export function violations(inventory = ALLOWLIST, leaks) {
  const problems = [];

  const declaredKeys = new Set(Object.keys(inventory));

  const fileOf = (key) => key.slice(0, key.lastIndexOf(':'));
  /** Group `items` into a Map keyed by `keyOf(item)`. */
  const bucketBy = (items, keyOf) => {
    const buckets = new Map();
    for (const item of items) {
      const k = keyOf(item);
      const bucket = buckets.get(k);
      if (bucket) bucket.push(item);
      else buckets.set(k, [item]);
    }
    return buckets;
  };

  // Every leak sitting at a given key today — ALL of them, never collapsed to
  // "the last one wins". A `key -> single leak` map here was exactly the
  // defect a 2026-08-31 review found: two `log::` calls sharing one line
  // (today the only way two DIFFERENT sites can share a key) produce two leak
  // records with that key, and keeping only the last meant whichever survived
  // decided BOTH records' fate — a declared site's own `sig` could "match"
  // for an undeclared leak one bucket over, purely because the map lookup by
  // key can't tell the two apart. Matching below is therefore always done
  // leak-by-leak (`leakLineExactOk`, using the LEAK's own `sig`) or, from the
  // entry's side, by asking "does ANY leak in this key's bucket satisfy the
  // entry" (`entryLineExactOk`) — never by re-deriving a single "the" leak
  // for a key.
  const leaksByKey = bucketBy(leaks, (l) => l.key);

  // Content-anchored matching (opt-in — an ALLOWLIST entry declares its own
  // `sig`, copied from the `sig` `findLeaks` reports for it): a line-keyed
  // entry breaks the moment an unrelated edit inserts a line above it (its
  // key no longer matches anything — the defect a 2026-08-19 review found).
  // An entry's OWN message text almost never changes at the same time, so
  // matching on (file, sig) survives that shift. Deliberately conservative:
  // only an UNAMBIGUOUS pairing counts — exactly one declared entry and
  // exactly one found leak sharing a (file, sig) pair. A genuinely repeated
  // message (two distinct call sites, identical text) falls back to the
  // ordinary line-exact rules below, so each site still needs its own
  // declaration rather than one `sig` silently covering both.
  //
  // Unambiguous means unambiguous on BOTH sides, and only against a leak no
  // other entry has already claimed:
  //
  //   - Entry side: two entries sharing one (file, sig) were each satisfied
  //     by the SAME single leak, so the stale duplicate was never reported.
  //   - Already-claimed leak: an entry's sig can match the leak a DIFFERENT
  //     entry declares by key; that entry then has no site of its own and IS
  //     stale. A sig pairing therefore only counts for a leak no entry
  //     declares by key — when the leak sits on an entry's own declared key
  //     (see `entryMatchesLeak` below), that's the ordinary same-site case
  //     and `sig` (if present) is checked there instead, not here.
  //
  // Both holes could only ever suppress a STALE-entry error, never an
  // undeclared-leak one: a second site with the same message makes the LEAK
  // bucket length 2, which disables sig matching for that bucket outright.
  // NUL separator: it occurs in neither a path nor a Rust string literal, so
  // two different (file, sig) pairs can never collide on one bucket key.
  const leaksBySig = bucketBy(leaks, (l) => `${l.file}\0${l.sig}`);
  const entriesBySig = bucketBy(
    Object.entries(inventory).filter(([, entry]) => entry.sig),
    ([entryKey, entry]) => `${fileOf(entryKey)}\0${entry.sig}`
  );
  const sigSatisfiedLeakKeys = new Set();
  const sigSatisfiedEntryKeys = new Set();
  for (const [bucketKey, entries] of entriesBySig) {
    if (entries.length !== 1) continue;
    const bucket = leaksBySig.get(bucketKey);
    if (!bucket || bucket.length !== 1) continue;
    if (declaredKeys.has(bucket[0].key)) continue;
    sigSatisfiedLeakKeys.add(bucket[0].key);
    sigSatisfiedEntryKeys.add(entries[0][0]);
  }

  // An entry with no opinion on content (no `sig`) matches any leak sitting
  // at its key; a `sig`-bearing entry only matches a leak whose OWN text
  // agrees — so a message changed in place (line untouched) still fails to
  // match, and (the fix here) a DIFFERENT leak sharing the same key never
  // borrows this leak's verdict just because they resolve to the same key.
  const entryMatchesLeak = (entry, leak) => !entry.sig || entry.sig === leak.sig;

  // Checked per LEAK OBJECT — never by re-looking-up "the" leak for a key,
  // which is what let a same-key collision silently borrow a neighbor's
  // verdict (see `leaksByKey` above).
  const leakLineExactOk = (leak) => {
    const entry = inventory[leak.key];
    return Boolean(entry) && entryMatchesLeak(entry, leak);
  };

  // An entry is still live at its key when AT LEAST ONE leak sitting there
  // satisfies it — not "the" leak, since (with a same-key collision) a key
  // can hold more than one.
  const entryLineExactOk = (key) => {
    const entry = inventory[key];
    const candidates = leaksByKey.get(key);
    if (!entry || !candidates) return false;
    return candidates.some((l) => entryMatchesLeak(entry, l));
  };

  const undeclared = leaks.filter((l) => !leakLineExactOk(l) && !sigSatisfiedLeakKeys.has(l.key));
  if (undeclared.length > 0) {
    problems.push(
      'These log call sites interpolate a caught error, captured or positional, which can ' +
        'leak ' +
        'an absolute path (rusqlite::Error::InvalidPath, a filesystem std::io::Error), a ' +
        'credential-bearing URL (reqwest::Error), or a host:port into the log — and are ' +
        'not declared in ALLOWLIST:\n' +
        undeclared.map((l) => `    ${l.key}`).join('\n') +
        '\n  Fix it (`.code()` for a fixed category, or ' +
        'observability::sanitize_reason(&e.to_string()) to keep the safe part of the ' +
        'message — see the comment on applications::ApplicationStore::open), or add an ' +
        'ALLOWLIST entry in scripts/check-log-error-leaks.mjs stating why this one ' +
        'cannot leak.'
    );
  }

  const stale = [...declaredKeys].filter(
    (k) => !entryLineExactOk(k) && !sigSatisfiedEntryKeys.has(k)
  );
  if (stale.length > 0) {
    problems.push(
      'Declared in ALLOWLIST but no `{e}` site found there anymore (fixed, moved, or ' +
        'deleted) — remove the stale entries so the list stays a true inventory:\n' +
        stale.map((k) => `    ${k}`).join('\n')
    );
  }

  const unexplained = Object.entries(inventory)
    .filter(([, e]) => !e.reason || e.reason.trim().length < MIN_REASON_CHARS)
    .map(([k]) => k);
  if (unexplained.length > 0) {
    problems.push(
      'Every ALLOWLIST entry must state why that site cannot leak (or is deliberately ' +
        'left for a follow-up):\n' +
        unexplained.map((k) => `    ${k}`).join('\n')
    );
  }

  const badStatus = Object.entries(inventory)
    .filter(([, e]) => e.status !== 'safe' && e.status !== 'debt')
    .map(([k]) => k);
  if (badStatus.length > 0) {
    problems.push(
      "`status` must be 'safe' or 'debt':\n" + badStatus.map((k) => `    ${k}`).join('\n')
    );
  }

  return problems;
}

/**
 * Below this many total sites, the scan is treated as broken rather than
 * "every leak got fixed" — an ABSOLUTE floor, not a comparison against
 * ALLOWLIST's size (that comparison is exactly what would make a single
 * legitimately-fixed site indistinguishable from every regex match silently
 * going to zero; the `stale`-entry check above already covers the former).
 * The real repo sits at 57 as of this guard's introduction; a Rust log-macro
 * idiom change or a regex typo dropping this near zero must fail loudly
 * rather than read as "nothing left to declare".
 */
export const MIN_SITES = 40;

// Skipped when imported by the test file.
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const leaks = findLeaks();

  if (leaks.length < MIN_SITES) {
    console.error(
      `✗ Only ${leaks.length} \`{e}\` site(s) found under ${SRC_REL} — expected at ` +
        `least ${MIN_SITES}. The log-macro pattern this scans for may have changed; fix ` +
        'the scan before trusting a green run.'
    );
    process.exit(1);
  }

  const problems = violations(ALLOWLIST, leaks);

  if (problems.length > 0) {
    for (const p of problems) console.error(`✗ ${p}`);
    process.exit(1);
  }

  const safe = Object.values(ALLOWLIST).filter((e) => e.status === 'safe').length;
  const debt = Object.values(ALLOWLIST).filter((e) => e.status === 'debt').length;
  console.log(
    `check:log-error-leaks OK — ${leaks.length} \`{e}\` site(s) found, all declared ` +
      `(${safe} provably safe, ${debt} tracked as debt).`
  );
}

export { ALLOWLIST, SRC_REL };
