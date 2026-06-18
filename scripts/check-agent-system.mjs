// Drift guard for the `.claude/` agent fleet + its docs + the other AI-tool configs,
// so the agent system can never silently fall out of sync with reality (sibling of
// check-landing-drift.mjs — "never drift from reality").
//
// The agent definitions, routing, knowledge index, and the parallel AI-assistant rule
// files (aider/cursor/copilot/…) all encode factual claims. Nothing regenerates them,
// so when the fleet or conventions move, they rot (cf. the aider prompt that still
// described the removed Electron app). This validator reads them as data and fails when
// a claim no longer matches the live tree.
//
// Checks:
//   1. Stale tokens — dead import/package/arch references in .claude, docs, AI configs.
//   2. ADR index ↔ files — README lists exactly the ADRs on disk.
//   3. Routes ↔ agents — every routed owner has an agent file (and the fallback exists).
//   4. Agents ↔ CLAUDE.md — every agent appears in the project CLAUDE.md routing table.
//   5. Author/critic pairs — each declared author + its independent critic both exist.
//   6. Explainer complete — landing/agent-system.html exists and has a card per agent.
//   7. AI configs → CLAUDE.md — each parallel rule file points at CLAUDE.md (single source).
//
// Deferred (local-only / future, kept out of CI to stay dependency-free): codegraph
// symbol-resolution for dead doc pointers, and `Last updated:` vs git-mtime staleness.
//
// Read-only. Run via `pnpm check:agent-system`; wired into .husky/pre-push + quality.yml.

import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = process.cwd();

const AGENTS_DIR = '.claude/agents';
const ADR_DIR = 'docs/knowledge/decision-records';
const KNOWLEDGE_README = 'docs/knowledge/README.md';
const CLAUDE_MD = 'CLAUDE.md';
const ROUTES = '.claude/review-routes.json';
const EXPLAINER = 'landing/agent-system.html';

// Author → its independent critic (the writer never approves its own work).
const PAIRS = [
  ['rust-backend-author', 'rust-backend-architect'],
  ['frontend-author', 'frontend-reviewer'],
  ['job-match-author', 'job-match-expert'],
  ['ai-provider-author', 'ai-provider-expert'],
  ['scraping-applier-author', 'scraping-applier-expert'],
  ['test-author', 'testing-reviewer'],
  ['code-quality-author', 'code-quality-reviewer'],
  ['pdf-docx-generator', 'resume-export-expert'],
];

// Parallel AI-assistant rule files that must defer to CLAUDE.md as the single source.
const AI_CONFIGS = [
  '.aider/system-prompt.md',
  '.github/copilot-instructions.md',
  '.windsurfrules',
  '.clinerules',
  '.codexrules',
  '.roorules',
  '.jba/guidelines.md',
  'AGENTS.md',
];
// AI-adjacent files scanned for stale tokens only (no CLAUDE.md-pointer requirement).
const TOKEN_SCAN_EXTRA = ['.aider.conf.yml', '.coderabbit.yaml', 'CONTRIBUTING.md'];

// Old import paths — these are code examples that must be current, so scan EVERYWHERE (incl. docs).
const IMPORT_TOKENS = [
  /@\/lib\/i18n\b/, // → @ajh/translations (shim @/i18n)
  /@\/lib\/motion\b/, // → transition from @ajh/ui
];
// Electron-era arch/package names — only drift when a file claims them as the CURRENT stack, so scan
// .claude + AI configs only (docs/ legitimately narrate the migration history away from them).
const ARCH_TOKENS = [
  /\bapps\/desktop\b/, // Electron-era app dir → apps/tauri
  /@ajh\/(core|ai|data)\b/, // Electron-era packages → tauri Rust core / @ajh/ui|translations|prompts
  /\bElectron\b/, // the app is Tauri now
];

const failures = [];
const fail = (check, file, detail) => failures.push({ check, file, detail });

const read = (rel) => readFileSync(join(ROOT, rel), 'utf8');
const exists = (rel) => existsSync(join(ROOT, rel));

/** Recursively collect files under `rel` matching `test(name)`, skipping `skip` dirs. */
function walk(rel, test, skip = []) {
  const out = [];
  const abs = join(ROOT, rel);
  if (!existsSync(abs)) return out;
  for (const entry of readdirSync(abs, { withFileTypes: true })) {
    if (entry.name.startsWith('.') && entry.isDirectory()) continue;
    const childRel = `${rel}/${entry.name}`;
    if (entry.isDirectory()) {
      if (skip.includes(entry.name)) continue;
      out.push(...walk(childRel, test, skip));
    } else if (test(entry.name)) {
      out.push(childRel);
    }
  }
  return out;
}

const agentNames = () =>
  exists(AGENTS_DIR)
    ? readdirSync(join(ROOT, AGENTS_DIR))
        .filter((f) => f.endsWith('.md'))
        .map((f) => f.replace(/\.md$/, ''))
    : [];

