/**
 * Parity test (PR3 §B.2): every value in `notebook-palette.ts` must match
 * the corresponding custom property in `popup/popup.css` — in the base
 * `:root` block (light) AND BOTH dark sources (`@media
 * (prefers-color-scheme: dark)` and `:root[data-theme='dark']`) — so a
 * badge/stamp rendered on the page can never silently drift from the
 * extension's own identity.
 *
 * Reads the real `popup.css` text and extracts each block by its own
 * selector marker; hand-written CSS property names (not re-derived from the
 * palette module) so this cannot pass by proving `x === x`.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { describe, expect, it } from 'vitest';

import { NOTEBOOK_DARK, NOTEBOOK_LIGHT, type NotebookPalette } from './notebook-palette';

const CSS = readFileSync(join(__dirname, '../popup/popup.css'), 'utf8');

/** Slice the `{ ... }` body immediately following the first occurrence of
 *  `marker` in the source. */
function extractBlock(source: string, marker: string): string {
  const start = source.indexOf(marker);
  if (start === -1) throw new Error(`marker not found in popup.css: ${marker}`);
  const braceStart = source.indexOf('{', start);
  const braceEnd = source.indexOf('}', braceStart);
  if (braceStart === -1 || braceEnd === -1) {
    throw new Error(`could not find a { ... } body after marker: ${marker}`);
  }
  return source.slice(braceStart, braceEnd);
}

function extractVar(block: string, cssName: string): string {
  const m = block.match(new RegExp(`--${cssName}:\\s*([^;]+);`));
  if (!m?.[1]) throw new Error(`--${cssName} not declared in this block`);
  return m[1].trim();
}

/** [palette key, CSS custom-property name] — hand-written on purpose. */
const KEYS: [keyof NotebookPalette, string][] = [
  ['paper', 'paper'],
  ['card', 'card'],
  ['ink', 'ink'],
  ['inkSoft', 'ink-soft'],
  ['red', 'red'],
  ['redInk', 'red-ink'],
  ['ok', 'ok'],
  ['warn', 'warn'],
  ['shadow', 'shadow'],
];

describe('notebook palette parity with popup.css', () => {
  const rootBlock = extractBlock(CSS, ':root {');
  it.each(KEYS)('light %s matches the base :root block', (key, cssName) => {
    expect(NOTEBOOK_LIGHT[key]).toBe(extractVar(rootBlock, cssName));
  });

  const mediaBlock = extractBlock(CSS, '@media (prefers-color-scheme: dark)');
  it.each(KEYS)('dark %s matches the prefers-color-scheme block', (key, cssName) => {
    expect(NOTEBOOK_DARK[key]).toBe(extractVar(mediaBlock, cssName));
  });

  const themeBlock = extractBlock(CSS, "[data-theme='dark']");
  it.each(KEYS)('dark %s matches the data-theme=dark block', (key, cssName) => {
    expect(NOTEBOOK_DARK[key]).toBe(extractVar(themeBlock, cssName));
  });
});
