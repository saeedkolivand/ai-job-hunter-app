#!/usr/bin/env node
/**
 * review-gate.mjs — global Stop hook. Tiered, batched, token-efficient code review.
 * Generic by default; specialized per-project via <cwd>/.claude/review-routes.json + .claude/agents/*.md.
 * NEVER hard-fails the session: any error → exit 0 (don't block the user on a hook bug),
 * but every meaningful run — including failures — logs one line to .claude/.review-metrics.jsonl.
 *
 * Tiers: guards → skip-list → Tier 0 deterministic arch-guards → reviewed-hash cache →
 *        route to ≤3 owner checklists → ONE batched `claude -p` (schema-1 JSON findings) →
 *        deterministic verdict: block iff parsed HIGH/CRITICAL with confidence ≥ 0.6.
 * Scope: the full branch range (merge-base with origin/main → HEAD) PLUS the working
 *        tree and untracked files — committing no longer blinds the gate.
 */
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync, spawnSync } from 'node:child_process';
import {
  SKIP_GLOBS,
  matchesAny,
  globToRe,
  splitByFile,
  assembleDiff,
  hunkHashes,
  FINDING_CONTRACT,
  blockingFindings,
  formatFinding,
  countBySeverity,
  runClaudeReview,
  appendMetrics,
  readLearnings,
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

  // 3. skip-list (no LLM)
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
  const { diff, omitted, deletedCount } = assembleDiff(segments, MAX);
  if (!diff.trim()) exit0();

  // trivial-change heuristic (comment/import/blank-only → skip)
  const codeLines = diff.split('\n').filter((l) => /^[+-]/.test(l) && !/^[+-]{3}/.test(l));
  const meaningful = codeLines.filter((l) => {
    const b = l.slice(1).trim();
    if (!b) return false;
    if (/^(\/\/|\/\*|\*|#)/.test(b)) return false;
    if (/^(import |use |pub use |mod |from )/.test(b)) return false;
    return true;
  });
  if (!meaningful.length) {
    // deletion-only / over-budget-only changes carry no +/- lines — the review is
    // skipped as a degradation, but the metrics must not read as "clean".
    if (deletedCount || omitted.length) logM({ outcome: 'degraded', blocked: false });
    exit0();
  }

  // 5. Tier 0 — architecture guards (deterministic findings, confidence 1.0).
  // ponytail: JS regex over changed .rs contents; PR 3 swaps this for ast-grep.
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
  const tier0 = [];
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
      /std::env::var\b/,
      /\/platform\//,
      'std::env::var outside platform/',
      'move env access into platform/config.rs',
    ],
    [/reqwest::Client\b/, /\/net\//, 'reqwest::Client outside net/', 'use net/http.rs shared()'],
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

  // 6. reviewed-hash cache (body-only hunk hashes; line-number agnostic)
  const cachePath = path.join(cwd, '.claude', '.review-cache');
  let cache = new Set();
  try {
    cache = new Set(fs.readFileSync(cachePath, 'utf8').split('\n').filter(Boolean));
  } catch {}
  const hashes = hunkHashes(diff);
  // pre-existing (non-blocking) tier-0 hits must not defeat the cache forever
  const tier0Blocking = tier0.some((t) => t.introduced_by_diff);
  if (hashes.length && hashes.every((h) => cache.has(h)) && !tier0Blocking) {
    logM({ outcome: 'cache-skip', blocked: false });
    exit0();
  }

  // an INTRODUCED tier-0 hit is deterministic (confidence 1) — the verdict is
  // already known, so don't spend an LLM review on it; the fixed code gets its
  // full review on the next finish.
  if (tier0Blocking) {
    metric.model = 'none';
    metric.findings = countBySeverity(tier0);
    logM({ outcome: 'tier0-block', blocked: true });
    block(
      `Review gate [tier-0 arch guards] — blocking issues, address then finish:\n\n${tier0
        .filter((t) => t.introduced_by_diff)
        .map(formatFinding)
        .join('\n')}`
    );
  }

  // 7. route → ≤cap owners (+ secondaries on risk)
  let routes = null;
  try {
    routes = JSON.parse(fs.readFileSync(path.join(cwd, '.claude', 'review-routes.json'), 'utf8'));
  } catch {}
  const cap = (routes && routes.cap) || 3;
  const owners = [];
  if (routes && routes.primary)
    for (const f of nonSkipped) {
      const r = routes.primary.find((rt) => globToRe(rt.glob).test(f));
      if (r && !owners.includes(r.owner)) owners.push(r.owner);
    }
  const secondaries = [];
  let securityMatched = false; // security globs MATCHED — independent of cap survival
  if (routes && routes.secondary)
    for (const s of routes.secondary) {
      if (!nonSkipped.some((f) => matchesAny(f, s.globs))) continue;
      if (s.owner === 'tauri-security-reviewer') securityMatched = true;
      if (!owners.includes(s.owner)) secondaries.push(s.owner);
    }
  // Reserve the security reviewer a slot BEFORE trimming primaries to the cap.
  let selected = owners.slice(0, securityMatched ? Math.max(0, cap - 1) : cap);
  if (securityMatched && !selected.includes('tauri-security-reviewer'))
    selected.push('tauri-security-reviewer');
  for (const s of secondaries) {
    if (selected.length >= cap) break;
    if (!selected.includes(s)) selected.push(s);
  }
  if (!selected.length) selected = ['rust-backend-architect'];

  // model by risk. sonnet across the board — haiku recall was the weakest link of
  // the only auto-firing reviewer. Escalation switch for security diffs:
  // const model = securityMatched ? 'opus' : 'sonnet';
  const model = 'sonnet';
  metric.model = model;

  // owner checklists — 12K TOTAL budget; drop whole trailing owners, never truncate text.
  // Owners with a shared checklist file load it INSTEAD of their agent persona —
  // .claude/review-checklists/*.md is the single source of truth shared with
  // pr-reviewer and the CI review job; agent files remain the fallback.
  const OWNER_CHECKLIST = {
    'frontend-reviewer': 'frontend',
    'rust-backend-architect': 'rust',
    'testing-reviewer': 'testing',
  };
  const agentDir = path.join(cwd, '.claude', 'agents');
  const checklistDir = path.join(cwd, '.claude', 'review-checklists');
  const CHECKLIST_BUDGET = 12000;
  const consulted = [];
  const notConsulted = [];
  let checklists = '';
  for (const name of selected) {
    let body = '';
    const domain = OWNER_CHECKLIST[name];
    if (domain) {
      try {
        body = fs.readFileSync(path.join(checklistDir, domain + '.md'), 'utf8').trim();
      } catch {}
    }
    if (!body) {
      try {
        body = fs
          .readFileSync(path.join(agentDir, name + '.md'), 'utf8')
          .replace(/^---[\s\S]*?---/, '')
          .trim();
      } catch {}
    }
    const entry = `### ${name}\n${body}\n\n`;
    if (checklists.length + entry.length > CHECKLIST_BUDGET && consulted.length) {
      notConsulted.push(name);
      continue;
    }
    checklists += entry;
    consulted.push(name);
  }
  if (notConsulted.length)
    checklists += `(not consulted, over budget: ${notConsulted.join(', ')})\n`;

  // lessons retrieval by touched domain (proactive, folded into the same prompt)
  let lessons = '';
  try {
    const lp = path.join(cwd, '.claude', 'hooks', 'lessons.mjs');
    if (fs.existsSync(lp) && routes && routes.lessons_domains) {
      const domains = Object.keys(routes.lessons_domains).filter((d) =>
        nonSkipped.some((f) => matchesAny(f, routes.lessons_domains[d]))
      );
      const out = [];
      for (const d of domains.slice(0, 3)) {
        const r = spawnSync(process.execPath, [lp, 'query', '--domain', d, '--limit', '4'], {
          cwd,
          encoding: 'utf8',
        });
        if (r.stdout && r.stdout.trim()) out.push(r.stdout.trim());
      }
      if (out.length)
        lessons = '\n\n## Relevant prior lessons (consult, do not just repeat)\n' + out.join('\n');
    }
  } catch {}

  // learnings — known repo false positives (memory unification: the gate now reads
  // the same store /review and pr-reviewer use)
  const learnings = readLearnings(cwd, 4000);
  const learningsBlock = learnings
    ? `\n\n## Known repo false positives — do NOT re-raise any of these\n${learnings}`
    : '';

  // 8. Tier 1 — ONE batched claude -p, schema-1 JSON findings
  const prompt = `You are a STRICT but calibrated code reviewer. Review ONLY the diff below, applying the relevant reviewer checklists.

## Severity rubric (STRICT — verify against the diff; do not assume coverage or key existence)
- CRITICAL: exploitable security on a secret/credential/IPC/updater/network-egress path; data loss/corruption; breaks a release or CI gate.
- HIGH: architecture-rule violation (std::env::var outside platform/, reqwest::Client outside net/, untyped Result<_,String> outside error/); changed non-trivial logic shipped WITHOUT a test, or a test whose assertion is weak/tautological/asserts the mock/doesn't exercise the change; untested error/edge/security path on changed code; provider-specific coupling in business logic; PII/temp-file-cleanup/retention regression; user-facing text whose i18n key is missing from en or de (or a t() referencing a non-existent key).
- MEDIUM: unguarded hot-path perf regression, non-blocking correctness smell, a missing NON-critical edge-case test.
- LOW: style/naming/comments/formatting/docs.
STRICT tie-break: round UP for test-coverage, error/edge-path, i18n, security, and data findings; round down only for pure style/naming/docs.

${FINDING_CONTRACT}

## Reviewer checklists (apply only those whose area the diff touches)
${checklists}${lessons}${learningsBlock}

## Diff
\`\`\`diff
${diff}
\`\`\`
`;
  const r = runClaudeReview({ cwd, prompt, model });
  metric.parse_retries = r.parseRetries;

  // 9. deterministic verdict — from parsed findings, never from prose
  let findings = [...tier0];
  let fallbackNote = '';
  if (r.findings) {
    findings.push(...r.findings);
  } else if (r.raw && r.parseFailed) {
    // conservative fallback: unparseable output that talks about HIGH/CRITICAL blocks
    if (/\b(HIGH|CRITICAL)\b/.test(r.raw) && !/^APPROVED/i.test(r.raw)) {
      findings.push({
        severity: 'HIGH',
        category: 'correctness',
        file: '(unparsed reviewer output)',
        line: 0,
        summary: 'reviewer flagged HIGH/CRITICAL but violated the output contract',
        evidence: r.raw.slice(0, 1500),
        fix: 'read the raw finding below and address it',
        confidence: 1,
        introduced_by_diff: true,
      });
      fallbackNote = `\n\nRaw reviewer output (contract violation):\n${r.raw.slice(0, 2000)}`;
    }
  }

  const blocking = blockingFindings(findings, 0.6);
  const unverified = findings.filter(
    (f) => (f.severity === 'HIGH' || f.severity === 'CRITICAL') && !blocking.includes(f)
  );
  const lowmed = findings.filter((f) => f.severity === 'MEDIUM' || f.severity === 'LOW');
  metric.findings = countBySeverity(findings);
  if (r.parseFailed) metric.parse_failed = true;

  // advisories (non-blocking)
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

  if (blocking.length) {
    // Do NOT cache blocking hunks. Unfixed HIGH/CRITICAL must be re-reviewed and
    // re-blocked on the next finish until it is actually fixed — no whitelist-on-block.
    logM({ outcome: 'blocked', blocked: true });
    block(
      `Review gate [${consulted.join(', ')}] — blocking issues, address then finish:\n\n${blocking
        .map(formatFinding)
        .join('\n')}` +
        (unverified.length
          ? `\n\nNon-blocking HIGH/CRITICAL (low confidence or pre-existing):\n${unverified.map(formatFinding).join('\n')}`
          : '') +
        (advisory.length ? `\n\nAdvisory:\n- ${advisory.join('\n- ')}` : '') +
        fallbackNote
    );
  }

  // Fail-open, but visibly — and never cache hunks the model did not actually review.
  if (r.error === 'llm_unavailable') {
    logM({ outcome: 'llm-unavailable', blocked: false, error: r.error });
    exit0('review-gate: LLM review skipped (reviewer unavailable) — diff NOT cached');
  }
  if (r.parseFailed) {
    logM({ outcome: 'parse-failed', blocked: false });
    exit0('review-gate: reviewer output unparseable (no HIGH/CRITICAL text) — diff NOT cached');
  }

  // No blocking findings → record reviewed hunks so a future finish skips this clean code.
  try {
    fs.mkdirSync(path.dirname(cachePath), { recursive: true });
    fs.appendFileSync(cachePath, hashes.join('\n') + '\n');
    // bound growth: keep only the most recent ~2000 hashes
    const lines = fs.readFileSync(cachePath, 'utf8').split('\n').filter(Boolean);
    if (lines.length > 2000) fs.writeFileSync(cachePath, lines.slice(-2000).join('\n') + '\n');
  } catch {}

  const advisoryOut = [];
  if (unverified.length)
    advisoryOut.push(
      `Non-blocking HIGH/CRITICAL (low confidence or pre-existing):\n${unverified.map(formatFinding).join('\n')}`
    );
  if (lowmed.length)
    advisoryOut.push(`Advisory findings:\n${lowmed.map(formatFinding).join('\n')}`);
  if (advisory.length) advisoryOut.push(`Reminders:\n- ${advisory.join('\n- ')}`);
  logM({ outcome: advisoryOut.length ? 'advisory' : 'clean', blocked: false });
  if (advisoryOut.length) exit0(`✓ Review gate: no blocking issues.\n${advisoryOut.join('\n')}`);
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
