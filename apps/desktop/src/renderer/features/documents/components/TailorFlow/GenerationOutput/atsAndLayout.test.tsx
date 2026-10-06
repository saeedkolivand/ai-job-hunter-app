/**
 * GenerationOutput — ATS-safe toggle and the cover letter-layout picker.
 * Mocks, props builder and helpers live in `harness.tsx` (see its header).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

import { TEMPLATES } from '@/lib/generate';

import { GenerationOutput, makeProps, renderOutput, resetHarness } from './harness';

beforeEach(resetHarness);

type Overrides = Parameters<typeof renderOutput>[0];
const cover = (o: Overrides = {}): Overrides => ({ target: 'both', activeOut: 'cover', ...o });
const resume = (o: Overrides = {}): Overrides => ({ activeOut: 'resume', ...o });

/** Renders with `onAtsModeChange` spied, runs `act`, returns the spy. */
async function withAtsSpy(overrides: Overrides, act: () => Promise<void>) {
  const onAtsModeChange = vi.fn();
  renderOutput({ ...overrides, onAtsModeChange });
  await act();
  return onAtsModeChange;
}

describe('GenerationOutput', () => {
  // ── 7. ATS toggle ─────────────────────────────────────────────────────────────
  // One flag, shown on whichever tab it can still change: the résumé tab when
  // isDesignTier(templateId) (two-column OR photo, incl. Lebenslauf), and the
  // cover tab when the letter layout carries a decoration ATS mode drops.

  describe('ATS toggle', () => {
    // 'atelier' is a confirmed two-column template; Lebenslauf is single-column-with-photo.
    it.each(['atelier', 'lebenslauf'] as const)(
      'renders a switch when the design-tier template %s is active on the resume tab',
      (templateId) => {
        renderOutput(resume({ templateId }));
        expect(screen.getByRole('switch')).toBeInTheDocument();
      }
    );

    it.each([
      'an ATS-tier template (classic)',
      'a single-column template (e.g. "classic")',
      'another single-column template ("classic")',
    ])('does NOT render a switch for %s', () => {
      renderOutput(resume({ templateId: 'classic' }));
      expect(screen.queryByRole('switch')).not.toBeInTheDocument();
    });

    // Awesome/Deedy are design-tier but neither two-column nor photo-bearing —
    // the toggle hint must NOT claim to remove a photo that doesn't exist (F1).
    it.each(['awesome', 'deedy'] as const)(
      'sets the toggle hint to the decorative-only copy for %s (not the false photo hint)',
      (id) => {
        renderOutput(resume({ templateId: id }));
        // The accessible DESCRIPTION, not the hover title: a `title` on the
        // role-less wrapper is never read out, so this is what a screen-reader
        // user actually hears about which document the switch changes.
        expect(screen.getByRole('switch')).toHaveAccessibleDescription(
          'aiGenerate.atsModeHintDecorative'
        );
      }
    );

    it('is absent on the cover tab for a two-column template when the letter is undecorated', () => {
      // The template picker still shows on the cover tab; the résumé's two columns
      // are not what the cover tab's toggle would be about, and the default
      // (classic) letter layout has no decoration to drop.
      renderOutput(cover({ templateId: 'atelier' }));
      expect(screen.getByTestId(TEST_IDS.documents.templatePicker)).toBeInTheDocument();
      expect(screen.queryByRole('switch')).not.toBeInTheDocument();
    });

    // ── the cover tab's own gate: a DECORATED letter layout ────────────────────
    // The letter renderer reads the same flag (`data.opts.ats`), so the switch has
    // to be reachable from the cover tab — including under an ATS-tier résumé
    // template, where the letter is the ONLY thing the flag still changes.

    it.each(['banded', 'sidebar', 'monogram'] as const)(
      'renders on the cover tab for the decorated layout %s under an ATS-tier template',
      (letterLayoutId) => {
        renderOutput(cover({ templateId: 'classic', letterLayoutId }));
        expect(screen.getByRole('switch')).toHaveAccessibleDescription(
          'aiGenerate.atsModeHintLetter'
        );
      }
    );

    it.each(['classic', 'refined', 'navy'] as const)(
      'is absent on the cover tab for the undecorated layout %s',
      (letterLayoutId) => {
        renderOutput(cover({ templateId: 'classic', letterLayoutId }));
        expect(screen.queryByRole('switch')).not.toBeInTheDocument();
      }
    );

    it('uses the résumé hint (not the letter one) back on the résumé tab', () => {
      renderOutput({
        target: 'both',
        activeOut: 'resume',
        templateId: 'atelier',
        letterLayoutId: 'monogram',
      });
      expect(screen.getByRole('switch')).toHaveAccessibleDescription(
        'aiGenerate.atsModeHintTwoColumn'
      );
      expect(screen.getByRole('switch')).not.toHaveAccessibleDescription(
        'aiGenerate.atsModeHintLetter'
      );
    });

    it('flips atsMode from the cover tab — the reachable off switch for a monogram letter', async () => {
      const user = userEvent.setup();
      const onAtsModeChange = await withAtsSpy(
        cover({ templateId: 'classic', letterLayoutId: 'monogram', atsMode: false }),
        () => user.click(screen.getByRole('switch'))
      );

      expect(onAtsModeChange).toHaveBeenCalledWith(true);
    });

    it('reflects atsMode on the cover tab via aria-checked', () => {
      renderOutput(cover({ templateId: 'classic', letterLayoutId: 'monogram', atsMode: true }));
      expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true');
    });

    // The template dropdown mock renders each option as a role="option" row.
    it.each([
      [
        'keeps atsMode when an ATS-tier template is picked while the letter is decorated',
        'monogram',
        false,
      ],
      [
        'still clears atsMode on an ATS-tier pick when the letter layout is undecorated',
        'navy',
        true,
      ],
    ] as const)('%s', async (_name, letterLayoutId, clears) => {
      const user = userEvent.setup();
      const onAtsModeChange = await withAtsSpy(
        cover({ templateId: 'atelier', letterLayoutId, atsMode: true }),
        () => user.click(screen.getByRole('option', { name: TEMPLATES['classic'].name }))
      );

      if (clears) expect(onAtsModeChange).toHaveBeenCalledWith(false);
      else expect(onAtsModeChange).not.toHaveBeenCalled();
    });

    // A silent DOM insertion otherwise: picking Monogram makes the switch appear
    // with nothing announced. The region must be mounted BEFORE the change (a
    // live region cannot announce its own first render), which is what the
    // "empty while hidden" half of this pair pins.
    it('announces the toggle becoming available, via an always-mounted live region', () => {
      const { rerender } = renderOutput(cover({ templateId: 'classic' }));
      const region = screen.getByRole('status');
      expect(region).toHaveTextContent('');

      rerender(
        <GenerationOutput
          {...makeProps(cover({ templateId: 'classic', letterLayoutId: 'monogram' }))}
        />
      );
      expect(screen.getByRole('status')).toHaveTextContent('aiGenerate.atsToggleAvailable');
    });

    it.each([false, true])('reflects atsMode=%s via aria-checked', (atsMode) => {
      renderOutput(resume({ templateId: 'atelier', atsMode }));
      expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', String(atsMode));
    });

    it('calls onAtsModeChange(!atsMode) when clicked', async () => {
      const user = userEvent.setup();
      const onAtsModeChange = await withAtsSpy(
        resume({ templateId: 'atelier', atsMode: false }),
        () => user.click(screen.getByRole('switch'))
      );

      expect(onAtsModeChange).toHaveBeenCalledTimes(1);
      expect(onAtsModeChange).toHaveBeenCalledWith(true);
    });

    it('calls onAtsModeChange(false) when toggled off', async () => {
      const user = userEvent.setup();
      const onAtsModeChange = await withAtsSpy(
        resume({ templateId: 'atelier', atsMode: true }),
        () => user.click(screen.getByRole('switch'))
      );

      expect(onAtsModeChange).toHaveBeenCalledWith(false);
    });
  });

  // ── 7b. Letter layout picker (cover-only) ─────────────────────────────────────
  // The layout picker is the cover-doc counterpart to the résumé-only ATS toggle:
  // it only affects the cover letter, so it renders on the cover tab and never on
  // the résumé or job-ad tabs.

  describe('letter layout picker', () => {
    const letterOption = (id: string) => `${TEST_IDS.generation.letterLayoutOption}-${id}`;
    const pick = (id: string) => userEvent.setup().click(screen.getByTestId(letterOption(id)));

    it('renders on the cover tab', () => {
      renderOutput(cover());
      expect(screen.getByTestId(letterOption('classic'))).toBeInTheDocument();
    });

    it('is absent on the résumé tab', () => {
      renderOutput({ target: 'both', activeOut: 'resume' });
      expect(screen.queryByTestId(letterOption('classic'))).not.toBeInTheDocument();
    });

    it('forwards a layout pick to onLetterLayoutChange', async () => {
      const onLetterLayoutChange = vi.fn();
      renderOutput(cover({ onLetterLayoutChange }));
      await pick('refined');
      expect(onLetterLayoutChange).toHaveBeenCalledWith('refined');
    });

    // Symmetry with the template picker: dropping to an undecorated layout has
    // to RELEASE the shared atsMode, or the next decorated layout returns
    // silently pre-ATS'd and the user exports a monogram-less Monogram letter.
    // target='cover' with nothing saved: no résumé is exported, so the
    // (design-tier) template must not hold the shared flag open once the letter
    // stops reading it — whether a résumé is REACHABLE in this panel (`hasResume`),
    // not whether the run produced one. The saved-résumé case is the counterpart:
    // that résumé is still on screen and exportable under the design-tier
    // template, so releasing the flag would strand it with no way back on. One
    // predicate (`hasResumeTab`) drives both the tab list and this decision.
    it.each([
      [
        'releases atsMode when the new layout is undecorated and nothing else reads it',
        // ATS-tier template → no-op for the résumé
        cover({ templateId: 'classic', letterLayoutId: 'monogram' }),
        'classic',
        true,
      ],
      [
        'keeps atsMode on the same change while a design-tier template reads it',
        cover({ templateId: 'atelier', letterLayoutId: 'monogram' }),
        'classic',
        false,
      ],
      [
        "target='cover': releases atsMode even under a design-tier template",
        {
          target: 'cover',
          hasResume: false,
          activeOut: 'cover',
          templateId: 'atelier',
          letterLayoutId: 'monogram',
        },
        'classic',
        true,
      ],
      [
        "target='cover' with a saved résumé: keeps atsMode under a design-tier template",
        {
          target: 'cover',
          hasResume: true,
          activeOut: 'cover',
          templateId: 'atelier',
          letterLayoutId: 'monogram',
        },
        'classic',
        false,
      ],
      [
        'does not release atsMode when swapping between two DECORATED layouts',
        cover({ templateId: 'classic', letterLayoutId: 'monogram' }),
        'sidebar',
        false,
      ],
    ] as const)('%s', async (_name, overrides, layout, releases) => {
      const onAtsModeChange = await withAtsSpy({ ...overrides, atsMode: true }, () => pick(layout));
      if (releases) expect(onAtsModeChange).toHaveBeenCalledWith(false);
      else expect(onAtsModeChange).not.toHaveBeenCalled();
    });
  });
});
