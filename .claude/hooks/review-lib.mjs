/**
 * review-lib.mjs — shared core for every AI-review surface (ADR-0008 program):
 * the Stop gate (review-gate.mjs), the pre-push gate (scripts/pre-push-review.mjs)
 * and the CI verdict script (scripts/ci-review-verdict.mjs).
 *
 * Owns the four contracts:
 *  1. Finding schema v1 — the ONLY model output (one fenced ```json block).
 *  2. Deterministic verdict — block/pass computed in JS from parsed findings,
 *     never from model prose (`blockingFindings`).
 *  3. Parse contract — last fenced json block → validate → ONE corrective retry
 *     → conservative regex fallback (`runClaudeReview`).
 *  4. Metrics — one JSONL line per meaningful run (`appendMetrics`).
 */
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';

// ─── globs ───────────────────────────────────────────────────────────────────

// Double-star prefix globs also match root files; u0001/u0002 are placeholders.
export const globToRe = (g) =>
  new RegExp(
    '^' +
      g
        .replace(/[.+^${}()|[\]\\]/g, '\\$&')
        .replace(/\*\*\//g, '')
        .replace(/\*\*/g, '')
        .replace(/\*/g, '[^/]*')
        .replace(//g, '(?:.*/)?')
        .replace(//g, '.*') +
      '$'
  );

export const matchesAny = (file, globs) => (globs || []).some((g) => globToRe(g).test(file));

/** Files no review surface should spend tokens on. */
export const SKIP_GLOBS = [
  'docs/knowledge/**',
  '**/*.md',
  '**/*.lock',
  '**/pnpm-lock.yaml',
  '**/package-lock.json',
  '**/Cargo.lock',
  '**/*.snap',
  '**/snapshots/**',
  '**/golden/**',
  '**/*.gen.*',
  '**/dist/**',
  '**/build/**',
  '**/target/**',
  'graphify-out/**',
];

// ─── diff assembly (drop-order, never cut inside a hunk) ─────────────────────

export const classifyFile = (f) => {
  if (/\.(test|spec)\.|\/tests?\/|\/e2e\/|\/__tests__\//.test(f)) return 'test';
  if (/\.(md|mdx|txt)$|^docs\//.test(f)) return 'docs';
  return 'source';
};

/**
 * Split one unified-diff blob into per-file segments.
 * Returns [{ file, text, adds, dels, deleted }].
 */
export const splitByFile = (blob) => {
  const segs = [];
  if (!blob) return segs;
  const parts = blob.split(/^(?=diff --git )/m).filter((p) => p.startsWith('diff --git '));
  for (const text of parts) {
    // `b/<path>` is authoritative (rename-aware); fall back to a/<path> for deletions.
    const m = /^diff --git "?a\/([^"\n]+)"? "?b\/([^"\n]+)"?$/m.exec(text);
    const file = m ? m[2] || m[1] : '';
    if (!file) continue;
    const adds = (text.match(/^\+(?!\+\+)/gm) || []).length;
    const dels = (text.match(/^-(?!--)/gm) || []).length;
    const deleted = /^deleted file mode /m.test(text);
    segs.push({ file, text, adds, dels, deleted });
  }
  return segs;
};

/**
 * Assemble per-file segments into one bounded diff, by priority: source → test →
 * docs (PR-Agent drop-order). Deletions and over-budget files degrade to
 * one-line notes; a single over-budget segment is cut at hunk boundaries only.
 * Returns { diff, omitted } — omitted = ['file (+a/-b)'].
 */
export const assembleDiff = (segments, max = 60000) => {
  const order = { source: 0, test: 1, docs: 2 };
  const segs = [...segments].sort(
    (a, b) => order[classifyFile(a.file)] - order[classifyFile(b.file)]
  );
  let diff = '';
  const omitted = [];
  let deletedCount = 0;
  for (const s of segs) {
    if (s.deleted) {
      deletedCount += 1;
      diff += `# deleted: ${s.file} (-${s.dels} lines)\n`;
      continue;
    }
    const room = max - diff.length;
    if (s.text.length <= room) {
      diff += s.text;
      continue;
    }
    // Doesn't fit whole — keep the longest prefix of WHOLE hunks if there is
    // real room left, otherwise degrade the file to a one-line note.
    if (room > 4000) {
      const starts = [...s.text.matchAll(/^@@.*$/gm)].map((h) => h.index);
      let cut = 0; // cut BEFORE hunk i ⇒ keep hunks 0..i-1 fully
      for (let i = 1; i < starts.length; i++) {
        if (starts[i] <= room) cut = starts[i];
        else break;
      }
      const keep = cut > 0 ? s.text.slice(0, cut) : '';
      if (/^@@/m.test(keep)) {
        diff += keep + `# …remaining hunks of ${s.file} omitted (+${s.adds}/-${s.dels} total)\n`;
        continue;
      }
    }
    omitted.push(`${s.file} (+${s.adds}/-${s.dels})`);
  }
  if (omitted.length) diff += `# omitted files (over budget): ${omitted.join(', ')}\n`;
  return { diff, omitted, deletedCount };
};

// ─── hunk hashing (body-only, line-number agnostic) ──────────────────────────

export const hunkBodies = (diff) =>
  diff
    .split(/^@@.*$/m)
    .slice(1)
    .map((h) =>
      h
        .split('\n')
        .filter((l) => /^[+-]/.test(l) && !/^[+-]{3}/.test(l))
        .map((l) => l.slice(1).trim())
        .join('\n')
    );

export const hunkHashes = (diff) =>
  hunkBodies(diff).map((b) => crypto.createHash('sha1').update(b).digest('hex'));

// ─── finding schema v1 + deterministic verdict ───────────────────────────────

export const SEVERITIES = ['CRITICAL', 'HIGH', 'MEDIUM', 'LOW'];

/** The output contract given verbatim to every review model. */
export const FINDING_CONTRACT = `## Output contract (schema 1 — MANDATORY)
Reply with EXACTLY ONE fenced \`\`\`json block and nothing else:
\`\`\`json
{ "schema": 1, "findings": [ {
  "severity": "CRITICAL|HIGH|MEDIUM|LOW",
  "category": "security|correctness|data-loss|arch|test-coverage|i18n|perf|style",
  "file": "repo/relative/path", "line": 42,
  "summary": "one sentence: the defect",
  "evidence": "why it is real: traced path / quoted rule / constructed triggering input",
  "fix": "one-line fix",
  "confidence": 0.85,
  "introduced_by_diff": true } ] }
\`\`\`
Rules: \`confidence\` is your calibrated probability (0-1) that the finding is real AND correctly
severity-ranked — anything you cannot substantiate from the diff + checklists gets <= 0.5.
\`introduced_by_diff\` is false ONLY for pre-existing defects in unchanged code. No findings ->
\`{ "schema": 1, "findings": [] }\`. No prose outside the json block.`;

/** Shape-validate + normalize a parsed candidate. Returns findings[] or null. */
export const validateFindings = (obj) => {
  if (!obj || typeof obj !== 'object' || !Array.isArray(obj.findings)) return null;
  const out = [];
  for (const f of obj.findings) {
    if (!f || typeof f !== 'object') return null;
    const severity = String(f.severity || '').toUpperCase();
    if (!SEVERITIES.includes(severity)) return null;
    if (typeof f.file !== 'string' || typeof f.summary !== 'string') return null;
    out.push({
      severity,
      category: typeof f.category === 'string' ? f.category : 'correctness',
      file: f.file,
      line: Number.isFinite(Number(f.line)) ? Number(f.line) : 0,
      summary: f.summary,
      evidence: typeof f.evidence === 'string' ? f.evidence : '',
      fix: typeof f.fix === 'string' ? f.fix : '',
      confidence: Number.isFinite(Number(f.confidence))
        ? Math.max(0, Math.min(1, Number(f.confidence)))
        : 1,
      introduced_by_diff: f.introduced_by_diff !== false,
    });
  }
  return out;
};

/** Extract findings from model stdout: LAST fenced json block wins. */
export const parseFindings = (stdout) => {
  if (!stdout) return null;
  const blocks = [...stdout.matchAll(/```json\s*([\s\S]*?)```/g)];
  for (let i = blocks.length - 1; i >= 0; i--) {
    try {
      const v = validateFindings(JSON.parse(blocks[i][1]));
      if (v) return v;
    } catch {}
  }
  // tolerate a bare JSON reply without fences
  try {
    const v = validateFindings(JSON.parse(stdout.trim()));
    if (v) return v;
  } catch {}
  return null;
};

/**
 * The deterministic gate. Never derived from model prose (a model saying
 * "APPROVED" while listing a HIGH, or prose containing the word HIGH, must not
 * decide anything).
 */
export const blockingFindings = (findings, minConfidence) =>
  (findings || []).filter(
    (f) =>
      (f.severity === 'HIGH' || f.severity === 'CRITICAL') &&
      (f.confidence ?? 1) >= minConfidence &&
      f.introduced_by_diff !== false
  );

export const formatFinding = (f) =>
  `${f.severity} · ${f.file}${f.line ? ':' + f.line : ''} · ${f.summary}${f.fix ? ' · ' + f.fix : ''}`;

export const countBySeverity = (findings) => {
  const c = { critical: 0, high: 0, medium: 0, low: 0 };
  for (const f of findings || []) {
    const k = f.severity.toLowerCase();
    c[k] = (c[k] || 0) + 1;
  }
  return c;
};

// ─── claude -p driver (parse + one corrective retry) ─────────────────────────

/**
 * Run one non-interactive review. Returns
 * { findings, raw, parseRetries, parseFailed, error } — findings === null means
 * both parse attempts failed (caller decides the conservative fallback);
 * error is set when the binary itself was unavailable / timed out.
 */
export const runClaudeReview = ({ cwd, prompt, model, timeoutMs = 120000 }) => {
  const call = (p) => {
    try {
      const r = spawnSync('claude', ['-p', '--model', model, '--output-format', 'text'], {
        cwd,
        input: p,
        encoding: 'utf8',
        shell: true,
        env: { ...process.env, REVIEW_HOOK_ACTIVE: '1' },
        timeout: timeoutMs,
        maxBuffer: 10 * 1024 * 1024,
      });
      return (r.stdout || '').trim();
    } catch {
      return '';
    }
  };
  const first = call(prompt);
  if (!first)
    return {
      findings: null,
      raw: '',
      parseRetries: 0,
      parseFailed: false,
      error: 'llm_unavailable',
    };
  let findings = parseFindings(first);
  if (findings) return { findings, raw: first, parseRetries: 0, parseFailed: false, error: null };
  const second = call(
    prompt +
      '\n\n## CONTRACT VIOLATION\nYour previous reply was not a single valid schema-1 json block. Re-emit ONLY the findings JSON — one fenced ```json block, no prose.'
  );
  findings = parseFindings(second);
  return {
    findings,
    raw: second || first,
    parseRetries: 1,
    parseFailed: !findings,
    error: null,
  };
};

// ─── findings ledger (cross-run finding state; clean-hunk state stays in
// .review-cache — do not duplicate it here) ──────────────────────────────────

const LEDGER_MAX_LINES = 5000;

/** Per-file hunk hashes from diff segments: Map file → [sha1...]. */
export const fileHunkHashes = (segments) => {
  const m = new Map();
  for (const s of segments) m.set(s.file, [...(m.get(s.file) || []), ...hunkHashes(s.text)]);
  return m;
};

/** Stable identity for a finding across runs (summary rewording → new finding). */
export const ledgerKey = (f) =>
  `${f.file}|${f.category}|${crypto.createHash('sha1').update(f.summary).digest('hex').slice(0, 8)}`;

/**
 * Load the branch's ledger as Map(key → entry), last-status-wins.
 * Entry: { ts, branch, status: 'open'|'resolved-changed'|'suppressed',
 *          finding, fileHunks: [sha1...], reemits }.
 */
export const loadLedger = (cwd, branch) => {
  const m = new Map();
  try {
    const lines = fs
      .readFileSync(path.join(cwd, '.claude', '.review-ledger.jsonl'), 'utf8')
      .split('\n')
      .filter(Boolean);
    for (const line of lines) {
      try {
        const e = JSON.parse(line);
        if (e.branch === branch && e.finding) m.set(ledgerKey(e.finding), e);
      } catch {}
    }
  } catch {}
  return m;
};

/** True when a prior stop-gate run exists for this branch (convergence round ≥2). */
export const hasPriorRun = (cwd, branch) => {
  try {
    return fs
      .readFileSync(path.join(cwd, '.claude', '.review-metrics.jsonl'), 'utf8')
      .split('\n')
      .filter(Boolean)
      .some((l) => {
        try {
          const e = JSON.parse(l);
          return e.branch === branch && e.kind === 'stop-gate';
        } catch {
          return false;
        }
      });
  } catch {
    return false;
  }
};

/** Append ledger entries; trims to the most recent LEDGER_MAX_LINES. Never throws. */
export const appendLedger = (cwd, entries) => {
  if (!entries.length) return;
  try {
    const p = path.join(cwd, '.claude', '.review-ledger.jsonl');
    fs.mkdirSync(path.dirname(p), { recursive: true });
    fs.appendFileSync(
      p,
      entries.map((e) => JSON.stringify({ ts: new Date().toISOString(), ...e })).join('\n') + '\n'
    );
    const lines = fs.readFileSync(p, 'utf8').split('\n').filter(Boolean);
    if (lines.length > LEDGER_MAX_LINES)
      fs.writeFileSync(p, lines.slice(-LEDGER_MAX_LINES).join('\n') + '\n');
  } catch {}
};

// ─── metrics ─────────────────────────────────────────────────────────────────

const METRICS_MAX_LINES = 5000;

/** Append one metrics record; never throws. */
export const appendMetrics = (cwd, record) => {
  try {
    const p = path.join(cwd, '.claude', '.review-metrics.jsonl');
    fs.mkdirSync(path.dirname(p), { recursive: true });
    fs.appendFileSync(p, JSON.stringify({ ts: new Date().toISOString(), ...record }) + '\n');
    const lines = fs.readFileSync(p, 'utf8').split('\n').filter(Boolean);
    if (lines.length > METRICS_MAX_LINES)
      fs.writeFileSync(p, lines.slice(-METRICS_MAX_LINES).join('\n') + '\n');
  } catch {}
};

// ─── learnings (review-config.md — the FP memory pr-reviewer already uses) ───

/**
 * Return the learnings sections of .claude/review-config.md (everything from the
 * first learnings-ish heading to EOF), capped. '' when absent.
 */
export const readLearnings = (cwd, capChars = 4000) => {
  try {
    const txt = fs.readFileSync(path.join(cwd, '.claude', 'review-config.md'), 'utf8');
    const m = /^## (Learnings|Hard exclusions)[\s\S]*$/m.exec(txt);
    return m ? m[0].slice(0, capChars) : '';
  } catch {
    return '';
  }
};
