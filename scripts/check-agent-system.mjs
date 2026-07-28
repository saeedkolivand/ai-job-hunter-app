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
//   6. Explainer complete — apps/landing/src/data/agent-fleet.ts exists and names every agent.
//   7. AI configs → AGENTS.md — AGENTS.md exists, CLAUDE.md imports it unfenced, and every
//      tool rule file is a thin pointer at it (single source) rather than a forked copy.
//   8. Route globs → tree — every glob's static prefix exists on disk (dead prefix = dead route).
//   9. Referenced agents → files — every agent named in the CLAUDE.md agent table or the
//      explainer roster has a matching .claude/agents/<name>.md (reverse of 4 & 6).
//   10. Model tiering — every agent's model: pins an explicit tier (opus/sonnet/haiku) —
//       never 'inherit' or a full model ID; the opus and haiku sets match CLAUDE.md's
//       "Model & effort tiering" paragraph; every opus agent pins effort: xhigh, no
//       sonnet/haiku agent does.
//
// Deferred (local-only / future, kept out of CI to stay dependency-free): codegraph
// symbol-resolution for dead doc pointers, and `Last updated:` vs git-mtime staleness.
//
// Read-only. Run via `pnpm check:agent-system`; wired into .husky/pre-push + ci-pipeline.yml.

import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = process.cwd();

const AGENTS_DIR = '.claude/agents';
const ADR_DIR = 'docs/knowledge/decision-records';
const KNOWLEDGE_README = 'docs/knowledge/README.md';
const CLAUDE_MD = 'CLAUDE.md';
const ROUTES = '.claude/review-routes.json';
// The agent-system explainer is now the typed data source behind the /agent-system
// route (ported from the former public/agent-system.html in PR2). Every agent name
// must still appear here (check 6), and its roster tuples feed the reverse-check
// (check 9).
const EXPLAINER = 'apps/landing/src/data/agent-fleet.ts';

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
  ['extension-author', 'extension-reviewer'],
  // webgl-author/shader-engineer/webgl-reviewer are dormant (.claude/dormant/agents/,
  // ADR-0017) — not part of the active roster this check enforces.
];

// The canonical rule files: AGENTS.md holds the portable rules every tool reads, CLAUDE.md
// imports it (`@AGENTS.md`) and adds Claude-only orchestration. Scanned for stale tokens, but
// exempt from the pointer checks below — they are the source, not deferrers to it.
const AGENTS_MD = 'AGENTS.md';
const CANONICAL_RULES = [AGENTS_MD, CLAUDE_MD];

// Tool-specific rule files that must stay thin pointers to AGENTS.md, never forked copies.
const AI_CONFIGS = [
  '.aider/system-prompt.md',
  '.github/copilot-instructions.md',
  '.windsurfrules',
  '.clinerules',
  '.codexrules',
  '.roorules',
  '.jba/guidelines.md',
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
  /@ajh\/(core|ai|data)\b/, // Electron-era packages → tauri Rust core / @ajh/ui|translations|prompts
  /\bElectron\b/, // the app is Tauri now (apps/desktop is the current Tauri app dir)
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
    ...CANONICAL_RULES.filter(exists),
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
    return fail('Explainer', EXPLAINER, `${EXPLAINER} does not exist yet`);
  }
  const source = read(EXPLAINER);
  for (const name of agentNames()) {
    if (!source.includes(name)) {
      fail('Explainer card', EXPLAINER, `no card/mention for agent '${name}'`);
    }
  }
}

