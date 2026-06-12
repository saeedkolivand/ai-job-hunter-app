/**
 * Display-only ProseMirror decorations for the résumé/cover-letter editor.
 *
 * These NEVER touch the document model — they only add CSS classes / inline
 * `<a>` wrappers for presentation, and are recomputed as the doc (or the link
 * data) changes. Because they are decorations, the canonical markdown the editor
 * serializes is completely unaffected (the byte-exact round-trip in `markdown.ts`
 * is preserved).
 *
 * Two kinds:
 *  - **Header region** — the contiguous run of non-empty top-level paragraphs
 *    BEFORE the first section heading is the résumé header (name / role / contact
 *    line). They get centered, and the first (the name) is enlarged. A document
 *    with no heading (e.g. a cover letter) gets no header styling — it degrades to
 *    plain paragraph flow.
 *  - **Links** — plain text that the export pipeline WOULD hyperlink (contact /
 *    body labels resolved from the reference block + ContactProfile, supplied by
 *    the renderer) plus bare `http(s)` URLs are rendered as links, so the editor
 *    shows the same links the preview/export does. Text already inside a real
 *    `[label](url)` mark is skipped (never double-linked).
 */
import type { Node as PMNode } from '@tiptap/pm/model';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';
import { type Editor, Extension } from '@tiptap/react';

/** A label→url pair the editor should render as a (display-only) link. */
export interface LinkResolution {
  label: string;
  url: string;
}

interface DecorationState {
  resolutions: LinkResolution[];
  set: DecorationSet;
}

const decorationsKey = new PluginKey<DecorationState>('rich-text-editor-decorations');

/** Push the current link resolutions into the decoration plugin (display-only). */
export function setLinkResolutions(editor: Editor, resolutions: LinkResolution[]): void {
  const { view } = editor;
  view.dispatch(view.state.tr.setMeta(decorationsKey, resolutions));
}

function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

// A URL in body text: a scheme URL, OR a scheme-less domain WITH a path
// (`github.com/user/repo`) — the form résumé project links usually take. A bare
// domain with no path, or a token like `CI/CD`, is intentionally NOT matched.
const URL_RE = /\bhttps?:\/\/[^\s)]+|\b(?:[a-z0-9-]+\.)+[a-z]{2,}\/[^\s)]+/gi;

// ── Header region ───────────────────────────────────────────────────────────

/** Index of the first top-level heading node, or -1 when there is none. */
function firstHeadingIndex(doc: PMNode): number {
  let found = -1;
  doc.forEach((node, _offset, index) => {
    if (found === -1 && node.type.name === 'heading') found = index;
  });
  return found;
}

/** Document position where the first top-level heading starts, or -1 if none. */
function firstHeadingPos(doc: PMNode): number {
  let pos = -1;
  doc.forEach((node, offset) => {
    if (pos === -1 && node.type.name === 'heading') pos = offset;
  });
  return pos;
}

function headerDecorations(doc: PMNode): Decoration[] {
  const headingAt = firstHeadingIndex(doc);
  // No heading → no header region (cover letters degrade to plain flow).
  if (headingAt <= 0) return [];

  const decorations: Decoration[] = [];
  let headerSeen = 0;
  doc.forEach((node, offset, index) => {
    if (index >= headingAt) return;
    if (node.type.name !== 'paragraph' || node.textContent.trim() === '') return;
    const cls = headerSeen === 0 ? 'rt-header rt-name' : 'rt-header';
    decorations.push(Decoration.node(offset, offset + node.nodeSize, { class: cls }));
    headerSeen += 1;
  });
  return decorations;
}

// ── Links ───────────────────────────────────────────────────────────────────

function linkDecorations(doc: PMNode, resolutions: LinkResolution[]): Decoration[] {
  // Contact-brand labels (LinkedIn, GitHub, Website, …) are linked ONLY inside the
  // header region — the contact line lives before the first section heading —
  // mirroring the backend's contact-line gating. This stops a brand keyword from
  // being linked where it merely appears in body text (e.g. "github" inside a
  // project URL in a PROJECTS section). Project links are picked up as URLs below.
  const headingPos = firstHeadingPos(doc);
  const headerLimit = headingPos === -1 ? Number.POSITIVE_INFINITY : headingPos;

  // Whole-word, case-insensitive matchers for each resolved label. Labels under
  // 2 chars are skipped to avoid decorating stray fragments.
  const matchers = resolutions
    .map((r) => ({ label: r.label.trim(), url: r.url.trim() }))
    .filter((r) => r.label.length >= 2 && r.url.length > 0)
    .map((r) => ({ url: r.url, re: new RegExp(`\\b${escapeRegExp(r.label)}\\b`, 'gi') }));

  const decorations: Decoration[] = [];
  const linkSpec = (url: string) => ({ nodeName: 'a', class: 'rt-link', 'data-href': url });

  doc.descendants((node, pos) => {
    if (!node.isText || !node.text) return;
    // Skip text already carrying a real link mark — never double-link.
    if (node.marks.some((m) => m.type.name === 'link')) return;
    const text = node.text;

    // Brand labels: header region only (gated like the export's contact line).
    if (pos < headerLimit) {
      for (const { url, re } of matchers) {
        re.lastIndex = 0;
        let m: RegExpExecArray | null;
        while ((m = re.exec(text)) !== null) {
          decorations.push(
            Decoration.inline(pos + m.index, pos + m.index + m[0].length, linkSpec(url))
          );
        }
      }
    }

    // URLs (including scheme-less project links): anywhere in the document.
    URL_RE.lastIndex = 0;
    let u: RegExpExecArray | null;
    while ((u = URL_RE.exec(text)) !== null) {
      decorations.push(
        Decoration.inline(pos + u.index, pos + u.index + u[0].length, linkSpec(u[0]))
      );
    }
  });
  return decorations;
}

/** Build the full decoration set (header + links) for a document. */
export function buildDecorations(doc: PMNode, resolutions: LinkResolution[]): DecorationSet {
  const decorations = [...headerDecorations(doc), ...linkDecorations(doc, resolutions)];
  return decorations.length === 0 ? DecorationSet.empty : DecorationSet.create(doc, decorations);
}

/**
 * Tiptap extension adding the document-skin + link decorations. Stateless wrt the
 * document model — pure presentation. Link resolutions are pushed in by the React
 * component via `setLinkResolutions` (a transaction meta), so they react to the
 * current document without re-creating the editor.
 */
export const EditorDecorations = Extension.create({
  name: 'editorDecorations',
  addProseMirrorPlugins() {
    return [
      new Plugin<DecorationState>({
        key: decorationsKey,
        state: {
          init: (_config, { doc }) => ({ resolutions: [], set: buildDecorations(doc, []) }),
          apply: (tr, prev) => {
            const meta = tr.getMeta(decorationsKey) as LinkResolution[] | undefined;
            if (meta === undefined && !tr.docChanged) return prev;
            const resolutions = meta ?? prev.resolutions;
            return { resolutions, set: buildDecorations(tr.doc, resolutions) };
          },
        },
        props: {
          decorations(state) {
            return decorationsKey.getState(state)?.set;
          },
        },
      }),
    ];
  },
});
