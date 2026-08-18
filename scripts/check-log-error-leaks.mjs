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
// `debt` entries are scraping/** sites: `scraping/**` is scraping-applier's
// domain (AGENTS.md / CLAUDE.md domain routing), out of this pass's
// primary-path scope. They are pattern-identical to sites fixed elsewhere in
// this same pass (mostly a `reqwest::Error` embedding the request URL, or a
// `keyring` error) and are very likely genuine leaks — recorded rather than
// fixed here so the fix is reviewable by that domain's own author, not so it
// is forgotten. `pnpm check:log-error-leaks` will keep failing on any NEW
// site added anywhere in the crate, `scraping/**` included, the whole time
// this debt sits open.

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

  // ── debt: scraping-applier domain, out of this pass's scope ───────────────
  'scraping/board_health/mod.rs:594': scrapingDebt(),
  'scraping/board_login/mod.rs:202': scrapingDebt(),
  'scraping/board_login/import.rs:98': scrapingDebt(),
  'scraping/board_login/import.rs:174': scrapingDebt(),
  'scraping/boards/greenhouse/mod.rs:110': scrapingDebt(),
  'scraping/boards/breezy/mod.rs:100': scrapingDebt(),
  'scraping/boards/breezy/mod.rs:287': scrapingDebt(),
  'scraping/boards/ycombinator/mod.rs:109': scrapingDebt(),
  'scraping/boards/aggregator/adzuna.rs:264': scrapingDebt(),
  'scraping/boards/aggregator/adzuna.rs:269': scrapingDebt(),
  'scraping/boards/aggregator/adzuna.rs:422': scrapingDebt(),
  'scraping/boards/aggregator/adzuna.rs:555': scrapingDebt(),
  'scraping/engine/mod.rs:1081': scrapingDebt(),
  'scraping/boards/arbeitnow/mod.rs:81': scrapingDebt(),
  'scraping/boards/bamboohr/mod.rs:204': scrapingDebt(),
  'scraping/boards/arbeitsagentur/mod.rs:164': scrapingDebt(),
  'scraping/boards/aggregator/mod.rs:336': scrapingDebt(),
  'scraping/boards/aggregator/mod.rs:370': scrapingDebt(),
  'scraping/boards/aggregator/mod.rs:403': scrapingDebt(),
  'scraping/boards/aggregator/mod.rs:491': scrapingDebt(),
  'scraping/boards/aggregator/mod.rs:725': scrapingDebt(),
  'scraping/boards/aggregator/providers.rs:171': scrapingDebt(),
  'scraping/boards/aggregator/providers.rs:430': scrapingDebt(),
  'scraping/boards/aggregator/providers.rs:777': scrapingDebt(),
  'scraping/boards/ashby/mod.rs:146': scrapingDebt(),
  'scraping/boards/aggregator/freehire.rs:465': scrapingDebt(),
  'scraping/boards/comeet/mod.rs:69': scrapingDebt(),
  'scraping/boards/comeet/mod.rs:220': scrapingDebt(),
  'scraping/boards/comeet/mod.rs:226': scrapingDebt(),
  'scraping/boards/workable/mod.rs:116': scrapingDebt(),
  'scraping/boards/workable/mod.rs:294': scrapingDebt(),
  'scraping/linkedin/api_client/mod.rs:356': scrapingDebt(),
  'scraping/boards/lever/mod.rs:143': scrapingDebt(),
  'scraping/boards/pinpoint/mod.rs:181': scrapingDebt(),
  'scraping/boards/jobicy/mod.rs:64': scrapingDebt(),
  'scraping/boards/jobicy/mod.rs:165': scrapingDebt(),
  'scraping/boards/smartrecruiters/mod.rs:142': scrapingDebt(),
  'scraping/boards/smartrecruiters/mod.rs:180': scrapingDebt(),
  'scraping/boards/themuse/mod.rs:193': scrapingDebt(),
  'scraping/http/mod.rs:332': scrapingDebt(),
  'scraping/http/mod.rs:442': scrapingDebt(),
  'scraping/boards/recruitee/mod.rs:117': scrapingDebt(),
  'scraping/boards/rippling/mod.rs:89': scrapingDebt(),
  'scraping/boards/rippling/mod.rs:230': scrapingDebt(),
  'scraping/boards/personio/mod.rs:208': scrapingDebt(),
};

/** Factory rather than one repeated literal so every `debt` entry stays easy
 * to scan for its (shared) rationale in one place; still one explicit
 * ALLOWLIST entry per site, so the undeclared/stale checks below cover each
 * individually. */
function scrapingDebt() {
  return {
    status: 'debt',
    reason:
      "scraping-applier domain (AGENTS.md domain routing) — out of this pass's " +
      'primary-path scope. Pattern-identical to sites fixed elsewhere in this pass ' +
      '(a reqwest::Error embedding the request URL, or a keyring error) and very ' +
      "likely a genuine leak — needs a follow-up fix from that domain's author, not " +
      'yet applied here.',
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
