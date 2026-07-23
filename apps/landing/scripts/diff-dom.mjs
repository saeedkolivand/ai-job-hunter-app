// DOM-equivalence checker — proves a TSX-converted page renders the IDENTICAL
// body DOM as the pre-conversion baseline HTML (ADR 0018: the marketing skin
// is protected, ~2400 lines of vanilla JS in public/scripts/*.js bind to it by
// id/class/data-attr).
//
// Usage: node scripts/diff-dom.mjs <baseline.html> <new.html>
//
// What it checks: the <body> subtrees of both files, as normalized trees —
// lowercased tagName, sorted (name,value) attribute pairs, and normalized
// child sequence, recursively. It also asserts <new.html> links the
// self-hosted stylesheet (/fonts/fonts.css).
//
// What it deliberately SKIPS (and why): <style>/<script>/<link> elements
// (their counts/contents legitimately change page to page and build to
// build), comment nodes (React emits <!-- --> hydration separators the
// legacy HTML never had), and whitespace-only text nodes (raw HTML has
// indentation text nodes JSX never emits). KNOWN LIMITATION: because
// whitespace-only text is dropped, this tool cannot catch a regression in an
// inline word-gap (e.g. a dropped `{' '}` between two inline elements) —
// that class of bug is covered by explicit `{' '}` review in the diff plus a
// manual visual pass, not by this script.
//
// Node stdlib + jsdom only (already a devDependency of @ajh/landing).

import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

import { JSDOM } from 'jsdom';

// ── Normalize a DOM subtree into a plain comparable tree ─────────────────────
const SKIP_TAGS = new Set(['style', 'script', 'link']);

// Split a style attribute value on top-level `;` only — a `;` inside a
// quoted string (content: "a; b") or a url(...)/function argument must not
// split the declaration in two.
function splitStyleDeclarations(value) {
  const parts = [];
  let depth = 0;
  let quote = null;
  let current = '';
  for (const ch of value) {
    if (quote) {
      current += ch;
      if (ch === quote) quote = null;
    } else if (ch === '"' || ch === "'") {
      quote = ch;
      current += ch;
    } else if (ch === '(') {
      depth++;
      current += ch;
    } else if (ch === ')') {
      depth = Math.max(0, depth - 1);
      current += ch;
    } else if (ch === ';' && depth === 0) {
      parts.push(current);
      current = '';
    } else {
      current += ch;
    }
  }
  parts.push(current);
  return parts;
}

// React's inline-style serializer joins declarations with a bare `;` (no
// trailing space) and `prop:value` (no space after the colon); hand-authored
// HTML in the baseline has `; ` after each declaration and often `prop: value`
// too (e.g. how-it-works' `style="border-color: var(--ui); color: var(--ui)"`).
// Both are cosmetic (scripts read style via computed/DOM APIs, never the raw
// attribute string) — collapse both to the same form so they don't
// false-fail the structural diff. Only the FIRST `:` in a declaration is the
// property/value separator (a value like `url(a:b)` keeps any later colons).
function normalizeDeclaration(decl) {
  const sep = decl.indexOf(':');
  if (sep === -1) return decl;
  return `${decl.slice(0, sep).trim()}:${decl.slice(sep + 1).trim()}`;
}

function normalizeStyleAttr(value) {
  return splitStyleDeclarations(value)
    .map((decl) => decl.trim())
    .filter((decl) => decl.length > 0)
    .map(normalizeDeclaration)
    .join(';');
}

function normalizeElement(el) {
  const attrs = [...el.attributes]
    // Inline event handlers (onclick, onload, ...) are behavior, not structure —
    // a converted page attaches the same listener via React onClick at
    // hydration instead of a string attribute in the served HTML (e.g. home's
    // #cookie dismiss button), so comparing them here would false-fail on a
    // correct conversion. Narrowed to real event-handler IDL attributes (the
    // DOM itself is the allowlist: `onclick`/`onload`/... exist as properties
    // on the element; a custom attribute like `onboarding` does not) so this
    // never skips a legitimate non-handler attribute that merely starts with "on".
    .filter((a) => !(a.name.toLowerCase().startsWith('on') && a.name.toLowerCase() in el))
    .map((a) => [a.name, a.name === 'style' ? normalizeStyleAttr(a.value) : a.value])
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
  return { type: 'element', tag: el.tagName.toLowerCase(), attrs, children: normalizeChildren(el) };
}