// ── Check 1: stale tokens ────────────────────────────────────────────────────
function checkStaleTokens() {
  const md = (n) => n.endsWith('.md');
  const files = [
    ...walk('.claude/agents', md),
    ...walk('.claude/skills', (n) => n.endsWith('.md')),
    ...walk('.claude/commands', md),
    ...walk('docs', md, ['graphify-out']),
    ...walk('.aider', md),
    ...walk('.cursor', (n) => n.endsWith('.mdc')),
    ...AI_CONFIGS.filter(exists),
    ...TOKEN_SCAN_EXTRA.filter(exists),
  ];
  const describesCurrentStack = (f) => !f.startsWith('docs/'); // docs narrate history; .claude/configs describe today
  for (const file of [...new Set(files)]) {
    const text = read(file);
    for (const re of IMPORT_TOKENS) {
      const m = text.match(re);
      if (m) fail('Stale import path', file, `references '${m[0]}' — a removed import path`);
    }
    if (describesCurrentStack(file)) {
      for (const re of ARCH_TOKENS) {
        const m = text.match(re);
        if (m)
          fail('Stale architecture', file, `claims '${m[0]}' as current — the app is Tauri now`);
      }
    }
  }
}

// ── Check 2: ADR index ↔ files ───────────────────────────────────────────────
function checkAdrIndex() {
  if (!exists(ADR_DIR) || !exists(KNOWLEDGE_README)) return;
  const onDisk = readdirSync(join(ROOT, ADR_DIR))
    .filter((f) => /^adr-\d+.*\.md$/.test(f))
    .map((f) => f.replace(/\.md$/, ''));
  const readme = read(KNOWLEDGE_README);
  for (const adr of onDisk) {
    if (!readme.includes(adr)) {
      fail('ADR index drift', KNOWLEDGE_README, `missing index entry for ${adr} (present on disk)`);
    }
  }
  // Linked-but-absent: any decision-records/adr-* link whose file is gone.
  for (const [, name] of readme.matchAll(/decision-records\/(adr-[\w-]+)\.md/g)) {
    if (!onDisk.includes(name)) {
      fail('ADR index drift', KNOWLEDGE_README, `links ${name} — no such file in ${ADR_DIR}/`);
    }
  }
}

// ── Check 3: routes ↔ agents ─────────────────────────────────────────────────
function checkRoutes() {
  if (!exists(ROUTES)) return fail('Routes', ROUTES, 'review-routes.json not found');
  const names = new Set(agentNames());
  let routes;
  try {
    routes = JSON.parse(read(ROUTES));
  } catch (e) {
    return fail('Routes', ROUTES, `invalid JSON: ${e.message}`);
  }
  const owners = new Set();
  for (const r of routes.primary || []) owners.add(r.owner);
  for (const s of routes.secondary || []) owners.add(s.owner);
  for (const owner of owners) {
    if (!names.has(owner)) {
      fail('Routes → agent', ROUTES, `owner '${owner}' has no .claude/agents/${owner}.md`);
    }
  }
}

// ── Check 4: agents ↔ CLAUDE.md ──────────────────────────────────────────────
function checkClaudeMd() {
  if (!exists(CLAUDE_MD)) return;
  const text = read(CLAUDE_MD);
  for (const name of agentNames()) {
    if (!text.includes(name)) {
      fail('Agent ∉ CLAUDE.md', CLAUDE_MD, `agent '${name}' is not listed in the routing table`);
    }
  }
}

// ── Check 5: author/critic pairs ─────────────────────────────────────────────
function checkPairs() {
  const names = new Set(agentNames());
  for (const [author, critic] of PAIRS) {
    if (!names.has(author))
      fail('Missing author', AGENTS_DIR, `expected ${author}.md (paired with ${critic})`);
    if (!names.has(critic))
      fail('Missing critic', AGENTS_DIR, `expected ${critic}.md (audits ${author})`);
  }
}

// ── Check 6: explainer complete ──────────────────────────────────────────────
function checkExplainer() {
  if (!exists(EXPLAINER)) {
    return fail('Explainer', EXPLAINER, 'landing/agent-system.html does not exist yet');
  }
  const html = read(EXPLAINER);
  for (const name of agentNames()) {
    if (!html.includes(name)) {
      fail('Explainer card', EXPLAINER, `no card/mention for agent '${name}'`);
    }
  }
}

// ── Check 7: AI configs → CLAUDE.md ──────────────────────────────────────────
function checkAiConfigs() {
  const configs = [...AI_CONFIGS, '.aider.conf.yml', ...walk('.cursor', (n) => n.endsWith('.mdc'))];
  for (const file of configs) {
    if (!exists(file)) continue;
    if (!/CLAUDE\.md/.test(read(file))) {
      fail(
        'AI config ∌ CLAUDE.md',
        file,
        'should defer to CLAUDE.md as the single source of truth'
      );
    }
  }
}

// ── Run ──────────────────────────────────────────────────────────────────────
checkStaleTokens();
checkAdrIndex();
checkRoutes();
checkClaudeMd();
checkPairs();
checkExplainer();
checkAiConfigs();

if (failures.length === 0) {
  console.log(
    '✓ agent system in sync (tokens, ADR index, routes, CLAUDE.md, pairs, explainer, AI configs)'
  );
  process.exit(0);
}

console.error(`✗ agent-system drift detected — ${failures.length} issue(s):\n`);
const byCheck = new Map();
for (const f of failures) {
  if (!byCheck.has(f.check)) byCheck.set(f.check, []);
  byCheck.get(f.check).push(f);
}
for (const [check, items] of byCheck) {
  console.error(`  ${check}:`);
  for (const { file, detail } of items) console.error(`    - ${file}: ${detail}`);
  console.error('');
}
console.error('Fix the agent definitions / routes / docs / AI configs above, then re-run.');
process.exit(1);
