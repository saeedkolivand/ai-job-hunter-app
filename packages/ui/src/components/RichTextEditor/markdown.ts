/**
 * Custom, line-oriented markdown ⇄ ProseMirror round-trip for the resume /
 * cover-letter editor.
 *
 * WHY HAND-ROLLED (not markdown-it / prosemirror-markdown): generic markdown
 * engines normalise whitespace (collapsing runs of spaces) and escape literal
 * punctuation (`·`, `|`, `(`, `)`, `-`, `@`). Resume content depends on every
 * one of those surviving byte-for-byte — a job-entry line such as
 * `Senior Engineer␣␣␣Jan 2020 – Present` carries meaning in its double spaces,
 * and the Rust `parse_line` classifier reads pipe/middot/paren punctuation.
 *
 * The contract this module guarantees:
 *   serialize(parse(md)) === md   (byte-exact) for unedited real documents.
 *
 * The pure functions (`splitPreserved`, `joinPreserved`, `markdownToDoc`,
 * `docToMarkdown`, `roundTrip`) take/return plain data + a ProseMirror `Schema`,
 * so they are testable headlessly with no DOM or live editor — build the schema
 * with `getEditorSchema()` (or pass the live editor's `editor.schema`).
 */
import type { MarkType, Node as PMNode, NodeType, Schema } from '@tiptap/pm/model';
import { getSchema } from '@tiptap/react';

import { buildEditorExtensions } from './extensions';

/**
 * Look up a node type by name. The locked schema (see `extensions.ts`)
 * guarantees every name used here exists, so a miss is a programmer error.
 */
function nodeType(schema: Schema, name: string): NodeType {
  const type = schema.nodes[name];
  if (!type) throw new Error(`RichTextEditor schema is missing node type: ${name}`);
  return type;
}

/** Look up a mark type by name (see `nodeType`). */
function markType(schema: Schema, name: string): MarkType {
  const type = schema.marks[name];
  if (!type) throw new Error(`RichTextEditor schema is missing mark type: ${name}`);
  return type;
}

// ── Schema (headless) ───────────────────────────────────────────────────────

let cachedSchema: Schema | undefined;

/**
 * The ProseMirror `Schema` for the locked editor extensions, built without a
 * DOM or editor instance. Cached — the extension set is static. Use in tests and
 * anywhere a schema is needed off the main editor.
 */
export function getEditorSchema(): Schema {
  cachedSchema ??= getSchema(buildEditorExtensions());
  return cachedSchema;
}

// ── Hold-out of the trailing link-reference block ───────────────────────────

/**
 * The Rust PDF/DOCX extractor appends a `\n---\n` block of `- [Label](url)`
 * lines that the backend link classifier reads
 * (`packages/prompts/src/generate/links.ts::parseLinkBlock`, which scans from the
 * LAST `\n---\n`). The editor must never reorder, escape, or drop it, so it is
 * held out of the editable body entirely and re-appended verbatim on serialize.
 */
const LINK_BLOCK_SEP = '\n---\n';
/**
 * The URL portion of an inline/reference link: everything inside the `(...)`,
 * allowing ONE level of nested parentheses so URLs like
 * `https://en.wikipedia.org/wiki/C_(programming_language)` are matched to their
 * FINAL `)` instead of being truncated at the first interior `)`. Deeper
 * nesting (rare in real URLs) still stops at the first unbalanced `)`.
 * Kept as a source string so it can be embedded in several anchored patterns.
 */
const LINK_URL = String.raw`\(([^()]*(?:\([^()]*\)[^()]*)*)\)`;
/** A single reference-block line: `- [anchor](url)` (anchor/url non-empty). */
const LINK_BLOCK_LINE = new RegExp(String.raw`^- \[[^\]]+\]${LINK_URL}$`);

export interface SplitPreserved {
  /** The editable markdown (everything before the held-out tail). */
  body: string;
  /**
   * The held-out trailing link-reference block, INCLUDING its leading
   * `\n---\n`, re-appended verbatim by `joinPreserved`. `''` when absent.
   */
  tail: string;
}

/**
 * Split a markdown document into the editable `body` and a held-out `tail`
 * (the trailing `\n---\n` link-reference block). Mirrors the backend's
 * `lastIndexOf('\n---\n')` detection: the tail is the LAST `\n---\n` whose
 * following lines are ALL `- [Label](url)` entries to end-of-string. If the
 * block after the separator contains anything else, it is treated as ordinary
 * body content (no tail) so we never swallow a real `---` the user typed.
 */