function normalizeChildren(el) {
  const kept = [];
  for (const child of el.childNodes) {
    if (child.nodeType === 8) continue; // comment (React hydration separators)
    if (child.nodeType === 1) {
      if (SKIP_TAGS.has(child.tagName.toLowerCase())) continue;
      kept.push(normalizeElement(child));
    } else if (child.nodeType === 3) {
      kept.push({ type: 'text', value: child.nodeValue });
    }
  }
  // Merge adjacent text nodes (adjacent after comments/skipped tags removed).
  const merged = [];
  for (const item of kept) {
    const prev = merged[merged.length - 1];
    if (item.type === 'text' && prev?.type === 'text') {
      prev.value += item.value;
    } else {
      merged.push(item.type === 'text' ? { type: 'text', value: item.value } : item);
    }
  }
  // Collapse whitespace runs, trim, drop whitespace-only text nodes.
  return merged
    .map((item) =>
      item.type === 'text' ? { type: 'text', value: item.value.replace(/\s+/g, ' ').trim() } : item
    )
    .filter((item) => item.type !== 'text' || item.value !== '');
}

function describe(node) {
  if (!node) return '(missing)';
  if (node.type === 'text') return JSON.stringify(node.value);
  const attrStr = node.attrs.map(([k, v]) => `${k}="${v}"`).join(' ');
  return attrStr ? `<${node.tag} ${attrStr}>` : `<${node.tag}>`;
}

function childPath(parentPath, node, index) {
  const suffix = index > 0 ? `:nth-child(${index + 1})` : '';
  if (!node || node.type === 'text') return `${parentPath} > [text]${suffix}`;
  const cls = node.attrs.find(([name]) => name === 'class')?.[1];
  const classSeg = cls ? '.' + cls.trim().split(/\s+/).join('.') : '';
  return `${parentPath} > ${node.tag}${classSeg}${suffix}`;
}

function countElements(node) {
  if (node.type !== 'element') return 0;
  return 1 + node.children.reduce((n, child) => n + countElements(child), 0);
}

// ── Recursive diff, capped at 10 reported mismatches per call ────────────────
function compare(a, b, path, mismatches) {
  if (mismatches.length >= 10) return;
  if (!a || !b || a.type !== b.type || (a.type === 'element' && a.tag !== b.tag)) {
    mismatches.push({ path, expected: describe(a), actual: describe(b) });
    return;
  }
  if (a.type === 'text') {
    if (a.value !== b.value) mismatches.push({ path, expected: describe(a), actual: describe(b) });
    return;
  }
  if (JSON.stringify(a.attrs) !== JSON.stringify(b.attrs)) {
    mismatches.push({ path, expected: describe(a), actual: describe(b) });
  }
  const len = Math.max(a.children.length, b.children.length);
  for (let i = 0; i < len && mismatches.length < 10; i++) {
    const childA = a.children[i];
    const childB = b.children[i];
    compare(childA, childB, childPath(path, childA ?? childB, i), mismatches);
  }
}

// Diffs the <body> subtrees of two HTML documents (as strings). Exported so
// the self-test (diff-dom.test.mjs) can exercise the real comparison logic
// without shelling out to the CLI.
export function diffBodies(baselineHtml, newHtml) {
  const baselineBody = normalizeElement(new JSDOM(baselineHtml).window.document.body);
  const newBody = normalizeElement(new JSDOM(newHtml).window.document.body);
  const mismatches = [];
  compare(baselineBody, newBody, 'body', mismatches);
  return { mismatches, elementCount: countElements(newBody) };
}

function main() {
  const [, , baselinePath, newPath] = process.argv;
  if (!baselinePath || !newPath) {
    console.error('Usage: node scripts/diff-dom.mjs <baseline.html> <new.html>');
    process.exit(1);
  }

  const baselineHtml = readFileSync(baselinePath, 'utf8');
  const newHtml = readFileSync(newPath, 'utf8');

  // ── Self-hosted fonts guard — never regress to an external font URL ────────
  const newDom = new JSDOM(newHtml);
  const linksFonts = [...newDom.window.document.querySelectorAll('link[href]')].some(
    (link) => link.getAttribute('href') === '/fonts/fonts.css'
  );
  if (!linksFonts) {
    console.error(`FAIL — ${newPath} has no <link href="/fonts/fonts.css"> (self-hosted fonts).`);
    process.exit(1);
  }

  const { mismatches, elementCount } = diffBodies(baselineHtml, newHtml);

  if (mismatches.length > 0) {
    console.error(`FAIL — DOM mismatch between ${baselinePath} and ${newPath}:\n`);
    for (const m of mismatches) {
      console.error(`  at ${m.path}`);
      console.error(`    expected: ${m.expected}`);
      console.error(`    actual:   ${m.actual}\n`);
    }
    process.exit(1);
  }

  console.log(`ok — ${elementCount} elements match`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
