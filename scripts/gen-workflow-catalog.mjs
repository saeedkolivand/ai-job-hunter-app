// Generates the GitHub Actions catalog from the workflow files themselves, so the
// summary can never drift from reality:
//   1. .github/workflows/README.md  — a descriptive table (what each does, triggers, gate)
//      with a live status-badge grid.
//   2. README.md                    — the same badge grid, spliced between markers.
//
// Description text is read from each workflow's own leading comment block, so the
// single source of truth stays in the workflow file. Run with `pnpm gen:workflows`;
// CI enforces freshness via `pnpm gen:workflows:check` (regenerate + git diff).

import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { parse } from 'yaml';

const OWNER = 'saeedkolivand';
const REPO = 'ai-job-hunter-assistant-app';
const WORKFLOW_DIR = '.github/workflows';
const CATALOG_PATH = join(WORKFLOW_DIR, 'README.md');
const ROOT_README = 'README.md';
const BADGE_START = '<!-- workflows:badges:start -->';
const BADGE_END = '<!-- workflows:badges:end -->';

// The only workflow whose checks the branch ruleset requires (via its "✅ CI OK"
// umbrella). Everything else is advisory or reports to the Security tab. Keep this
// in sync with the ruleset's required_status_checks if it ever changes.
const GATING_FILES = new Set(['ci-pipeline.yml']);

/** Latest-run status badge for a workflow, linked to its runs page. */
function badge(file, name) {
  const base = `https://github.com/${OWNER}/${REPO}/actions/workflows/${file}`;
  return `[![${escapeAlt(name)}](${base}/badge.svg)](${base})`;
}

function escapeAlt(s) {
  return s.replace(/[[\]]/g, '');
}

/** Escape a cell for a Markdown table (pipes + collapse newlines). */
function cell(s) {
  return s
    .replace(/\|/g, '\\|')
    .replace(/\s*\n\s*/g, ' ')
    .trim();
}

/** Labels the structured-header style uses to separate sections. */
const HEADER_LABELS = /\s+(?:Triggers?|Jobs?|Source|Note|Notes|Outputs?|Lane|IMPORTANT)\b/i;

/**
 * One-line purpose, pulled from the file's leading `#` comment block (the lines
 * after `name:` up to the first blank line or real YAML). Box-drawing/divider
 * lines are dropped. Headers using the `Purpose: … Triggers: …` style yield the
 * Purpose clause; others yield the first sentence. Inline bullet lists are cut.
 */
