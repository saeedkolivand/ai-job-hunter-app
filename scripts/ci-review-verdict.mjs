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
const summary = (md) => {
  if (summaryPath) {
    try {
      fs.appendFileSync(summaryPath, md + '\n');
    } catch {
      /* fail-open: summary/parse failures must not crash the verdict */
    }
  }
  console.log(md);
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
  summary('## 🤖 AI Review OK\n\n⚠ No parseable findings file — fail-open (infra).');
  process.exit(0);
}

const blocking = blockingFindings(findings, 0.8);
const c = countBySeverity(findings);
summary(
  `## 🤖 AI Review ${blocking.length ? 'FAILED' : 'OK'}\n\n${findings.length} finding(s) — critical ${c.critical} · high ${c.high} · medium ${c.medium} · low ${c.low}\n`
);
if (findings.length) {
  summary('| severity | file:line | finding | fix |');
  summary('|---|---|---|---|');
  for (const f of findings)
    summary(
      `| ${f.severity}${blocking.includes(f) ? ' 🚫' : ''} | ${f.file}:${f.line} | ${f.summary.replace(/\|/g, '\\|')} | ${(f.fix || '').replace(/\|/g, '\\|')} |`
    );
}
if (blocking.length) {
  console.error(`✗ ${blocking.length} blocking finding(s) (HIGH/CRITICAL, confidence >= 0.8):`);
  blocking.forEach((f) => console.error('  ' + formatFinding(f)));
  process.exit(1);
}
console.log('✓ AI review: no blocking findings.');
