#!/usr/bin/env node
/**
 * pre-push-review.mjs — AI review gate for the push range (ADR-0008).
 *
 * Reads ranges from PREPUSH_RANGES (space-separated "A..B", accumulated by
 * .husky/pre-push's stdin loop — stdin is consumed there and cannot be re-read).
 *
 * Layers, cheapest first:
 *  1. cache fast-path — hunks the Stop gate already reviewed clean pass in <1s
 *  2. ast-grep deterministic scan — error-severity findings exit 1 from DAY ONE
 *  3. one sonnet schema-1 review — RATCHET: REVIEW_MODE=warn (default) prints
 *     loudly + exit 0 with a would_block metric; flip to block once
 *     /review-stats shows the FP rate is tolerable (~2 weeks)
 *
 * Fail-open on infra (no binary / timeout / double parse-fail) — CI is the
 * backstop. REVIEW_SKIP=1 skips but logs an AUDITED entry. Never bypass with
 * --no-verify: that skips the whole hook, this valve is visible.
 */
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync, spawnSync } from 'node:child_process';
import {
  SKIP_GLOBS,
  matchesAny,
  splitByFile,
  assembleDiff,
  keptHashes,
  fileHunkHashes,
  FINDING_CONTRACT,
  blockingFindings,
  formatFinding,
  countBySeverity,
  runClaudeReview,
  appendMetrics,
  readLearnings,
  loadLedger,
  appendLedger,
} from '../.claude/hooks/review-lib.mjs';

const cwd = process.cwd();
const t0 = Date.now();
const metric = { kind: 'pre-push', branch: '', model: '', files: 0 };
const logM = (extra) => appendMetrics(cwd, { ...metric, duration_ms: Date.now() - t0, ...extra });
const say = (s) => process.stdout.write(s + '\n');

const git = (args) =>
  execFileSync('git', args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });
try {
  metric.branch = git(['rev-parse', '--abbrev-ref', 'HEAD']).trim();
} catch {
  /* fail-open (ADR-0008): a hook bug must never block the push */
}

// audited escape hatch (REVIEW_SKIP=1 in .husky/pre-push routes here)
if (process.argv.includes('--log-skip')) {
  logM({ outcome: 'skipped', skipped: true, ranges: process.env.PREPUSH_RANGES || '' });
  process.exit(0);
}

const ranges = (process.env.PREPUSH_RANGES || '').trim().split(/\s+/).filter(Boolean);
const REVIEW_MODE = (process.env.REVIEW_MODE || 'warn').toLowerCase(); // ratchet default

