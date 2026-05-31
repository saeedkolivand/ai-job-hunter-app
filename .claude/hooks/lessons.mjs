#!/usr/bin/env node
// lessons.mjs — distilled experiential memory (CRUD). Only `project-steward` should WRITE;
// any agent may QUERY. Store: .claude/memory/lessons.jsonl (local, capped, archived on overflow).
// Usage:
//   node .claude/hooks/lessons.mjs add --category "<cat>" --context "..." --decision "..." --outcome "..." [--tags a,b]
//   node .claude/hooks/lessons.mjs query [--domain export|ats|scraping|ai|security|frontend|rust|testing] [--category X] [--text "..."] [--limit 8]
//   node .claude/hooks/lessons.mjs list | remove --text "..." | prune [--days 365]
import fs from 'node:fs';
import path from 'node:path';

const ROOT = process.env.CLAUDE_PROJECT_DIR || process.cwd();
const MEM = path.join(ROOT, '.claude', 'memory');
const FILE = path.join(MEM, 'lessons.jsonl');
const ARCHIVE = path.join(MEM, 'lessons.archive.jsonl');
const CAP = 200;

const CATEGORIES = [
  'Architecture decision',
  'Failed approach',
  'Proven approach',
  'Performance',
  'Security',
  'ATS',
  'Scraping',
  'AI-provider',
  'Export',
  'Testing discovery',
];
const DOMAIN_CATEGORIES = {
  export: ['Export', 'Performance'],
  ats: ['ATS'],
  scraping: ['Scraping', 'Performance'],
  ai: ['AI-provider', 'Performance'],
  security: ['Security'],
  frontend: ['Proven approach'],
  rust: ['Architecture decision', 'Proven approach', 'Failed approach'],
  testing: ['Testing discovery'],
};

const readAll = (file) => {
  try {
    return fs
      .readFileSync(file, 'utf8')
      .split('\n')
      .filter(Boolean)
      .map((l) => {
        try {
          return JSON.parse(l);
        } catch {
          return null;
        }
      })
      .filter(Boolean);
  } catch {
    return [];
  }
};
const writeAll = (file, arr) => {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, arr.map((o) => JSON.stringify(o)).join('\n') + (arr.length ? '\n' : ''));
};
const norm = (s) =>
  String(s || '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, ' ')
    .trim();
const fmt = (l) =>
  `- [${l.category}] Context: ${l.context} · Decision: ${l.decision} · Outcome: ${l.outcome}${l.tags && l.tags.length ? ` (tags: ${l.tags.join(',')})` : ''}`;
const parseArgs = (argv) => {
  const o = { _: [] };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a.startsWith('--')) {
      const k = a.slice(2);
      const v = argv[i + 1] && !argv[i + 1].startsWith('--') ? argv[++i] : 'true';
      o[k] = v;
    } else o._.push(a);
  }
  return o;
};

const [, , cmd, ...rest] = process.argv;
const args = parseArgs(rest);

try {
  if (cmd === 'add') {
    if (!CATEGORIES.includes(args.category)) {
      console.error(`add: --category must be one of: ${CATEGORIES.join(', ')}`);
      process.exit(1);
    }
    const lesson = {
      category: args.category,
      context: (args.context || '').trim(),
      decision: (args.decision || '').trim(),
      outcome: (args.outcome || '').trim(),
      tags: args.tags
        ? String(args.tags)
            .split(',')
            .map((s) => s.trim())
            .filter(Boolean)
        : [],
      created: new Date().toISOString(),
    };
    if (!lesson.context || !lesson.decision || !lesson.outcome) {
      console.error('add: --context, --decision, --outcome are required');
      process.exit(1);
    }
    const all = readAll(FILE);
    const ctxHead = norm(lesson.context).split(' ').slice(0, 4).join(' ');
    const dup = all.find(
      (l) =>
        l.category === lesson.category &&
        norm(l.context).split(' ').slice(0, 4).join(' ') === ctxHead &&
        norm(l.decision).includes(norm(lesson.decision).slice(0, 24))
    );
    if (dup) {
      console.log('skip: near-duplicate exists →', fmt(dup));
      process.exit(0);
    }
    all.push(lesson);
    if (all.length > CAP) {
      const overflow = all.splice(0, all.length - CAP);
      writeAll(ARCHIVE, readAll(ARCHIVE).concat(overflow));
    }
    writeAll(FILE, all);
    console.log('added:', fmt(lesson));
    process.exit(0);
  }

  if (cmd === 'query') {
    const all = readAll(FILE);
    let cats = null;
    if (args.domain) cats = DOMAIN_CATEGORIES[String(args.domain).toLowerCase()] || null;
    if (args.category) cats = [args.category];
    const text = args.text ? norm(args.text) : null;
    const limit = parseInt(args.limit || '8', 10);
    let res = all.filter((l) => {
      if (cats && !cats.includes(l.category)) return false;
      if (text) {
        const hay = norm(`${l.context} ${l.decision} ${l.outcome} ${(l.tags || []).join(' ')}`);
        if (!hay.includes(text)) return false;
      }
      return true;
    });
    res = res.slice(-limit);
    if (res.length) console.log(res.map(fmt).join('\n'));
    process.exit(0);
  }

  if (cmd === 'list') {
    const all = readAll(FILE);
    console.log(`${all.length} active lessons (cap ${CAP})`);
    if (all.length) console.log(all.map(fmt).join('\n'));
    process.exit(0);
  }

  if (cmd === 'remove') {
    const all = readAll(FILE);
    if (!args.text) {
      console.error('remove: --text required');
      process.exit(1);
    }
    const t = norm(args.text);
    const kept = all.filter((l) => !norm(`${l.context} ${l.decision} ${l.outcome}`).includes(t));
    writeAll(FILE, kept);
    console.log(`removed ${all.length - kept.length}`);
    process.exit(0);
  }

  if (cmd === 'prune') {
    const days = parseInt(args.days || '365', 10);
    const cutoff = Date.now() - days * 864e5;
    const all = readAll(FILE);
    const keep = all.filter((l) => new Date(l.created).getTime() >= cutoff);
    const old = all.filter((l) => new Date(l.created).getTime() < cutoff);
    if (old.length) writeAll(ARCHIVE, readAll(ARCHIVE).concat(old));
    writeAll(FILE, keep);
    console.log(`pruned ${old.length}, kept ${keep.length}`);
    process.exit(0);
  }

  console.log(`lessons.mjs — distilled experiential memory (only project-steward writes)
Usage:
  add    --category "<${CATEGORIES.join(' | ')}>" --context "..." --decision "..." --outcome "..." [--tags a,b]
  query  [--domain export|ats|scraping|ai|security|frontend|rust|testing] [--category X] [--text "..."] [--limit 8]
  list   |  remove --text "..."  |  prune [--days 365]`);
  process.exit(0);
} catch (e) {
  console.error('lessons.mjs error:', e && e.message ? e.message : e);
  process.exit(1);
}