// ── Check 7: AI configs → AGENTS.md ──────────────────────────────────────────
// AGENTS.md is canonical (every tool reads it, natively or via a pointer); CLAUDE.md
// imports it with `@AGENTS.md` and adds Claude-only orchestration. A tool file that
// restates rules instead of deferring is how the 13-copy drift started — so each one
// must name the canonical file, and stay short enough that it can't hide a forked ruleset.
const POINTER_MAX_BYTES = 1024;
// Real config files (settings, not rules) — they must still point at AGENTS.md, but
// growing past the pointer cap is legitimate for them.
const SIZE_EXEMPT = new Set(['.aider.conf.yml']);
// Claude loads AGENTS.md + CLAUDE.md in full every session, and adherence drops as they
// grow — Anthropic targets <200 lines. Capped here so the budget is enforced, not audited
// by hand; if a rule genuinely needs the room, move a section to .claude/rules/ with a
// `paths:` frontmatter so it loads only for matching files.
const CANONICAL_MAX_LINES = 200;

function checkAiConfigs() {
  // The whole rule set hangs on these two facts; nothing else asserts them. Without the
  // import, Claude Code loads CLAUDE.md's orchestration with zero coding rules while every
  // pointer file still truthfully says "read AGENTS.md" — a silent, total loss of rules.
  if (!exists(AGENTS_MD)) {
    fail(
      'Canonical rules missing',
      AGENTS_MD,
      `CLAUDE.md imports it — every tool's rule set is empty without it`
    );
  }
  // Import parsing skips code spans AND fenced blocks, so strip fences before testing —
  // `^` under /m matches inside a fence too, and a fenced import would otherwise pass green.
  // Inline backticks need no stripping: the line no longer starts with `@`.
  if (exists(CLAUDE_MD)) {
    // Match the FULL opening run (`{3,}) so a 3-tick inner fence can't close a 4-tick outer
    // one, and allow the 1-3 leading spaces CommonMark still treats as a fence.
    const importable = read(CLAUDE_MD).replace(/^ {0,3}(`{3,}|~{3,})[\s\S]*?^ {0,3}\1/gm, '');
    if (!/^@AGENTS\.md\s*$/m.test(importable)) {
      fail(
        'CLAUDE.md ∌ @AGENTS.md',
        CLAUDE_MD,
        'the import that loads the canonical rules is missing, fenced, or in backticks (imports skip code spans/blocks)'
      );
    }
  }

  const loadedLines = CANONICAL_RULES.filter(exists).reduce(
    (n, f) => n + read(f).split('\n').length,
    0
  );
  if (loadedLines > CANONICAL_MAX_LINES) {
    fail(
      'Canonical rules too long',
      CANONICAL_RULES.join(' + '),
      `${loadedLines} lines > ${CANONICAL_MAX_LINES} — loaded in full every session; trim or move a section to .claude/rules/ with paths: frontmatter`
    );
  }

  const configs = [...AI_CONFIGS, '.aider.conf.yml', ...walk('.cursor', (n) => n.endsWith('.mdc'))];
  for (const file of configs) {
    if (!exists(file)) continue;
    const text = read(file);
    if (!/AGENTS\.md/.test(text)) {
      fail(
        'AI config ∌ AGENTS.md',
        file,
        'should defer to AGENTS.md as the single source of truth'
      );
    }
    if (!SIZE_EXEMPT.has(file) && Buffer.byteLength(text, 'utf8') > POINTER_MAX_BYTES) {
      fail(
        'AI config too long',
        file,
        `${Buffer.byteLength(text, 'utf8')}B > ${POINTER_MAX_BYTES}B — a pointer, not a copy of the rules; edit AGENTS.md instead`
      );
    }
  }
}

// ── Check 8: route globs → tree ──────────────────────────────────────────────
// Every glob's static prefix (the part before the first wildcard) must exist on
// disk — a dead prefix means the route can never match anything.
function checkRouteGlobs() {
  if (!exists(ROUTES)) return;
  let routes;
  try {
    routes = JSON.parse(read(ROUTES));
  } catch {
    return; // invalid JSON already reported by checkRoutes
  }
  const globs = new Set();
  for (const r of routes.primary || []) globs.add(r.glob);
  for (const s of routes.secondary || []) (s.globs || []).forEach((g) => globs.add(g));
  for (const arr of Object.values(routes.advisory || {})) arr.forEach((g) => globs.add(g));
  for (const arr of Object.values(routes.lessons_domains || {})) arr.forEach((g) => globs.add(g));
  for (const g of globs) {
    const wild = g.search(/[*?[]/);
    // literal path → must exist as-is; wildcard → the dir part before the first wildcard must exist
    const prefix = (wild === -1 ? g : g.slice(0, wild).replace(/[^/]*$/, '')).replace(/\/+$/, '');
    if (prefix && !exists(prefix)) {
      fail('Route glob → tree', ROUTES, `glob '${g}' — static prefix '${prefix}' does not exist`);
    }
  }
}

// ── Check 9: referenced agents → files (reverse of 4 & 6) ────────────────────
function checkReferencedAgents() {
  const names = new Set(agentNames());
  const record = (name, where) => {
    if (!names.has(name)) {
      fail('Referenced agent missing', where, `references '${name}' — no ${AGENTS_DIR}/${name}.md`);
    }
  };
  if (exists(CLAUDE_MD)) {
    // the routing table: rows following the "| Touched area | Author | Critic(s) |" header
    const lines = read(CLAUDE_MD).split('\n');
    const start = lines.findIndex((l) => /^\|\s*Touched area/i.test(l));
    for (let i = start + 1; start !== -1 && i < lines.length && lines[i].startsWith('|'); i++) {
      for (const [, name] of lines[i].matchAll(/`([a-z][a-z0-9]*(?:-[a-z0-9]+)+)`/g)) {
        record(name, CLAUDE_MD);
      }
    }
  }
  if (exists(EXPLAINER)) {
    // the AGENT roster tuples: ['name', 'author'|'critic'|'cross', …]. The source
    // is now a prettier-formatted TS module, so tolerate the whitespace prettier
    // inserts around the array separators (the old HTML was space-free).
    for (const [, name] of read(EXPLAINER).matchAll(
      /\[\s*'([a-z][a-z0-9-]+)'\s*,\s*'(?:author|critic|cross)'/g
    )) {
      record(name, EXPLAINER);
    }
  }
}

// ── Check 10: model tiering ↔ CLAUDE.md policy ───────────────────────────────
// model: must pin an explicit tier (opus/sonnet/haiku) — never 'inherit' or a full model
// ID — so it auto-tracks the current family under that tier; the opus/haiku sets and the
// effort: high|xhigh pin must match the "Model & effort tiering" paragraph in CLAUDE.md
// (single source — read as data, not duplicated here).
function checkModelTiering() {
  if (!exists(CLAUDE_MD)) return;
  const policyLine = read(CLAUDE_MD)
    .split('\n')
    .find((l) => l.startsWith('**Model & effort tiering**'));
  if (!policyLine)
    return fail('Model tiering', CLAUDE_MD, 'no "Model & effort tiering" paragraph found');

  // Only backticked kebab-case tokens are agent names — excludes `effort: xhigh`/`high`/`low`
  // asides that also live inside these sentences.
  const backticked = (re) => {
    const m = policyLine.match(re);
    return new Set(
      m ? [...m[1].matchAll(/`([a-z][a-z0-9]*(?:-[a-z0-9]+)+)`/g)].map((g) => g[1]) : []
    );
  };
  const wantOpus = backticked(/Opus for last-line critics(.*?)\.\s+Sonnet for/);
  const wantHaiku = backticked(/Haiku for ([^.]+)\./);
  // the named last-line pair pins xhigh; every OTHER opus critic pins high
  const wantXhigh = backticked(/Opus for last-line critics(.*?)pin `effort: xhigh`/);

  const unquote = (v) => v?.replace(/^['"]|['"]$/g, '');
  const isDelim = (l) => /^---\s*$/.test(l);
  const gotOpus = new Set();
  const gotHaiku = new Set();
  const malformed = new Set(); // already reported below — exempt from the reverse-check
  for (const name of agentNames()) {
    const file = `${AGENTS_DIR}/${name}.md`;
    const raw = read(file);
    const lines = (raw.charCodeAt(0) === 0xfeff ? raw.slice(1) : raw).split('\n');
    const end = isDelim(lines[0]) ? lines.findIndex((l, i) => i > 0 && isDelim(l)) : -1;
    const frontmatter = end === -1 ? '' : lines.slice(1, end).join('\n');
    const model = unquote(frontmatter.match(/^model:\s*(\S+)/m)?.[1]);
    const effort = unquote(frontmatter.match(/^effort:\s*(\S+)/m)?.[1]);

    if (!['opus', 'sonnet', 'haiku'].includes(model)) {
      fail(
        'Model tiering',
        file,
        `model: '${model ?? '(missing)'}' — this repo pins explicit tiers (opus|sonnet|haiku); 'inherit'/full model IDs are not allowed`
      );
      malformed.add(name);
      continue;
    }
    if (model === 'opus') gotOpus.add(name);
    if (model === 'haiku') gotHaiku.add(name);
    // Opus effort pins follow the tiering sentence exactly: the named last-line pair
    // pins xhigh, every other opus critic pins high. CLAUDE.md's "default high
    // everywhere" / "low fine for steward runs" describe harness defaults, not
    // frontmatter to check.
    if (model === 'opus' && wantXhigh.size) {
      const want = wantXhigh.has(name) ? 'xhigh' : 'high';
      if (effort !== want)
        fail(
          'Model tiering',
          file,
          `model: opus requires effort: ${want} for ${name} (CLAUDE.md § Model & effort tiering)`
        );
    }
    if (model !== 'opus' && effort === 'xhigh')
      fail('Model tiering', file, `model: ${model} must not set effort: xhigh (opus-only)`);
  }

  // A broken anchor (wording moved in CLAUDE.md) means an empty want-set, which would
  // otherwise cascade into one confusing "not listed" failure per real agent below —
  // fail once, intelligibly, and skip the reverse-check instead.
  let anchorOk = true;
  if (wantOpus.size === 0) {
    fail(
      'Model tiering',
      CLAUDE_MD,
      'could not locate the Opus-critic list in CLAUDE.md § Model & effort tiering — wording changed? update the anchor regex'
    );
    anchorOk = false;
  }
  if (wantHaiku.size === 0) {
    fail(
      'Model tiering',
      CLAUDE_MD,
      'could not locate the Haiku list in CLAUDE.md § Model & effort tiering — wording changed? update the anchor regex'
    );
    anchorOk = false;
  }
  if (!anchorOk) return;

  for (const name of wantOpus)
    if (!gotOpus.has(name) && !malformed.has(name))
      fail('Model tiering', CLAUDE_MD, `'${name}' listed as an Opus critic but has no model: opus`);
  for (const name of gotOpus)
    if (!wantOpus.has(name))
      fail(
        'Model tiering',
        CLAUDE_MD,
        `'${name}' has model: opus but isn't listed as an Opus critic`
      );
  for (const name of wantHaiku)
    if (!gotHaiku.has(name) && !malformed.has(name))
      fail('Model tiering', CLAUDE_MD, `'${name}' expected model: haiku`);
  for (const name of gotHaiku)
    if (!wantHaiku.has(name))
      fail(
        'Model tiering',
        CLAUDE_MD,
        `'${name}' has model: haiku but isn't in the expected haiku set`
      );
}

// ── Run ──────────────────────────────────────────────────────────────────────
checkStaleTokens();
checkAdrIndex();
checkRoutes();
checkClaudeMd();
checkPairs();
checkExplainer();
checkAiConfigs();
checkRouteGlobs();
checkReferencedAgents();
checkModelTiering();

if (failures.length === 0) {
  console.log(
    '✓ agent system in sync (tokens, ADR index, routes, CLAUDE.md, pairs, explainer, AI configs, route globs, referenced agents, model tiering)'
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
