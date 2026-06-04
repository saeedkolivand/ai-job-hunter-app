import { readFileSync } from 'node:fs';

import { danger, message, warn } from 'danger';

// ──────────────────────────────────────────────────────────────────────────
// Lane Z — deterministic PR rules. No AI, no external calls. Reuses the owner
// routing in .claude/review-routes.json so the review-bot guidance matches the
// local agent system. Uses only warn()/message() (never fail()) → advisory.
// ──────────────────────────────────────────────────────────────────────────

interface PrimaryRoute {
  glob: string;
  owner: string;
}

interface SecondaryRoute {
  owner: string;
  globs: string[];
}

interface ReviewRoutes {
  primary: PrimaryRoute[];
  secondary: SecondaryRoute[];
  advisory: { docs_stale: string[] };
}

// Minimal glob matcher for the patterns used in review-routes.json (supports
// **, **/ and *). Anchored, full-path match against repo-relative paths.
function globToRegExp(glob: string): RegExp {
  const escaped = glob.replace(/[.+^${}()|[\]\\]/g, '\\$&'); // escape specials (keep * and /)
  // Single pass so the inserted regex (which contains *) is never re-scanned.
  const source = escaped.replace(/\*\*\/|\*\*|\*/g, (token) => {
    if (token === '**/') return '(?:.*/)?'; // **/ → optional any dirs
    if (token === '**') return '.*'; // **  → anything
    return '[^/]*'; // *   → any non-slash run
  });
  return new RegExp(`^${source}$`);
}

function matches(file: string, glob: string): boolean {
  return globToRegExp(glob).test(file);
}

const changed = [...danger.git.created_files, ...danger.git.modified_files];

let routes: ReviewRoutes | null = null;
try {
  routes = JSON.parse(readFileSync('.claude/review-routes.json', 'utf8')) as ReviewRoutes;
} catch {
  // review-routes.json missing or invalid — owner-routing rules are skipped.
}

// ── 1. Primary-owner hint ──────────────────────────────────────────────────
if (routes) {
  const owners = new Set<string>();
  for (const file of changed) {
    const hit = routes.primary.find((route) => matches(file, route.glob));
    if (hit) owners.add(hit.owner);
  }
  if (owners.size > 0) {
    const list = [...owners].map((owner) => `\`${owner}\``).join(', ');
    message(`👤 **Suggested review owners** (by changed paths): ${list}`);
  }
}

// ── 2. Missing tests for changed source ────────────────────────────────────
const TEST_GLOBS = ['**/*.test.*', '**/*.spec.*', 'apps/tauri/src-tauri/tests/**', '**/e2e/**'];
const SOURCE_GLOBS = [
  'apps/tauri/src-tauri/src/**',
  'apps/tauri/src/renderer/**',
  'packages/*/src/**',
];

const isTest = (file: string): boolean => TEST_GLOBS.some((glob) => matches(file, glob));
const isSource = (file: string): boolean =>
  !isTest(file) && SOURCE_GLOBS.some((glob) => matches(file, glob));

const changedSource = changed.filter(isSource);
const changedTests = changed.filter(isTest);

if (changedSource.length > 0 && changedTests.length === 0) {
  warn(
    'This PR changes source files but no test files (`*.test.*` / `*.spec.*` / `tests/**`). ' +
      'If the change touches testable logic, add or update tests.'
  );
}

// ── 3. Security-sensitive surface ──────────────────────────────────────────
if (routes) {
  const security = routes.secondary.find((route) => route.owner === 'tauri-security-reviewer');
  if (security) {
    const hits = changed.filter((file) => security.globs.some((glob) => matches(file, glob)));
    if (hits.length > 0) {
      const sample = hits
        .slice(0, 10)
        .map((file) => `- \`${file}\``)
        .join('\n');
      warn(
        'This PR touches security-sensitive paths — apply the `tauri-security-reviewer` ' +
          `checklist (capabilities, IPC, secrets, deps, updater):\n${sample}`
      );
    }
  }
}

// ── 4. IPC / contract drift ────────────────────────────────────────────────
if (routes) {
  const docsStale = routes.advisory?.docs_stale ?? [];
  const ipcHits = changed.filter((file) => docsStale.some((glob) => matches(file, glob)));
  if (ipcHits.length > 0) {
    message('🔗 IPC/contract surface changed — run `pnpm gen:ipc` and update docs if it changed.');
  }
}

// ── 5. PR hygiene ──────────────────────────────────────────────────────────
const pr = danger.github.pr;
const churn = pr.additions + pr.deletions;

if (churn > 600) {
  warn(`This is a large PR (~${churn} changed lines). Consider splitting it for easier review.`);
}

if (!pr.body || pr.body.trim().length < 20) {
  warn('The PR description is empty or very short — add context (what changed and why).');
}
