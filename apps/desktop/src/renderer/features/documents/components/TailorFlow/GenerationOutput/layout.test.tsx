/**
 * GenerationOutput — template picker, preview market wiring and scroll boundary.
 * Mocks, props builder and helpers live in `harness.tsx` (see its header).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

import { clickJobAdTab, pickTemplate, renderOutput, resetHarness } from './harness';

beforeEach(resetHarness);

describe('GenerationOutput', () => {
  // ── 6. Template picker ────────────────────────────────────────────────────────
  // The single chosen template drives BOTH docs' preview + export, so the picker
  // strip is visible on BOTH doc tabs (résumé AND cover) — never on the job-ad tab.

  describe('Template picker', () => {
    it.each([
      ['resume tab', { activeOut: 'resume' }],
      ['cover tab', { target: 'both', activeOut: 'cover' }],
    ] as const)('renders the template picker on the %s (doc view)', (_tab, overrides) => {
      renderOutput(overrides);
      expect(screen.getByTestId(TEST_IDS.documents.templatePicker)).toBeInTheDocument();
    });

    it('is absent after switching to the job-ad view', async () => {
      const user = userEvent.setup();
      renderOutput({ activeOut: 'resume' });

      await clickJobAdTab(user);

      expect(screen.queryByTestId(TEST_IDS.documents.templatePicker)).not.toBeInTheDocument();
    });

    it.each([
      // Start on a single-column template; pick a two-column one ('atelier').
      // Two-column → ATS mode must NOT be forced off.
      [
        'calls onTemplateChange with the selected id when a two-column template is picked',
        'classic',
        false,
        'atelier',
        false,
      ],
      // Start on a two-column template; pick a single-column one ('classic').
      [
        'calls onTemplateChange AND onAtsModeChange(false) when a single-column template is picked',
        'atelier',
        true,
        'classic',
        true,
      ],
      [
        'does NOT reset ATS mode when Lebenslauf (design tier) is picked',
        'atelier',
        true,
        'lebenslauf',
        false,
      ],
    ] as const)('%s', async (_name, templateId, atsMode, picked, resetsAts) => {
      const user = userEvent.setup();
      const onTemplateChange = vi.fn();
      const onAtsModeChange = vi.fn();
      renderOutput({ activeOut: 'resume', templateId, onTemplateChange, onAtsModeChange, atsMode });

      await pickTemplate(user, picked);

      expect(onTemplateChange).toHaveBeenCalledWith(picked);
      if (resetsAts) expect(onAtsModeChange).toHaveBeenCalledWith(false);
      else expect(onAtsModeChange).not.toHaveBeenCalled();
    });
  });

  // ── 8b. Export/preview market → PdfPreview's `locale` ────────────────────────
  // Regression guard: `useTailorPipeline` resolves the export market and passes
  // it as `market`, but the live preview used to receive no locale at all (the
  // Rust exporter then silently falls back to market "intl" for the preview
  // while the real export uses the resolved one — a German posting showed an
  // English salutation on screen but a German one in the download). Asserting
  // the actual forwarded value, not just that a render happened, is what would
  // fail if this prop were ever dropped again.

  describe('Export/preview market', () => {
    it.each([
      ['forwards a German market to PdfPreview as `locale` (not undefined)', 'de', 'de'],
      ['leaves `locale` empty when no market was resolved', undefined, ''],
    ])('%s', (_name, market, locale) => {
      renderOutput({ market });
      expect(screen.getByTestId(TEST_IDS.documents.pdfPreview)).toHaveAttribute(
        'data-locale',
        locale
      );
    });
  });

  // ── 10. Scroll boundary — the tab/action header stays pinned ─────────────────
  // Regression guard for the "header scrolls away" bug: the viewer used to grow
  // past its host (intrinsic `min-h-[32rem]` panel + no overflow of its own), so
  // the PARENT scrolled the whole component — header included. The scroll boundary
  // now lives on the tabpanel: the tab/action bar is a pinned `shrink-0` sibling
  // OUTSIDE it, while the option strips live INSIDE and scroll with the document.
  // jsdom has no layout, so these assert the structural invariants that produce the
  // behaviour, not pixels — the pixel measurements come from the Chromium run
  // recorded in the handoff.

  describe('scroll boundary', () => {
    const SCROLLS = /overflow-(?:y-)?(?:auto|scroll)/;

    it('the tabpanel owns the vertical scroll', () => {
      renderOutput({ target: 'both', activeOut: 'resume' });
      expect(screen.getByRole('tabpanel').className).toMatch(SCROLLS);
    });

    it('the tabpanel is bounded by its host, with no intrinsic min-height', () => {
      renderOutput({ target: 'both', activeOut: 'resume' });
      const panel = screen.getByRole('tabpanel');
      expect(panel.className).toContain('min-h-0');
      expect(panel.className).toContain('flex-1');
      // An arbitrary min-height (e.g. `min-h-[32rem]`) makes the panel taller than
      // its host again, which pushes the scroll back up to the caller.
      expect(panel.className).not.toMatch(/min-h-(?!0\b)/);
    });

    it('the root is height-bounded so the caller never has to scroll it', () => {
      renderOutput({ target: 'both', activeOut: 'resume' });
      const root = screen.getByRole('tabpanel').parentElement;
      expect(root).not.toBeNull();
      expect(root?.className).toContain('min-h-0');
      expect(root?.className).toContain('flex-1');
      expect(root?.className).toContain('overflow-hidden');
      expect(root?.className).not.toMatch(/min-h-(?!0\b)/);
    });

    it('gives the document region a floor so the scrollport can actually engage', () => {
      // Without a floor every child is flex-1/h-full, the content fits the
      // scrollport exactly and `overflow-y-auto` can never fire.
      renderOutput({ target: 'both', activeOut: 'resume' });
      const region = screen.getByTestId(TEST_IDS.documents.documentRegion);
      expect(region.className).toMatch(/min-h-\[\d+rem\]/);
      expect(region.className).toContain('flex-1');
      expect(screen.getByRole('tabpanel').contains(region)).toBe(true);
    });

    it('nothing between the root and the tabpanel is a second scroll container', () => {
      const { container } = renderOutput({ target: 'both', activeOut: 'resume' });
      for (
        let el = screen.getByRole('tabpanel').parentElement;
        el !== null && el !== container;
        el = el.parentElement
      ) {
        expect(el.className).not.toMatch(SCROLLS);
      }
    });

    it('keeps the tabs and the Copy/Export actions outside the scrollport', () => {
      renderOutput({ target: 'both', activeOut: 'resume' });
      const panel = screen.getByRole('tabpanel');
      expect(panel.contains(screen.getByRole('tablist'))).toBe(false);
      expect(panel.contains(screen.getByRole('button', { name: /autopilot\.apply\.copy/i }))).toBe(
        false
      );
      expect(panel.contains(screen.getByRole('button', { name: /aiGenerate\.export/i }))).toBe(
        false
      );
    });

    it('scrolls the template / accent / letter-layout strips WITH the document', () => {
      // Pinning these costs more permanent chrome than a small window can spare:
      // the document collapses to nothing and the last strip is clipped out of
      // reach behind the root's overflow-hidden. They belong in the scrollport.
      renderOutput({ target: 'both', activeOut: 'cover' });
      const panel = screen.getByRole('tabpanel');
      expect(panel.contains(screen.getByTestId(TEST_IDS.documents.templatePicker))).toBe(true);
      expect(
        panel.contains(screen.getByTestId(`${TEST_IDS.generation.letterLayoutOption}-classic`))
      ).toBe(true);
    });
  });
});
