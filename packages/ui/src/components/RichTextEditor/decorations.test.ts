import { describe, expect, it } from 'vitest';

import { buildDecorations, type LinkResolution } from './decorations';
import { getEditorSchema, markdownToDoc } from './markdown';

/** The text each decoration in the built set covers, for assertion convenience. */
function decoratedTexts(md: string, resolutions: LinkResolution[] = []): string[] {
  const schema = getEditorSchema();
  const doc = markdownToDoc(md, schema);
  const set = buildDecorations(doc, resolutions);
  return set.find().map((d) => doc.textBetween(d.from, d.to, '\n', ''));
}

describe('buildDecorations — links', () => {
  it('decorates a plain contact label that resolves to a url', () => {
    const texts = decoratedTexts('Köln | me@x.com | LinkedIn', [
      { label: 'LinkedIn', url: 'https://linkedin.com/in/x' },
    ]);
    expect(texts).toContain('LinkedIn');
  });

  it('does NOT decorate the label when there is no resolution for it', () => {
    expect(decoratedTexts('Köln | me@x.com | LinkedIn')).not.toContain('LinkedIn');
  });

  it('autolinks a bare http(s) url in body text', () => {
    const texts = decoratedTexts('Project — https://github.com/me/repo');
    expect(texts).toContain('https://github.com/me/repo');
  });

  it('never double-links text already inside a real markdown link mark', () => {
    // The label is already an inline [LinkedIn](url) link → its text carries a
    // link mark, so the resolution must NOT add a second decoration over it.
    const texts = decoratedTexts('Köln | [LinkedIn](https://linkedin.com/in/x)', [
      { label: 'LinkedIn', url: 'https://linkedin.com/in/x' },
    ]);
    expect(texts).not.toContain('LinkedIn');
  });

  it('ignores labels shorter than two characters', () => {
    expect(decoratedTexts('a b a', [{ label: 'a', url: 'https://x.com' }])).toHaveLength(0);
  });
});

describe('buildDecorations — header region', () => {
  it('tags the paragraphs before the first section heading as the header', () => {
    const texts = decoratedTexts('Saeed Kolivand\nSenior Developer\n## EXPERIENCE\n- did things');
    expect(texts.some((t) => t.includes('Saeed Kolivand'))).toBe(true);
    expect(texts.some((t) => t.includes('Senior Developer'))).toBe(true);
  });

  it('adds no header decorations when there is no section heading (cover letter)', () => {
    // No heading → degrade to plain flow (only link decorations, of which there
    // are none here).
    expect(decoratedTexts('Dear hiring manager,\n\nI am writing to apply…')).toHaveLength(0);
  });
});
