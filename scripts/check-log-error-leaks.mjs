// Drift guard: no bare `{e}` interpolation of a caught error inside a
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
// Every `log::{warn,error,info,debug}!(...)` call whose leading string
// literal interpolates a bound error via the literal captured-identifier
// form `{e}` (Rust 2021 format-string capture — the only shape this
// codebase's ~250 log call sites ever used for an error binding; a
// positional `"{}", e` call is not detected — see the note on `findLeaks`).
//
// NOT "no site may ever print `{e}`" — an error whose `Display` structurally
// cannot carry a path/URL/host/credential (a pure JSON/PDF parse failure, a
// fixed-string domain error) loses real diagnostic value for nothing if it is
// forced through `.code()`. This enforces that the question was ANSWERED:
// every surviving `{e}` site is declared in ALLOWLIST with a one-line reason,
// tagged `safe` (provably cannot leak) or `debt` (a real leak candidate this
// pass did not fix — see each entry). A newly introduced site fails until
// someone writes that sentence, mirroring check-event-subscriptions.mjs.
//
// Both directions are checked, so the list cannot rot: an undeclared `{e}`
// site fails, and so does an ALLOWLIST entry that no longer corresponds to a
// real site (fixed, moved, or deleted) — see `violations()`.
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

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const SRC_REL = 'apps/desktop/src-tauri/src';

/**
 * Every surviving `{e}` site, keyed `"<path relative to src/>:<line>"` (the
 * line the literal `{e}` text sits on — for a multi-line format string that
 * is not necessarily the `log::` call's own line).
 *
 * `status: 'safe'` — the error's `Display` structurally cannot carry a
 *   path/URL/host/credential; `.code()`/`sanitize_reason` would only lose
 *   diagnostic value. `status: 'debt'` — a likely-real leak, out of this
 *   pass's scope, tracked for a follow-up fix (see the file header).
 */