function describe(raw) {
  const lines = raw.split(/\r?\n/);
  const collected = [];
  let seenName = false;
  for (const line of lines) {
    if (!seenName) {
      if (/^name:/.test(line)) seenName = true;
      continue;
    }
    const trimmed = line.trim();
    if (trimmed === '') {
      if (collected.length) break;
      continue;
    }
    if (!trimmed.startsWith('#')) break;
    const content = trimmed.replace(/^#+/, '').trim();
    if (content === '' || /^[─-╿\-=*_]+$/.test(content)) continue;
    collected.push(content);
  }
  let text = collected.join(' ').replace(/\s+/g, ' ').trim();
  if (!text) return '';

  // Drop an internal "Lane X —" tag prefix.
  text = text.replace(/^Lane\s+\S+\s*[—-]+\s*/i, '');

  const purpose = text.match(/Purpose:\s*(.+)/i);
  if (purpose) {
    // Structured header: keep the Purpose clause up to the next section label.
    text = purpose[1].split(HEADER_LABELS)[0].trim();
  } else {
    // Prose header: first sentence.
    text = text.split(/(?<=\.)\s/)[0];
  }

  // Cut an inline bullet-list tail ("foo: - a - b" → "foo").
  text = text.replace(/:\s*[-–]\s.*$/, '').trim();

  if (text.length > 160) text = `${text.slice(0, 159).replace(/\s+\S*$/, '')}…`;
  return text;
}

/** Human-readable trigger summary from a parsed `on:` node. */
function triggers(on) {
  if (on == null) return '—';
  const keys = typeof on === 'string' ? [on] : Array.isArray(on) ? on : Object.keys(on);
  const labels = [];
  if (keys.includes('pull_request') || keys.includes('pull_request_target')) labels.push('PR');
  if (keys.includes('push')) labels.push('push');
  if (keys.includes('schedule')) labels.push(scheduleLabel(on?.schedule));
  if (keys.includes('issue_comment') || keys.includes('pull_request_review_comment'))
    labels.push('comment');
  if (keys.includes('workflow_dispatch')) labels.push('manual');
  if (keys.includes('workflow_call')) labels.push('reusable');
  return labels.length ? [...new Set(labels)].join(', ') : keys.join(', ');
}

function scheduleLabel(schedule) {
  const cron = schedule?.[0]?.cron ?? '';
  const [, , dom, , dow] = cron.split(' ');
  if (dow && dow !== '*') return 'weekly';
  if (dom && dom !== '*') return 'monthly';
  return 'daily';
}

const SECURITY_FILES = new Set(['codeql.yml', 'semgrep.yml', 'scorecard.yml', 'security.yml']);
const DEPLOY_FILES = new Set(['release.yml', 'pages.yml']);

/** The workflow's role. Only `✅ required` gates merge; the rest never block. */
function role(file) {
  if (GATING_FILES.has(file)) return '✅ required';
  if (DEPLOY_FILES.has(file)) return 'deploy';
  if (SECURITY_FILES.has(file)) return 'security';
  return 'advisory';
}

function buildRows() {
  const files = readdirSync(WORKFLOW_DIR)
    .filter((f) => f.endsWith('.yml') || f.endsWith('.yaml'))
    .sort();

  return files.map((file) => {
    const raw = readFileSync(join(WORKFLOW_DIR, file), 'utf8');
    const doc = parse(raw);
    const name = String(doc?.name ?? file);
    return {
      file,
      name,
      description: describe(raw) || '—',
      triggers: triggers(doc?.on),
      role: role(file),
    };
  });
}

function renderBadges(rows) {
  return rows.map((r) => badge(r.file, r.name)).join('\n');
}

function renderCatalog(rows) {
  const header = [
    '<!-- AUTO-GENERATED by scripts/gen-workflow-catalog.mjs — do not edit by hand.',
    '     Update a workflow file (incl. its leading comment) then run `pnpm gen:workflows`. -->',
    '',
    '# ⚙️ GitHub Actions — workflow catalog',
    '',
    `${rows.length} workflows. Descriptions come from each workflow's own header comment.`,
    'Roles: **✅ required** is the only one that gates merge (CI Pipeline → its `✅ CI OK`',
    'umbrella check); **advisory** runs but never blocks; **security** reports to the',
    'Security tab; **deploy** publishes on push to `main`.',
    '',
    '## Status',
    '',
    renderBadges(rows),
    '',
    '## Catalog',
    '',
    '| Workflow | Triggers | Role | What it does |',
    '| --- | --- | --- | --- |',
  ];
  const body = rows.map(
    (r) =>
      `| [${cell(r.name)}](${r.file}) | ${cell(r.triggers)} | ${r.role} | ${cell(r.description)} |`
  );
  return `${[...header, ...body].join('\n')}\n`;
}

function spliceBadges(readme, rows) {
  const start = readme.indexOf(BADGE_START);
  const end = readme.indexOf(BADGE_END);
  if (start === -1 || end === -1) {
    throw new Error(
      `Markers ${BADGE_START} / ${BADGE_END} not found in ${ROOT_README}. ` +
        'Add them where the workflow badge grid should render.'
    );
  }
  const before = readme.slice(0, start + BADGE_START.length);
  const after = readme.slice(end);
  const grid = `\n\n${renderBadges(rows)}\n\n`;
  return `${before}${grid}${after}`;
}

const rows = buildRows();
writeFileSync(CATALOG_PATH, renderCatalog(rows));
writeFileSync(ROOT_README, spliceBadges(readFileSync(ROOT_README, 'utf8'), rows));
console.log(
  `Generated ${CATALOG_PATH} and refreshed badges in ${ROOT_README} (${rows.length} workflows).`
);