export function splitPreserved(md: string): SplitPreserved {
  const sep = md.lastIndexOf(LINK_BLOCK_SEP);
  if (sep === -1) return { body: md, tail: '' };
  const after = md.slice(sep + LINK_BLOCK_SEP.length);
  // Every non-empty line after the separator must be a reference-block entry.
  const lines = after.split('\n');
  const allLinkLines =
    lines.length > 0 &&
    lines.some((l) => l.length > 0) &&
    lines.every((l) => l.length === 0 || LINK_BLOCK_LINE.test(l));
  if (!allLinkLines) return { body: md, tail: '' };
  return { body: md.slice(0, sep), tail: md.slice(sep) };
}

/** Re-append a held-out `tail` (from `splitPreserved`) to an edited `body`. */
export function joinPreserved(body: string, tail: string): string {
  return tail ? body + tail : body;
}

// ── Inline marks (hand-rolled, whitespace- and punctuation-preserving) ──────

interface InlineSeg {
  text: string;
  bold?: boolean;
  italic?: boolean;
  link?: string;
}

// Order matters: links first (so `*` inside a label/url is not mistaken for
// emphasis), then bold (`**`) before italic (`*`). The URL sub-pattern allows
// one level of nested parens (see `LINK_URL`) so wiki-style URLs that contain
// a balanced `(...)` are captured whole instead of truncated at the first `)`.
const INLINE_TOKEN = new RegExp(
  String.raw`(\[[^\]]+\]${LINK_URL})|(\*\*[^*]+\*\*)|(\*[^*\n]+\*)`,
  'g'
);
const INLINE_LINK = new RegExp(String.raw`^\[([^\]]+)\]${LINK_URL}$`);

/**
 * Parse a single line of inline markdown into segments. Only `**bold**`,
 * `*italic*`, and `[label](url)` are consumed; EVERY other character —
 * including consecutive spaces and `·|()@-` — is preserved verbatim. Marks are
 * intentionally non-nesting (matches the flat export vocabulary).
 */
function parseInline(line: string): InlineSeg[] {
  const segs: InlineSeg[] = [];
  let last = 0;
  for (const m of line.matchAll(INLINE_TOKEN)) {
    const idx = m.index ?? 0;
    if (idx > last) segs.push({ text: line.slice(last, idx) });
    const token = m[0];
    const link = INLINE_LINK.exec(token);
    if (link) {
      segs.push({ text: link[1] ?? '', link: link[2] ?? '' });
    } else if (token.startsWith('**')) {
      segs.push({ text: token.slice(2, -2), bold: true });
    } else {
      segs.push({ text: token.slice(1, -1), italic: true });
    }
    last = idx + token.length;
  }
  if (last < line.length) segs.push({ text: line.slice(last) });
  return segs;
}

/** Serialize inline segments back to markdown (inverse of `parseInline`). */
function serializeInline(segs: InlineSeg[]): string {
  let out = '';
  for (const seg of segs) {
    let t = seg.text;
    // Apply emphasis innermost-first, THEN wrap the (possibly emphasised) label
    // in the link, so a segment that is both a link and bold serialises as
    // `[**foo**](url)` — emphasis must wrap from `t`, not from the raw text, or
    // a bold/italic link silently loses its marks. A run can be both bold and
    // italic; ordering bold-then-italic yields `***foo***`.
    if (seg.bold) t = `**${t}**`;
    if (seg.italic) t = `*${t}*`;
    if (seg.link !== undefined) t = `[${t}](${seg.link})`;
    out += t;
  }
  return out;
}

// ── Block model ─────────────────────────────────────────────────────────────

type Block =
  | { kind: 'heading'; level: 2 | 3; text: string }
  | { kind: 'bullet'; items: string[] }
  | { kind: 'paragraph'; text: string };

const HEADING_RE = /^(#{2,3}) (.*)$/;
const BULLET_RE = /^([-*•]) (.*)$/;

/**
 * Parse the editable body markdown into a flat block list, ONE BLOCK PER SOURCE
 * LINE (except bullets, which group). This line-structure-preserving model is
 * what makes the round-trip byte-exact: the canonical resume format the Rust
 * parser consumes is line-oriented and does NOT guarantee a blank line between
 * blocks, so blanks must be preserved exactly rather than normalised away.
 *
 * - `## ` → H2, `### ` → H3 (marker stripped; remainder parsed for inline marks)
 * - `- ` / `* ` / `• ` → bullet item; consecutive bullet lines group into ONE list
 * - blank line → empty paragraph (so it survives serialize as a blank line)
 * - anything else → paragraph (single source line)
 */
function parseBlocks(md: string): Block[] {
  const lines = md.split('\n');
  const blocks: Block[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i] ?? '';
    const heading = HEADING_RE.exec(line);
    if (heading) {
      const level = (heading[1] ?? '').length === 2 ? 2 : 3;
      blocks.push({ kind: 'heading', level, text: heading[2] ?? '' });
      i += 1;
      continue;
    }
    if (BULLET_RE.test(line)) {
      const items: string[] = [];
      while (i < lines.length) {
        const m = BULLET_RE.exec(lines[i] ?? '');
        if (!m) break;
        items.push(m[2] ?? '');
        i += 1;
      }
      blocks.push({ kind: 'bullet', items });
      continue;
    }
    // Both blank and non-blank lines become a paragraph (empty ↔ blank line),
    // preserving the source line count exactly.
    blocks.push({ kind: 'paragraph', text: line });
    i += 1;
  }
  return blocks;
}

