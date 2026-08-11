#!/usr/bin/env node
/**
 * review-gate.mjs — global Stop hook. Deterministic, near-free code review.
 * Generic by default; specialized per-project via <cwd>/.claude/review-routes.json.
 * NEVER hard-fails the session: any error → exit 0 (don't block the user on a hook bug),
 * but every meaningful run — including failures — logs one line to .claude/.review-metrics.jsonl.
 *
 * 2026-07-28: the LLM review moved OFF the Stop hook to pre-push and CI — 74.7%
 * of 830 stop-gate LLM calls died in their own 120s timeout for 68 findings /
 * 8 blocks ever; the per-finish nested `claude -p` was the setup's single
 * largest recurring token cost. 2026-08-11: the pre-push half was removed too
 * (CodeRabbit + CI cover it), so CI is the only repository-owned automated LLM
 * gate (the interactive pre-PR agent chain and CodeRabbit still review, outside
 * this repo's own automation). The Stop gate
 * keeps every deterministic tier: guards → skip-list → ledger re-emits → Tier 0
 * ast-grep arch-guards → reviewed-hash cache → verdict. Unresolved ledger findings
 * (seeded by the CI LLM review) still re-block a finish until the file
 * actually changes.
 * Scope: the full branch range (merge-base with origin/main → HEAD) PLUS the working
 *        tree and untracked files — committing does not blind the gate.
 */
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync, spawnSync } from 'node:child_process';
import {
  SKIP_GLOBS,
  SEVERITIES,
  matchesAny,
  splitByFile,
  assembleDiff,
  fileHunkHashes,
  loadLedger,
  appendLedger,
  blockingFindings,
  formatFinding,
  countBySeverity,
  appendMetrics,
} from './review-lib.mjs';

const exit0 = (msg) => {
  if (msg) process.stdout.write(msg);
  process.exit(0);
};
const block = (reason) => {
  process.stdout.write(JSON.stringify({ decision: 'block', reason }));
  process.exit(0);
};

const t0 = Date.now();
const metric = { kind: 'stop-gate', branch: '', model: '', files: 0 };
let metricsCwd = process.cwd();
const logM = (extra) =>
  appendMetrics(metricsCwd, { ...metric, duration_ms: Date.now() - t0, ...extra });

