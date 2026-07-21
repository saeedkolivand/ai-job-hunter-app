#!/usr/bin/env node
/**
 * ci-review-verdict.mjs — deterministic verdict for the "🤖 AI Review OK" required
 * check (ADR-0008). Reads the schema-1 findings JSON the review step wrote and
 * computes the exit code in JS — never from model prose.
 *
 * FAIL-OPEN on infra (file missing/unparseable → warn + exit 0: an action outage
 * must not freeze merges; ci-ok + pre-push + CodeRabbit still gate).
 * FAIL-CLOSED on findings (HIGH/CRITICAL at confidence ≥ 0.8 → exit 1).
 */
import fs from 'node:fs';
import {
  validateFindings,
  blockingFindings,
  formatFinding,
  countBySeverity,
} from '../.claude/hooks/review-lib.mjs';

const FILE = process.argv[2] || 'review-findings.json';
const summaryPath = process.env.GITHUB_STEP_SUMMARY;
const lines = [];

const summary = (md) => {
  lines.push(md);
  if (summaryPath) {
    try {
      fs.appendFileSync(summaryPath, md + '\n');
    } catch {
      /* fail-open: summary/parse failures must not crash the verdict */
    }
  }
  console.log(md);
};

const writeSummaryFile = () => {
  try {
    fs.writeFileSync('review-comment.md', lines.join('\n'));
  } catch {
    /* fail-open: file write failures must not crash the verdict */
  }
};

let findings = null;
try {
  findings = validateFindings(JSON.parse(fs.readFileSync(FILE, 'utf8')));
} catch {
  /* fail-open: summary/parse failures must not crash the verdict */
}

if (!findings) {
  console.log(
    `::warning::AI review produced no parseable verdict (${FILE} missing/invalid) — infra fail-open; ci-ok, pre-push and CodeRabbit still gate.`
  );
  summary('## 🤖 AI Review OK\n\n**Verdict: fail-open (infra)** — ⚠ no parseable findings file.');
  writeSummaryFile();
  process.exit(0);
}

// Presentation only — no table to pipe-escape for, just flatten embedded newlines.
const flatten = (t) => String(t || '').replace(/\r?\n/g, ' ');

// Expanded severity sections, in display order (LOW gets its own collapsed block below).
const SEVERITY_SECTIONS = [
  ['CRITICAL', '🔴 Critical'],
  ['HIGH', '🟠 High'],
  ['MEDIUM', '🟡 Medium'],
];

const blocking = blockingFindings(findings, 0.8);
const c = countBySeverity(findings);
summary(
  `## 🤖 AI Review ${blocking.length ? 'FAILED' : 'OK'}\n\n**Verdict: ${blocking.length ? `${blocking.length} blocking finding(s)` : 'no blocking findings'}** (blocking = HIGH/CRITICAL at confidence ≥ 0.8) — ${findings.length} finding(s): critical ${c.critical} · high ${c.high} · medium ${c.medium} · low ${c.low}.`
);

for (const [severity, heading] of SEVERITY_SECTIONS) {
  const group = findings.filter((f) => f.severity === severity);
  if (!group.length) continue;
  summary(`\n### ${heading}\n`);
  for (const f of group)
    summary(
      `**\`${f.file}:${f.line}\`**${blocking.includes(f) ? ' 🚫' : ''} — ${flatten(f.summary)}\n**Fix:** ${flatten(f.fix)}\n`
    );
}

const low = findings.filter((f) => f.severity === 'LOW');
if (low.length) {
  summary(`\n<details><summary>⚪ Low (${low.length}, advisory)</summary>\n`);
  for (const f of low)
    summary(`- **\`${f.file}:${f.line}\`** — ${flatten(f.summary)}. _Fix:_ ${flatten(f.fix)}`);
  summary('\n</details>');
}
writeSummaryFile();
if (blocking.length) {
  console.error(`✗ ${blocking.length} blocking finding(s) (HIGH/CRITICAL, confidence >= 0.8):`);
  blocking.forEach((f) => console.error('  ' + formatFinding(f)));
  process.exit(1);
}
console.log('✓ AI review: no blocking findings.');
