/**
 * Guards the composite-container focus-ring suppression in `utilities.css`.
 *
 * Why a test and not a review note: the rule is plain CSS keyed on markup that
 * lives in TypeScript. Nothing in the type system or the linter connects
 * `[role='radiogroup'][tabindex='-1']` in the stylesheet to the `role` +
 * `tabIndex` a component actually renders, so either side can drift and the
 * only symptom is a 2px brand ring painted around a WHOLE radiogroup / tablist
 * / lightbox the moment the container takes focus (a mousedown in the gap
 * between options, then any keypress).
 *
 * The selectors are read out of the stylesheet rather than re-typed here, so
 * this cannot pass against a rule that no longer matches the shipped markup.
 */
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import { ImagePreview } from '../components/Image/ImagePreview';
import { SegmentedControl } from '../components/SegmentedControl/SegmentedControl';
import { Tabs } from '../components/Tabs/Tabs';

const HERE = dirname(fileURLToPath(import.meta.url));
/**
 * The stylesheet with comments stripped. This file explains its own cascade in
 * prose, so `@layer` and `[role=…]` both appear inside comments; matching on
 * the raw text would read documentation as CSS.
 */
const CSS = readFileSync(join(HERE, 'utilities.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');

/**
 * The selector list of the `{ outline: none }` composite rule, exactly as
 * authored. Split on `}` — no rule body in this file contains one — and take
 * the block that mentions a `role` attribute, `:focus-visible`, and the
 * suppression itself.
 */
function suppressionSelectors(): string[] {
  const block = CSS.split('}').find(
    (b) => b.includes('[role=') && b.includes(':focus-visible') && /outline:\s*none/.test(b)
  );
  if (!block) throw new Error('composite focus-suppression rule not found in utilities.css');
  return block
    .slice(0, block.indexOf('{'))
    .split(',')
    .map((selector) => selector.trim())
    .filter(Boolean);
}

/**
 * The same selectors with `:focus-visible` removed. That pseudo-class is the
 * WHEN; the attribute half is the WHAT, and it is the half that has to keep
 * describing the rendered markup. jsdom's selector engine does not evaluate
 * `:focus-visible`, so matching on the structural half is also the only way to
 * ask this question in a unit test.
 */
const STRUCTURAL = suppressionSelectors().map((selector) => selector.replace(':focus-visible', ''));

const matchesRule = (element: Element) => STRUCTURAL.some((selector) => element.matches(selector));

describe('composite containers are exempt from the global focus ring', () => {
  it('the stylesheet is unlayered — a layered rule could not win', () => {
    // Tailwind v4 emits utilities inside `@layer utilities`, and an unlayered
    // rule beats a layered one at ANY specificity. That is why this suppression
    // is CSS and not an `outline-none` class; wrapping this file in a layer
    // would silently disarm both it and the ring it opts out of.
    expect(CSS).not.toMatch(/@layer/);
    expect(STRUCTURAL.length).toBeGreaterThan(0);
  });

  it('covers the SegmentedControl radiogroup', () => {
    render(
      <SegmentedControl
        ariaLabel="Prompt quality"
        options={[
          { value: 'full', label: 'Full' },
          { value: 'auto', label: 'Auto' },
        ]}
        value="auto"
        onChange={vi.fn()}
      />
    );
    expect(matchesRule(screen.getByRole('radiogroup', { name: 'Prompt quality' }))).toBe(true);
    // …and NOT the option that actually is the tab stop: it keeps its own ring.
    expect(matchesRule(screen.getByRole('radio', { name: 'Auto' }))).toBe(false);
  });

  it('covers the Tabs tablist', () => {
    render(
      <Tabs
        ariaLabel="Sections"
        items={[
          { value: 'one', label: 'One' },
          { value: 'two', label: 'Two' },
        ]}
        value="one"
        onChange={vi.fn()}
      />
    );
    expect(matchesRule(screen.getByRole('tablist', { name: 'Sections' }))).toBe(true);
    expect(matchesRule(screen.getByRole('tab', { name: 'One' }))).toBe(false);
  });

  it('does NOT cover a dialog shell — it has no roving child to carry the ring', () => {
    render(
      <ImagePreview
        items={['https://example.com/a.png']}
        index={0}
        open
        onIndexChange={vi.fn()}
        onOpenChange={vi.fn()}
      />
    );
    // The radiogroup/tablist exemption is paid for by a roving tabindex: a
    // child is always the tab stop and paints its own ring. A dialog shell has
    // no such child — it is focused on its own, and with no enabled focusable
    // inside it the container's outline is the only indicator that exists. So
    // the suppression must not reach it (WCAG 2.4.7).
    expect(matchesRule(screen.getByRole('dialog'))).toBe(false);
  });

  it('leaves an indicator on a focused dialog shell with no focusable child', () => {
    // The case the exemption silently broke: a `tabIndex={-1}` dialog that
    // takes focus itself and contains nothing focusable. Written as bare markup
    // rather than a component so it cannot be fixed by changing where some
    // component happens to put focus.
    const { container } = render(
      <div role="dialog" aria-modal="true" tabIndex={-1} aria-label="Empty dialog" />
    );
    const dialog = container.firstElementChild as HTMLElement;
    dialog.focus();

    expect(dialog).toHaveFocus();
    expect(matchesRule(dialog)).toBe(false);
  });

  it('never reaches a composite that IS keyboard-reachable', () => {
    // The `[tabindex='-1']` half of each selector is load-bearing: a container
    // a user can Tab to must keep a visible indicator (WCAG 2.4.7). Relaxing
    // the rule to the bare role would silently strip it.
    const { container } = render(
      <div role="radiogroup" tabIndex={0} aria-label="Reachable by Tab" />
    );
    const group = container.firstElementChild;
    expect(group).not.toBeNull();
    expect(matchesRule(group as Element)).toBe(false);
  });
});