const ALLOWLIST = {
  // ── safe: provably cannot carry a path/URL/host/credential ────────────────
  'autopilot/mod.rs:754': {
    status: 'safe',
    reason:
      "serde_json::from_value type-mismatch parsing the app's own Autopilot " +
      'record on the load path — a pure parse failure, never a path/URL.',
  },
  'autopilot_helpers/mod.rs:395': {
    status: 'safe',
    reason:
      "limits::Limiter::charge_provider_daily's AppError::RateLimited message is a " +
      'fixed, author-written template naming only the provider id and a static ' +
      'daily-ceiling number — see limits/mod.rs.',
  },
  'documents/mod.rs:1153': {
    status: 'safe',
    reason: 'Same charge_provider_daily fixed-template message as autopilot_helpers/mod.rs:395.',
  },
  'validate/mod.rs:401': {
    status: 'safe',
    reason:
      'lopdf::Document::load_mem parses IN-MEMORY bytes, not a file path — its errors ' +
      'are fixed format-parse messages ("trailer not found", …), never a path.',
  },
  'profile_import/linkedin.rs:134': {
    status: 'safe',
    reason:
      "net::http::read_text_capped's error path already calls reqwest::Error::" +
      'without_url() before wrapping in AppError::Network (see accumulate_capped in ' +
      'net/http.rs) — the URL is stripped upstream of this call site.',
  },
  'postings/mod.rs:424': {
    status: 'safe',
    reason:
      "serde_json::to_string_pretty is a SERIALIZE error on the app's own " +
      'InteractionRecord — a type-system failure, not an echo of file content.',
  },
  'notifications/mod.rs:231': {
    status: 'safe',
    reason: 'Same serde_json serialize-error shape as postings/mod.rs:424 (AppNotification).',
  },
  'commands/autopilot/rerank.rs:270': {
    status: 'safe',
    reason: 'Same charge_provider_daily fixed-template message as autopilot_helpers/mod.rs:395.',
  },
  'commands/profile_import.rs:20': {
    status: 'safe',
    reason:
      "import_from_url's only branch (linkedin::import) already collapses every " +
      "failure to a fixed AppError::Network string ('could not reach linkedin' / " +
      "'could not read the linkedin response') before returning — no URL/host " +
      'reaches this call site.',
  },
  'extension_bridge/register.rs:114': {
    status: 'safe',
    reason:
      'std::env::current_exe() takes no input path to leak — a failure here is a ' +
      'generic OS resource-resolution error.',
  },
  'extension_bridge/native_host.rs:50': {
    status: 'safe',
    reason:
      'tokio::runtime::Builder::build() failure is a generic OS-thread/resource error; ' +
      'building a runtime touches no filesystem path.',
  },
  'extension_bridge/mod.rs:483': {
    status: 'safe',
    reason:
      'TcpListener::accept() failure is a local socket-resource error (e.g. EMFILE); it ' +
      'carries no peer address or path — the peer is a separate, unread `_peer` binding.',
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
  'scraping/boards/greenhouse/mod.rs:110': httpChokepointSafe(),
  'scraping/boards/breezy/mod.rs:287': httpChokepointSafe(),
  'scraping/boards/ycombinator/mod.rs:109': httpChokepointSafe(),
  'scraping/boards/aggregator/adzuna.rs:429': httpChokepointSafe(),
  'scraping/boards/aggregator/adzuna.rs:562': httpChokepointSafe(),
  'scraping/boards/arbeitnow/mod.rs:81': httpChokepointSafe(),
  'scraping/boards/bamboohr/mod.rs:204': httpChokepointSafe(),
  'scraping/boards/arbeitsagentur/mod.rs:164': httpChokepointSafe(),
  'scraping/boards/aggregator/mod.rs:336': httpChokepointSafe(),
  'scraping/boards/aggregator/mod.rs:370': httpChokepointSafe(),
  'scraping/boards/aggregator/mod.rs:403': httpChokepointSafe(),
  'scraping/boards/aggregator/mod.rs:491': httpChokepointSafe(),
  'scraping/boards/aggregator/mod.rs:725': httpChokepointSafe(),
  'scraping/boards/ashby/mod.rs:146': httpChokepointSafe(),
  'scraping/boards/aggregator/freehire.rs:465': httpChokepointSafe(),
  'scraping/boards/workable/mod.rs:294': httpChokepointSafe(),
  'scraping/boards/lever/mod.rs:143': httpChokepointSafe(),
  'scraping/boards/pinpoint/mod.rs:181': httpChokepointSafe(),
  'scraping/boards/smartrecruiters/mod.rs:142': httpChokepointSafe(),
  'scraping/boards/smartrecruiters/mod.rs:180': httpChokepointSafe(),
  'scraping/boards/themuse/mod.rs:193': httpChokepointSafe(),
  'scraping/boards/recruitee/mod.rs:117': httpChokepointSafe(),
  'scraping/boards/rippling/mod.rs:230': httpChokepointSafe(),
  'scraping/boards/personio/mod.rs:208': httpChokepointSafe(),

  // ── scraping/**: pure in-memory parse of already-fetched data ─────────────
  // `serde_json::from_value`/`from_str` on a JSON value/body already read into
  // memory — Display is a schema-mismatch message ("missing field `x`",
  // "invalid type: …"), structurally incapable of carrying a path/URL/host/
  // credential.
  'scraping/boards/breezy/mod.rs:100': inMemoryParseSafe(),
  'scraping/boards/jobicy/mod.rs:64': inMemoryParseSafe(),
  'scraping/boards/jobicy/mod.rs:165': inMemoryParseSafe(),
  'scraping/boards/comeet/mod.rs:70': inMemoryParseSafe(),
  'scraping/boards/rippling/mod.rs:89': inMemoryParseSafe(),
  'scraping/boards/workable/mod.rs:116': inMemoryParseSafe(),
  'scraping/http/mod.rs:332': inMemoryParseSafe(),

  'scraping/http/mod.rs:442': {
    status: 'safe',
    reason:
      "htmd::convert parses an already-fetched, in-memory HTML string (html_to_markdown's " +
      'own fallback path) — its Display is a structural markdown-conversion failure, never ' +
      'a path/URL/host/credential.',
  },
  'scraping/engine/mod.rs:1081': {
    status: 'safe',
    reason:
      'tokio::task::JoinError from the record_health spawn_blocking handle (the task ' +
      'panicked or was cancelled) — its Display names only the task id and panic/cancel ' +
      'state, never a path/URL/host/credential. Distinct from the sibling arm 3 lines ' +
      'above, which IS a path-carrying AppError from the store and already uses .code() ' +
      'for exactly that reason.',
  },
};

/** Factory for the fetch_text/fetch_json-chokepoint shape — see the comment
 * above the first entry that uses it for the full rationale; kept as one
 * factory so the reasoning is written once and every site stays independently
 * declared (the undeclared/stale checks below still cover each individually). */
function httpChokepointSafe() {
  return {
    status: 'safe',
    reason:
      'Every error here originates from scraping::http::fetch_text/fetch_json (or, for ' +
      'the Adzuna/JSearch/Jooble/Apify providers, a thin anyhow wrapper one hop away from ' +
      "the same call) — the fleet's HTTP chokepoint for named-board scrapers, whose " +
      'transport-failure branch already calls reqwest::Error::without_url() before ' +
      'wrapping in AppError::Network, and whose other error variants (HTTP-status, ' +
      'body-too-large, generic schema-drift, cancelled) are fixed path/URL-free messages.',
  };
}

/** Factory for the pure in-memory-parse shape — see the comment above the
 * first entry that uses it. */
function inMemoryParseSafe() {
  return {
    status: 'safe',
    reason:
      'serde_json parsing an already-fetched, in-memory JSON value/body — a pure schema-' +
      'mismatch failure, never a path, URL, host, or credential.',
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
 * Every `log::{warn,error,info,debug}!(...)` call site, under `srcDir`, whose
 * leading string-literal argument contains the literal captured-identifier
 * `{e}`.
 *
 * Detects only the captured-identifier form (`"...{e}..."`), which is the
 * shape every log call in this codebase used for an error binding at the time
 * this guard was written. A positional call (`log::warn!("...{}...", e)`)
 * would dodge detection — not handled, since introducing that shape here
 * would itself be a conspicuous, reviewable stylistic departure, and this
 * guard's job is to catch the shape that actually recurred 145 times, not
 * every theoretical rewrite of it.
 *
 * Returns `{ key, file, line }[]`, `key` being `"<path relative to src/>:<line>"`
 * — the ALLOWLIST's own key shape.
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
      const braceOffset = m[0].indexOf('{e}');
      if (braceOffset === -1) continue;
      const line = lineOf(text, m.index + braceOffset);
      found.push({ key: `${rel}:${line}`, file: rel, line });
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

  const foundKeys = new Set(leaks.map((l) => l.key));
  const declaredKeys = new Set(Object.keys(inventory));

  const undeclared = leaks.filter((l) => !declaredKeys.has(l.key));
  if (undeclared.length > 0) {
    problems.push(
      'These log call sites interpolate a caught error via bare `{e}`, which can leak ' +
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

  const stale = [...declaredKeys].filter((k) => !foundKeys.has(k));
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