// ── markdown → ProseMirror doc ──────────────────────────────────────────────

/** Build the inline text/mark nodes for one block's text into a content array. */
function inlineToPM(schema: Schema, text: string): PMNode[] {
  const segs = parseInline(text);
  const nodes: PMNode[] = [];
  for (const seg of segs) {
    if (seg.text === '') continue;
    const marks = [];
    if (seg.bold) marks.push(markType(schema, 'bold').create());
    if (seg.italic) marks.push(markType(schema, 'italic').create());
    if (seg.link !== undefined) marks.push(markType(schema, 'link').create({ href: seg.link }));
    nodes.push(schema.text(seg.text, marks));
  }
  return nodes;
}

/**
 * Parse markdown (the editable BODY — call `splitPreserved` first) into a
 * ProseMirror document for the locked schema. Pure: no DOM / editor needed.
 */
export function markdownToDoc(md: string, schema: Schema = getEditorSchema()): PMNode {
  const blocks = parseBlocks(md);
  const content: PMNode[] = [];
  for (const block of blocks) {
    if (block.kind === 'heading') {
      content.push(
        nodeType(schema, 'heading').create({ level: block.level }, inlineToPM(schema, block.text))
      );
    } else if (block.kind === 'bullet') {
      const items = block.items.map((item) =>
        nodeType(schema, 'listItem').create(
          null,
          nodeType(schema, 'paragraph').create(null, inlineToPM(schema, item))
        )
      );
      content.push(nodeType(schema, 'bulletList').create(null, items));
    } else {
      content.push(nodeType(schema, 'paragraph').create(null, inlineToPM(schema, block.text)));
    }
  }
  // An empty body must still be a valid doc (one empty paragraph).
  if (content.length === 0) content.push(nodeType(schema, 'paragraph').create());
  return nodeType(schema, 'doc').create(null, content);
}

// ── ProseMirror doc → markdown ──────────────────────────────────────────────

/** Serialize one textblock's inline content (text + marks) to markdown. */
function inlineFromPM(node: PMNode): string {
  const segs: InlineSeg[] = [];
  node.forEach((child) => {
    if (!child.isText) {
      // hardBreak (the only other inline node) → newline within the block.
      if (child.type.name === 'hardBreak') segs.push({ text: '\n' });
      return;
    }
    const seg: InlineSeg = { text: child.text ?? '' };
    for (const mark of child.marks) {
      if (mark.type.name === 'bold') seg.bold = true;
      else if (mark.type.name === 'italic') seg.italic = true;
      else if (mark.type.name === 'link') seg.link = String(mark.attrs.href ?? '');
    }
    segs.push(seg);
  });
  return serializeInline(segs);
}

/**
 * Serialize a ProseMirror document back to markdown (inverse of
 * `markdownToDoc`). Each top-level block emits its own source line(s) joined by
 * a single `\n`; an empty paragraph emits a blank line, so the line structure
 * (including blank lines) round-trips byte-exact. The caller re-appends the
 * held-out `tail` via `joinPreserved`.
 */
export function docToMarkdown(doc: PMNode): string {
  const out: string[] = [];
  doc.forEach((node) => {
    switch (node.type.name) {
      case 'heading': {
        const hashes = node.attrs.level === 2 ? '##' : '###';
        out.push(`${hashes} ${inlineFromPM(node)}`);
        break;
      }
      case 'bulletList': {
        node.forEach((item) => {
          // listItem contains a single paragraph in the flat schema.
          item.forEach((para) => out.push(`- ${inlineFromPM(para)}`));
        });
        break;
      }
      case 'paragraph':
      default: {
        out.push(inlineFromPM(node));
        break;
      }
    }
  });
  return out.join('\n');
}

// ── Pure end-to-end round-trip (test/verification helper) ───────────────────

/**
 * The full idempotency contract as a single pure function: split off the tail,
 * parse the body to a doc, serialize it back, and re-append the tail verbatim.
 * For an unedited real document this returns `md` byte-for-byte.
 */
export function roundTrip(md: string, schema: Schema = getEditorSchema()): string {
  const { body, tail } = splitPreserved(md);
  return joinPreserved(docToMarkdown(markdownToDoc(body, schema)), tail);
}