try {
  if (!ranges.length) {
    say('⚠ AI review: no push range — skipping (CI is the backstop).');
    logM({ outcome: 'no-range', error: 'no_range' });
    process.exit(0);
  }

  // ── scope: the exact commits being pushed ──
  const segments = [];
  for (const range of ranges) {
    try {
      segments.push(...splitByFile(git(['diff', '-M', '--unified=3', range])));
    } catch {
      /* fail-open (ADR-0008): a hook bug must never block the push */
    }
  }
  const scoped = segments.filter((s) => !matchesAny(s.file, SKIP_GLOBS));
  if (!scoped.length) {
    say('✓ AI review: nothing reviewable in the push range.');
    logM({ outcome: 'clean', blocked: false });
    process.exit(0);
  }
  metric.files = new Set(scoped.map((s) => s.file)).size;

  // ── 1. cache fast-path (hunks the Stop gate already reviewed clean) ──
  const fileHunks = fileHunkHashes(scoped);
  const allHashes = [...fileHunks.values()].flat();
  let cache = new Set();
  try {
    cache = new Set(
      fs
        .readFileSync(path.join(cwd, '.claude', '.review-cache'), 'utf8')
        .split('\n')
        .filter(Boolean)
    );
  } catch {
    /* fail-open (ADR-0008): a hook bug must never block the push */
  }
  const ledger = loadLedger(cwd, metric.branch);
  const openBlocking = [...ledger.values()]
    .filter((e) => e.status === 'open' && e.finding)
    .map((e) => e.finding)
    .filter(
      (f) => (f.severity === 'HIGH' || f.severity === 'CRITICAL') && (f.confidence ?? 1) >= 0.6
    );
  if (allHashes.length && allHashes.every((h) => cache.has(h)) && !openBlocking.length) {
    say('✓ AI review: push range already reviewed clean.');
    logM({ outcome: 'cache-skip', blocked: false });
    process.exit(0);
  }
  if (openBlocking.length) {
    // unresolved blockers from previous reviews ride the deterministic lane
    say('✗ AI review: unresolved HIGH/CRITICAL findings from previous reviews:');
    openBlocking.forEach((f) => say('  ' + formatFinding(f)));
    logM({ outcome: 'reemit-block', blocked: true });
    process.exit(REVIEW_MODE === 'block' ? 1 : 0); // ledger re-emits ratchet too
  }

  // added (+) lines per file — everything in the push range is being pushed, but
  // pre-existing violations in touched files still must not block (CI owns them)
  const addedByFile = new Map();
  for (const s of scoped) {
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

  // ── 2. deterministic ast-grep layer (blocks from day one, even in warn mode) ──
  const detFindings = [];
  try {
    const files = [...new Set(scoped.map((s) => s.file))];
    const r = spawnSync(
      'pnpm',
      ['exec', 'ast-grep', 'scan', '--json=compact', '--', ...files.map((f) => `"${f}"`)],
      { cwd, encoding: 'utf8', shell: true, timeout: 60000, maxBuffer: 20 * 1024 * 1024 }
    );
    const outTxt = (r.stdout || '').trim();
    const matches = !r.error && r.status === 0 && !outTxt ? [] : JSON.parse(outTxt);
    for (const m of matches) {
      const file = String(m.file || '').replace(/\\/g, '/');
      if (m.severity !== 'error' || !introducedIn(file, m.text)) continue;
      detFindings.push({
        severity: 'HIGH',
        category: 'arch',
        file,
        line: (m.range && m.range.start && m.range.start.line + 1) || 0,
        summary: m.message || m.ruleId,
        evidence: `ast-grep rule ${m.ruleId}`,
        fix: m.note || '',
        confidence: 1,
        introduced_by_diff: true,
      });
    }
  } catch {
    metric.sg_fallback = true; // sg unavailable — the LLM layer still runs
  }
  if (detFindings.length) {
    say('✗ AI review: deterministic rule violations (blocking):');
    detFindings.forEach((f) => say('  ' + formatFinding(f)));
    logM({ outcome: 'tier0-block', blocked: true, findings: countBySeverity(detFindings) });
    process.exit(1);
  }

  // ── 3. one sonnet schema-1 review of the range ──
  const { diff, kept } = assembleDiff(scoped, 60000);
  const meaningful = diff
    .split('\n')
    .filter((l) => /^[+-]/.test(l) && !/^[+-]{3}/.test(l))
    .some((l) => {
      const b = l.slice(1).trim();
      return b && !/^(\/\/|\/\*|\*|#)/.test(b) && !/^(import |use |pub use |mod |from )/.test(b);
    });
  if (!meaningful) {
    say('✓ AI review: only trivial changes in the push range.');
    logM({ outcome: 'clean', blocked: false });
    process.exit(0);
  }
  const model = 'sonnet';
  metric.model = model;
  const learnings = readLearnings(cwd, 4000);
  const prompt = `You are a STRICT but calibrated pre-push code reviewer. Review ONLY the diff below — the exact commits about to be pushed. Report ONLY defects a reviewer would block a push for: correctness, security, data loss, contract breakage, untested error paths. No style commentary.

${FINDING_CONTRACT}
${learnings ? `\n## Known repo false positives — do NOT re-raise any of these\n${learnings}\n` : ''}
## Diff
\`\`\`diff
${diff}
\`\`\`
`;
  // no Stop-hook timeout constrains us here — give big pushes more headroom
  const r = runClaudeReview({ cwd, prompt, model, timeoutMs: 180000 });
  metric.parse_retries = r.parseRetries;
  if (r.error === 'llm_unavailable' || r.parseFailed) {
    say(
      `⚠ AI review: reviewer ${r.error ? 'unavailable' : 'output unparseable'} — passing (CI is the backstop).`
    );
    logM({
      outcome: r.error ? 'llm-unavailable' : 'parse-failed',
      blocked: false,
      error: r.error || undefined,
    });
    process.exit(0);
  }
  const blocking = blockingFindings(r.findings, 0.6);
  metric.findings = countBySeverity(r.findings);
  if (!blocking.length) {
    // seed the cache with ONLY what the model actually saw (kept), so the Stop gate
    // + the next push skip this clean range — omitted files are never marked clean
    try {
      const p = path.join(cwd, '.claude', '.review-cache');
      fs.mkdirSync(path.dirname(p), { recursive: true });
      fs.appendFileSync(p, keptHashes(kept).join('\n') + '\n');
    } catch {
      /* fail-open (ADR-0008): a hook bug must never block the push */
    }
    say('✓ AI review: push range clean.');
    logM({ outcome: 'clean', blocked: false });
    process.exit(0);
  }
  appendLedger(
    cwd,
    blocking
      .filter((f) => fileHunks.has(f.file))
      .map((f) => ({
        branch: metric.branch,
        status: 'open',
        source: 'pre-push',
        finding: f,
        fileHunks: fileHunks.get(f.file),
        reemits: 0,
      }))
  );
  say(`${REVIEW_MODE === 'block' ? '✗' : '⚠'} AI review findings (${REVIEW_MODE} mode):`);
  blocking.forEach((f) => say('  ' + formatFinding(f)));
  if (REVIEW_MODE === 'block') {
    logM({ outcome: 'blocked', blocked: true });
    say('Fix the findings (or REVIEW_SKIP=1 git push for an audited skip).');
    process.exit(1);
  }
  logM({ outcome: 'would-block', blocked: false, would_block: true, mode: 'warn' });
  say(
    '(warn mode — push proceeds; flip REVIEW_MODE=block after /review-stats shows a clean FP rate)'
  );
  process.exit(0);
} catch (e) {
  try {
    logM({
      outcome: 'error',
      error: 'prepush_exception',
      message: String(e && e.message).slice(0, 300),
    });
  } catch {
    /* fail-open (ADR-0008): a hook bug must never block the push */
  }
  say('⚠ AI review: internal error — passing (CI is the backstop).');
  process.exit(0);
}