try {
  // --- read Stop payload (stdin) ---
  let payload = {};
  if (!process.stdin.isTTY) {
    try {
      payload = JSON.parse(fs.readFileSync(0, 'utf8') || '{}');
    } catch {}
  }

  // 1. Guards (two distinct mechanisms)
  if (process.env.REVIEW_HOOK_ACTIVE) exit0(); // reviewer subprocess must never review itself (fork-bomb guard)
  if (payload.stop_hook_active === true) exit0(); // one review→fix cycle per finish-chain (block-once)

  const cwd = payload.cwd || process.cwd();
  metricsCwd = cwd;
  const git = (args) =>
    execFileSync('git', args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });

  // 2. git + scope: branch range (merge-base..HEAD) ∪ working tree ∪ untracked
  let inRepo = false;
  try {
    inRepo = git(['rev-parse', '--is-inside-work-tree']).trim() === 'true';
  } catch {}
  if (!inRepo) exit0();

  let branch = '';
  try {
    branch = git(['rev-parse', '--abbrev-ref', 'HEAD']).trim();
  } catch {}
  metric.branch = branch;

  // committed-but-unmerged range — skipped on main/detached (PRs-only repo policy)
  let mergeBase = '';
  if (branch && branch !== 'main' && branch !== 'HEAD') {
    try {
      mergeBase = git(['merge-base', 'origin/main', 'HEAD']).trim();
      if (mergeBase === git(['rev-parse', 'HEAD']).trim()) mergeBase = ''; // nothing committed
    } catch {}
  }

  const names = (args) => {
    try {
      return git(args)
        .split('\n')
        .map((s) => s.trim())
        .filter(Boolean);
    } catch {
      return [];
    }
  };
  const committedNames = mergeBase ? names(['diff', '--name-only', `${mergeBase}..HEAD`]) : [];
  const workingNames = names(['diff', '--name-only', 'HEAD']);
  const untracked = names(['ls-files', '--others', '--exclude-standard']);
  const changed = [...new Set([...committedNames, ...workingNames, ...untracked])];
  if (!changed.length) exit0();

  // 3. skip-list (deterministic)
  const nonSkipped = changed.filter((f) => !matchesAny(f, SKIP_GLOBS));
  if (!nonSkipped.length) exit0();
  metric.files = nonSkipped.length;

  // 4. per-file diff segments (rename-aware) → drop-order assembly, hunk-safe cuts
  const MAX = 60000;
  const segments = [];
  const collect = (args) => {
    try {
      segments.push(...splitByFile(git(args)));
    } catch {}
  };
  // ONE diff from merge-base (or HEAD) to the WORKING TREE — committed + uncommitted
  // captured in a single pass, so a file changed in both never yields two segments.
  collect(['diff', '-M', mergeBase || 'HEAD', '--unified=3', '--', ...nonSkipped]);
  // `git diff HEAD` is blind to untracked files — diff each against /dev/null.
  // --no-index exits 1 on difference, so output is recovered from the thrown error.
  for (const f of untracked) {
    if (!nonSkipped.includes(f)) continue;
    try {
      segments.push(
        ...splitByFile(git(['diff', '--no-index', '--unified=3', '--', '/dev/null', f]))
      );
    } catch (e) {
      if (e && typeof e.stdout === 'string') segments.push(...splitByFile(e.stdout));
    }
  }
  // count files that actually produced diff segments (a committed-then-reverted
  // file is in nonSkipped but nets to zero) — metrics must reflect what was reviewed
  metric.files = new Set(segments.map((s) => s.file)).size;

  // 4b. findings ledger — cross-run finding state for this branch, seeded by the
  // pre-push/CI LLM reviews. OPEN findings whose file diff is byte-identical
  // re-emit VERBATIM; a changed file auto-resolves its old findings. Categories
  // the user has ignored 3+ consecutive times auto-suppress — but ONLY style/perf/i18n,
  // never security/correctness/data-loss/arch/test-coverage.
  const fileHunks = fileHunkHashes(segments);
  const ledger = loadLedger(cwd, branch);
  const sameHunks = (a, b) => a.length === b.length && a.every((h, i) => h === b[i]);
  const SUPPRESSIBLE = new Set(['style', 'perf', 'i18n']);
  const openEntries = [...ledger.values()].filter((e) => e.status === 'open' && e.finding);
  const suppressedCategories = new Set(
    openEntries
      .filter((e) => SUPPRESSIBLE.has(e.finding.category) && (e.reemits || 0) >= 3)
      .map((e) => e.finding.category)
  );
  const reEmitted = [];
  const reEmitFiles = new Set();
  const ledgerAppends = [];
  for (const e of openEntries) {
    // ledger rows are written verbatim by prior runs — never trust their shape (a
    // malformed finding would throw in countBySeverity/formatFinding downstream)
    if (
      typeof e.finding.file !== 'string' ||
      typeof e.finding.summary !== 'string' ||
      !SEVERITIES.includes(String(e.finding.severity))
    )
      continue;
    // entries without a hunk baseline (e.g. /review-sourced) can't be re-emitted or
    // auto-resolved reliably — they exist for /review-stats only
    if (!Array.isArray(e.fileHunks) || !e.fileHunks.length) continue;
    const cur = fileHunks.get(e.finding.file);
    if (!cur || !sameHunks(e.fileHunks || [], cur)) {
      ledgerAppends.push({
        branch,
        status: 'resolved-changed',
        finding: e.finding,
        fileHunks: cur || [],
      });
    } else if (suppressedCategories.has(e.finding.category)) {
      ledgerAppends.push({
        branch,
        status: 'suppressed',
        finding: e.finding,
        fileHunks: cur,
        reemits: e.reemits || 0,
      });
    } else {
      reEmitted.push(e.finding);
      reEmitFiles.add(e.finding.file);
      ledgerAppends.push({
        branch,
        status: 'open',
        finding: e.finding,
        fileHunks: cur,
        reemits: (e.reemits || 0) + 1,
      });
    }
  }
  metric.reemits = reEmitted.length;

  // files with verbatim re-emits carry a known verdict already
  const modelSegments = segments.filter((s) => !reEmitFiles.has(s.file));
  const { diff, omitted, deletedCount } = assembleDiff(modelSegments, MAX);
  if (!diff.trim() && !reEmitted.length) {
    appendLedger(cwd, ledgerAppends); // record resolved-changed transitions
    exit0();
  }

  // trivial-change heuristic (comment/import/blank-only → skip)
  const codeLines = diff.split('\n').filter((l) => /^[+-]/.test(l) && !/^[+-]{3}/.test(l));
  const meaningful = codeLines.filter((l) => {
    const b = l.slice(1).trim();
    if (!b) return false;
    if (/^(\/\/|\/\*|\*|#)/.test(b)) return false;
    if (/^(import |use |pub use |mod |from )/.test(b)) return false;
    return true;
  });
  if (!meaningful.length && !reEmitted.length) {
    // deletion-only / over-budget-only changes carry no +/- lines — the review is
    // skipped as a degradation, but the metrics must not read as "clean".
    appendLedger(cwd, ledgerAppends); // record resolved-changed transitions
    if (deletedCount || omitted.length) logM({ outcome: 'degraded', blocked: false });
    exit0();
  }

  // 5. Tier 0 — deterministic guards (confidence 1.0). Primary: the ast-grep pack
  // (.claude/review-rules/*.yml, discovered via repo-root sgconfig.yml — never pass
  // -c, it silently breaks the rules' files: globs). Fallback: legacy JS regexes
  // when the binary is unavailable.
  const findLine = (content, re) => {
    const i = content.split('\n').findIndex((l) => re.test(l));
    return i >= 0 ? i + 1 : 0;
  };
  // added (+) lines per file — a violation only BLOCKS when the diff introduced it;
  // a pre-existing hit in a touched file surfaces honestly as non-blocking
  // (introduced_by_diff: false — CI architecture tests own pre-existing debt).
  const addedByFile = new Map();
  for (const s of segments) {
    const added = s.text
      .split('\n')
      .filter((l) => l.startsWith('+') && !l.startsWith('+++'))
      .map((l) => l.slice(1))
      .join('\n');
    addedByFile.set(s.file, (addedByFile.get(s.file) || '') + added + '\n');
  }
  const introducedIn = (file, snippet) => {
    const probe = (snippet || '').split('\n')[0].trim();
    return probe ? (addedByFile.get(file) || '').includes(probe) : false;
  };
  const tier0 = [];
  let sgRan = false;
  try {
    // ast-grep exits 1 when error findings exist — that is a result, not a failure;
    // forward-slash repo-relative paths only (backslashes break files: globs).
    // shell:true is required on Windows (pnpm is a .cmd) — quote every path so
    // spaces/metacharacters in a filename can't break or be interpreted by the shell.
    const r = spawnSync(
      'pnpm',
      ['exec', 'ast-grep', 'scan', '--json=compact', '--', ...nonSkipped.map((f) => `"${f}"`)],
      // 20s fits inside the 30s Stop-hook budget (settings.json) with headroom —
      // the harness would otherwise SIGKILL the hook mid-scan on a slow ast-grep run
      { cwd, encoding: 'utf8', shell: true, timeout: 20000, maxBuffer: 20 * 1024 * 1024 }
    );
    const out = (r.stdout || '').trim();
    // distinguish "ran clean, empty output" from "binary missing/errored" — only the
    // latter may fall back to the legacy regexes
    const matches = !r.error && r.status === 0 && !out ? [] : JSON.parse(out);
    sgRan = true;
    for (const m of matches) {
      const file = String(m.file || '').replace(/\\/g, '/');
      const introduced = introducedIn(file, m.text);
      tier0.push({
        severity: m.severity === 'error' ? 'HIGH' : 'MEDIUM',
        category: 'arch',
        file,
        line: (m.range && m.range.start && m.range.start.line + 1) || 0,
        summary: (m.message || m.ruleId) + (introduced ? '' : ' (pre-existing)'),
        evidence: introduced
          ? `ast-grep rule ${m.ruleId}`
          : `ast-grep rule ${m.ruleId} — pre-existing in a touched file, not introduced by this diff`,
        fix: m.note || '',
        confidence: 1,
        introduced_by_diff: introduced,
      });
    }
  } catch {}
  if (!sgRan) {
    metric.sg_fallback = true;
    const arch = (file, line, summary, fix, introduced) =>
      tier0.push({
        severity: 'HIGH',
        category: 'arch',
        file,
        line,
        summary: introduced ? summary : `${summary} (pre-existing)`,
        evidence: introduced
          ? 'deterministic Tier-0 guard (docs/architecture-rules.md)'
          : 'pre-existing in a touched file — not introduced by this diff',
        fix,
        confidence: 1,
        introduced_by_diff: introduced,
      });
    const ARCH_RULES = [
      [
        /std::env::var(_os)?\b/,
        /\/platform\//,
        'std::env::var outside platform/',
        'move env access into platform/config.rs',
      ],
      [
        // R5 is about CONSTRUCTION — using the reqwest::Client type elsewhere is fine.
        // exempt only net/http.rs, matching the ast-grep rule's ignores exactly
        /reqwest::Client::(new|builder)\(|reqwest::ClientBuilder::new\(/,
        /\/net\/http\.rs$/,
        'reqwest client constructed outside net/http.rs',
        'use net/http.rs shared()/build_client()',
      ],
      [
        /Result<[^>]*,\s*String\s*>/,
        /\/error(\.rs|\/)/,
        'untyped Result<_, String> outside error/',
        'use AppError/AppResult',
      ],
    ];
    for (const f of nonSkipped) {
      if (!f.endsWith('.rs')) continue;
      let content = '';
      try {
        content = fs.readFileSync(path.join(cwd, f), 'utf8');
      } catch {
        continue;
      }
      const p = '/' + f;
      const added = addedByFile.get(f) || '';
      // true = introduced by the diff, false = pre-existing, null = absent
      const hit = (re) => (re.test(added) ? true : re.test(content) ? false : null);
      for (const [re, exemptRe, summary, fix] of ARCH_RULES) {
        if (exemptRe.test(p)) continue;
        const h = hit(re);
        if (h !== null) arch(f, findLine(content, re), summary, fix, h);
      }
    }
  }

  // 6. reviewed-hash cache (body-only hunk hashes; line-number agnostic).
  // READ-ONLY here — the Stop gate fast-exits on already-reviewed-clean code but
  // never writes reviewed-clean state. Its only writer was the pre-push LLM gate,
  // deleted 2026-08-11, so on a fresh clone the file is simply absent and every
  // run falls through to the deterministic tiers below (a slower pass, never a
  // wrong one). Left in place: a still-present cache from before the removal is
  // valid, and the CI surface may seed it again.
  const cachePath = path.join(cwd, '.claude', '.review-cache');
  let cache = new Set();
  try {
    cache = new Set(fs.readFileSync(cachePath, 'utf8').split('\n').filter(Boolean));
  } catch {}
  // cache membership is judged over ALL segments (incl. re-emit files)
  const hashes = [...fileHunks.values()].flat();
  // pre-existing / warning-severity tier-0 hits must not defeat the cache forever
  const tier0Blocking = tier0.some((t) => t.introduced_by_diff && t.severity === 'HIGH');
  if (hashes.length && hashes.every((h) => cache.has(h)) && !tier0Blocking && !reEmitted.length) {
    logM({ outcome: 'cache-skip', blocked: false });
    exit0();
  }

  // an INTRODUCED tier-0 hit is deterministic (confidence 1) — block immediately.
  if (tier0Blocking) {
    metric.model = 'none';
    metric.findings = countBySeverity(tier0);
    appendLedger(cwd, ledgerAppends); // resolved/suppressed transitions still recorded
    logM({ outcome: 'tier0-block', blocked: true });
    block(
      `Review gate [tier-0 arch guards] — blocking issues, address then finish:\n\n${tier0
        .filter((t) => t.introduced_by_diff && t.severity === 'HIGH')
        .map(formatFinding)
        .join('\n')}`
    );
  }

  // 7. Verdict — deterministic only (no LLM on the Stop path; see header).
  metric.model = 'none';
  metric.findings = countBySeverity([...tier0, ...reEmitted]);
  appendLedger(cwd, ledgerAppends);

  const fmt = (f) =>
    formatFinding(f) + (reEmitted.includes(f) ? ' · (unresolved from previous review)' : '');
  const reBlocking = blockingFindings(reEmitted, 0.6); // introduced tier-0 HIGH blocked above
  if (reBlocking.length) {
    logM({ outcome: 'reemit-block', blocked: true });
    block(
      `Review gate — unresolved findings from the previous review (file unchanged):\n\n${reBlocking
        .map(fmt)
        .join('\n')}`
    );
  }

  // advisories (non-blocking)
  let routes = null;
  try {
    routes = JSON.parse(fs.readFileSync(path.join(cwd, '.claude', 'review-routes.json'), 'utf8'));
  } catch {}
  const advisory = [];
  if (routes && routes.advisory) {
    if (nonSkipped.some((f) => matchesAny(f, routes.advisory.docs_stale)))
      advisory.push('docs may be stale → run /update-docs');
    if (nonSkipped.some((f) => matchesAny(f, routes.advisory.release)))
      advisory.push('release config changed → run /prepare-release');
  }
  const testable = nonSkipped.some(
    (f) => /\.(rs|ts|tsx)$/.test(f) && !/\.(test|spec)\./.test(f) && !/\.d\.ts$/.test(f)
  );
  const testChanged = changed.some(
    (f) => /\.(test|spec)\./.test(f) || /\/tests\//.test(f) || /\/e2e\//.test(f)
  );
  if (testable && !testChanged)
    advisory.push('changed logic without accompanying tests → run /add-tests');

  const open = [
    ...tier0.filter((t) => !(t.introduced_by_diff && t.severity === 'HIGH')),
    ...reEmitted.filter((f) => !reBlocking.includes(f)),
  ];
  const advisoryOut = [];
  if (open.length)
    advisoryOut.push(`Advisory findings (non-blocking):\n${open.map(fmt).join('\n')}`);
  if (advisory.length) advisoryOut.push(`Reminders:\n- ${advisory.join('\n- ')}`);
  logM({ outcome: 'tier0-only', blocked: false });
  if (advisoryOut.length)
    exit0(`✓ Review gate (tier-0, deterministic): no blocking issues.\n${advisoryOut.join('\n')}`);
  exit0();
} catch (e) {
  try {
    logM({
      outcome: 'error',
      blocked: false,
      error: 'gate_exception',
      message: String(e && e.message).slice(0, 300),
    });
  } catch {}
  process.exit(0);
}
