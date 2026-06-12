/**
 * Display-only ProseMirror decorations for the résumé/cover-letter editor.
 *
 * These NEVER touch the document model — they only add CSS classes for the
 * "document skin" (so the editing surface reads like a rendered résumé) and are
 * recomputed on every doc change. Because they are decorations, the canonical
 * markdown the editor serializes is completely unaffected (the byte-exact
 * round-trip in `markdown.ts` is preserved).
 *
 * Header region: the contiguous run of non-empty top-level paragraphs BEFORE the
 * first section heading is the résumé header (name / role / contact line). They
 * get centered, and the first (the name) is enlarged. A document with no heading
 * (e.g. a cover letter) gets no header styling — it degrades to plain paragraph
 * flow, which is correct since a cover letter has no section structure to anchor.
 */
import type { Node as PMNode } from '@tiptap/pm/model';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';
import { Extension } from '@tiptap/react';

const decorationsKey = new PluginKey('rich-text-editor-decorations');

/** Index of the first top-level heading node, or -1 when there is none. */
function firstHeadingIndex(doc: PMNode): number {
  let found = -1;
  doc.forEach((node, _offset, index) => {
    if (found === -1 && node.type.name === 'heading') found = index;
  });
  return found;
}

function buildDecorations(doc: PMNode): DecorationSet {
  const headingAt = firstHeadingIndex(doc);
  // No heading → no header region (cover letters degrade to plain flow).
  if (headingAt <= 0) return DecorationSet.empty;

  const decorations: Decoration[] = [];
  let headerSeen = 0;
  doc.forEach((node, offset, index) => {
    if (index >= headingAt) return;
    if (node.type.name !== 'paragraph' || node.textContent.trim() === '') return;
    // First non-empty header paragraph is the candidate's name (enlarged).
    const cls = headerSeen === 0 ? 'rt-header rt-name' : 'rt-header';
    decorations.push(Decoration.node(offset, offset + node.nodeSize, { class: cls }));
    headerSeen += 1;
  });
  return DecorationSet.create(doc, decorations);
}

/**
 * Tiptap extension that adds the document-skin decorations. Stateless wrt the
 * document model — pure presentation, recomputed only when the doc changes.
 */
export const EditorDecorations = Extension.create({
  name: 'editorDecorations',
  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: decorationsKey,
        state: {
          init: (_config, { doc }) => buildDecorations(doc),
          apply: (tr, old) => (tr.docChanged ? buildDecorations(tr.doc) : old),
        },
        props: {
          decorations(state) {
            return decorationsKey.getState(state);
          },
        },
      }),
    ];
  },
});
