// ATS-mode toggle: tier-aware gate, hint copy, decorated letter layouts, a11y contract.
import { beforeEach, describe, expect, it, type Mock, vi } from 'vitest';
import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { renderStepWith } from './test-support';

vi.mock('@ajh/translations', async () => (await import('./stubs')).translationsMock);
vi.mock('../../../samples', async () => (await import('./stubs')).samplesMock());

describe('StepTemplate', () => {
  let onTemplateChange: Mock;
  let onAtsModeChange: Mock;
  const renderStep = (props: Parameters<typeof renderStepWith>[2] = {}) =>
    renderStepWith(onTemplateChange, onAtsModeChange, props);

  beforeEach(() => {
    onTemplateChange = vi.fn();
    onAtsModeChange = vi.fn();
  });

  // ── ATS toggle gate is tier-aware (the Lebenslauf photo fix) ─────────────────

  it.each(['atelier', 'portrait', 'lebenslauf', 'aria', 'saffron', 'awesome', 'deedy'] as const)(
    'shows the ATS toggle for the design-tier template %s',
    (id) => {
      renderStep({ templateId: id });
      expect(screen.getByRole('switch', { name: 'aiGenerate.atsMode' })).toBeInTheDocument();
    }
  );

  it.each([
    'classic',
    'swiss-minimal',
    'academic',
    'meridian',
    'throughline',
    'cadence',
    'regent',
    'cologne-navy',
    'jake',
  ] as const)('hides the ATS toggle for the ATS-tier template %s', (id) => {
    renderStep({ templateId: id });
    expect(screen.queryByRole('switch')).not.toBeInTheDocument();
  });

  it('uses the two-column hint for a two-column template but the photo hint for Lebenslauf', () => {
    const { unmount } = renderStep({ templateId: 'atelier' });
    expect(screen.getByText('aiGenerate.atsModeHintTwoColumn')).toBeInTheDocument();
    expect(screen.queryByText('aiGenerate.atsModeHintPhoto')).not.toBeInTheDocument();
    unmount();

    renderStep({ templateId: 'lebenslauf' });
    expect(screen.getByText('aiGenerate.atsModeHintPhoto')).toBeInTheDocument();
    expect(screen.queryByText('aiGenerate.atsModeHintTwoColumn')).not.toBeInTheDocument();
  });

  // Awesome/Deedy are design-tier but neither two-column NOR photo-bearing —
  // routing them to the photo hint is factually false (no photo to remove).
  // They need the decorative-only hint instead (F1).
  it.each(['awesome', 'deedy'] as const)(
    'uses the decorative hint (not the false photo hint) for %s',
    (id) => {
      renderStep({ templateId: id });
      expect(screen.getByText('aiGenerate.atsModeHintDecorative')).toBeInTheDocument();
      expect(screen.queryByText('aiGenerate.atsModeHintPhoto')).not.toBeInTheDocument();
      expect(screen.queryByText('aiGenerate.atsModeHintTwoColumn')).not.toBeInTheDocument();
    }
  );

  // Portrait is two-column AND has a photo — it must get the (inclusive)
  // two-column hint copy, which also covers photo removal, not the photo-only key.
  it('uses the inclusive two-column hint for Portrait (two-column + photo)', () => {
    renderStep({ templateId: 'portrait' });
    expect(screen.getByText('aiGenerate.atsModeHintTwoColumn')).toBeInTheDocument();
    expect(screen.queryByText('aiGenerate.atsModeHintPhoto')).not.toBeInTheDocument();
  });

  it('resets ATS mode when an ATS-tier template is selected from a design-tier one', async () => {
    const user = userEvent.setup();
    renderStep({ templateId: 'lebenslauf', atsMode: true });
    const swissButton = screen.getByText('Swiss Minimal').closest('button');
    if (!swissButton) throw new Error('Swiss Minimal button not found');
    await user.click(swissButton);
    expect(onTemplateChange).toHaveBeenCalledWith('swiss-minimal');
    expect(onAtsModeChange).toHaveBeenCalledWith(false);
  });

  it('does NOT reset ATS mode when Lebenslauf (design tier) is selected', async () => {
    const user = userEvent.setup();
    renderStep({ templateId: 'atelier', atsMode: true });
    const lebenslaufButton = screen.getByText('Lebenslauf (DACH)').closest('button');
    if (!lebenslaufButton) throw new Error('Lebenslauf button not found');
    await user.click(lebenslaufButton);
    expect(onTemplateChange).toHaveBeenCalledWith('lebenslauf');
    expect(onAtsModeChange).not.toHaveBeenCalled();
  });

  // ── ATS toggle for a DECORATED cover-letter layout ───────────────────────────
  // The letter renderer reads the same atsMode flag (`data.opts.ats`), so a
  // decorated layout (band / rail / monogram tile) needs the switch on a surface
  // that produces a letter — even when the résumé template is ATS-tier and the
  // résumé itself has nothing to linearize.

  it.each(['banded', 'sidebar', 'monogram'] as const)(
    'target=cover: shows the ATS toggle for the decorated layout %s under an ATS-tier template',
    (letterLayoutId) => {
      renderStep({ templateId: 'classic', target: 'cover', letterLayoutId });
      expect(screen.getByRole('switch', { name: 'aiGenerate.atsMode' })).toBeInTheDocument();
      expect(screen.getByText('aiGenerate.atsModeHintLetter')).toBeInTheDocument();
    }
  );

  it.each(['classic', 'refined', 'navy'] as const)(
    'target=cover: hides the ATS toggle for the undecorated layout %s',
    (letterLayoutId) => {
      renderStep({ templateId: 'classic', target: 'cover', letterLayoutId });
      expect(screen.queryByRole('switch')).not.toBeInTheDocument();
    }
  );

  it('target=cover: hides the ATS toggle when no layout has been picked yet (unset → classic)', () => {
    renderStep({ templateId: 'classic', target: 'cover' });
    expect(screen.queryByRole('switch')).not.toBeInTheDocument();
  });

  it("target='both' with a decorated letter shows the toggle even for an ATS-tier template", () => {
    renderStep({ templateId: 'classic', target: 'both', letterLayoutId: 'monogram' });
    expect(screen.getByRole('switch', { name: 'aiGenerate.atsMode' })).toBeInTheDocument();
    // Résumé is ATS-tier → only the letter hint, no résumé hint.
    expect(screen.getByText('aiGenerate.atsModeHintLetter')).toBeInTheDocument();
    expect(screen.queryByText('aiGenerate.atsModeHintTwoColumn')).not.toBeInTheDocument();
  });

  it("target='both' with a design-tier template AND a decorated letter states BOTH effects", () => {
    renderStep({ templateId: 'atelier', target: 'both', letterLayoutId: 'sidebar' });
    expect(screen.getByText('aiGenerate.atsModeHintTwoColumn')).toBeInTheDocument();
    expect(screen.getByText('aiGenerate.atsModeHintLetter')).toBeInTheDocument();
  });

  it("target='resume' never shows the letter hint, whatever layout is threaded through", () => {
    // A résumé-only run exports no letter — a letter hint there would be a lie.
    renderStep({ templateId: 'classic', target: 'resume', letterLayoutId: 'monogram' });
    expect(screen.queryByRole('switch')).not.toBeInTheDocument();
    expect(screen.queryByText('aiGenerate.atsModeHintLetter')).not.toBeInTheDocument();
  });

  it('the toggle actually flips atsMode on for a decorated letter (the reachable off switch)', async () => {
    const user = userEvent.setup();
    renderStep({
      templateId: 'classic',
      target: 'cover',
      letterLayoutId: 'monogram',
      atsMode: false,
    });
    await user.click(screen.getByRole('switch'));
    expect(onAtsModeChange).toHaveBeenCalledWith(true);
  });

  it('reflects the current atsMode via aria-checked so the letter state is readable', () => {
    const { unmount } = renderStep({
      templateId: 'classic',
      target: 'cover',
      letterLayoutId: 'monogram',
      atsMode: true,
    });
    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true');
    unmount();

    renderStep({
      templateId: 'classic',
      target: 'cover',
      letterLayoutId: 'monogram',
      atsMode: false,
    });
    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'false');
  });

  // ── force-clear must not strand a decorated letter ───────────────────────────

  it('does NOT clear atsMode when an ATS-tier template is picked while a decorated letter is in the run', async () => {
    const user = userEvent.setup();
    renderStep({
      templateId: 'atelier',
      target: 'both',
      letterLayoutId: 'monogram',
      atsMode: true,
    });
    const swissButton = screen.getByText('Swiss Minimal').closest('button');
    if (!swissButton) throw new Error('Swiss Minimal button not found');
    await user.click(swissButton);
    expect(onTemplateChange).toHaveBeenCalledWith('swiss-minimal');
    expect(onAtsModeChange).not.toHaveBeenCalled();
    // …and the switch survives the template change, still on.
    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true');
  });

  it('still clears atsMode for an ATS-tier template when the letter layout is undecorated', async () => {
    const user = userEvent.setup();
    renderStep({ templateId: 'atelier', target: 'both', letterLayoutId: 'navy', atsMode: true });
    const swissButton = screen.getByText('Swiss Minimal').closest('button');
    if (!swissButton) throw new Error('Swiss Minimal button not found');
    await user.click(swissButton);
    expect(onAtsModeChange).toHaveBeenCalledWith(false);
  });

  // ── the switch's a11y contract + layout anchoring ────────────────────────────

  it('names the switch with the label ALONE — the hints are descriptions, not part of the name', () => {
    // With both hint lines up, a name built from the button's text content would
    // read "ATS-safe mode <résumé hint> <letter hint> <both-docs note>". Assert the
    // EXACT name so that concatenation is a failure, not a passing substring.
    renderStep({ templateId: 'atelier', target: 'both', letterLayoutId: 'monogram' });
    const atsSwitch = screen.getByRole('switch');
    expect(atsSwitch).toHaveAccessibleName('aiGenerate.atsMode');

    // …and every hint line it points at is really in the document, under the
    // same render guard (no dangling aria-describedby ids).
    const described = atsSwitch.getAttribute('aria-describedby')?.split(' ') ?? [];
    expect(described).toHaveLength(3);
    for (const id of described) expect(document.getElementById(id)).not.toBeNull();
    expect(atsSwitch).toHaveAccessibleDescription(
      /aiGenerate\.atsModeHintTwoColumn.*aiGenerate\.atsModeHintLetter.*aiGenerate\.atsModeHintBothDocs/s
    );
  });

  it('points aria-describedby at exactly the hint lines that render (résumé only)', () => {
    renderStep({ templateId: 'atelier', target: 'resume' });
    const described = screen.getByRole('switch').getAttribute('aria-describedby')?.split(' ') ?? [];
    expect(described).toHaveLength(1);
    expect(document.getElementById(described[0] as string)?.textContent).toBe(
      'aiGenerate.atsModeHintTwoColumn'
    );
  });

  it('says ONE switch drives both documents only when it actually drives both', () => {
    const { unmount } = renderStep({
      templateId: 'atelier',
      target: 'both',
      letterLayoutId: 'monogram',
    });
    expect(screen.getByText('aiGenerate.atsModeHintBothDocs')).toBeInTheDocument();
    unmount();

    // Résumé is ATS-tier → the switch drives the letter only; the note would lie.
    renderStep({ templateId: 'classic', target: 'both', letterLayoutId: 'monogram' });
    expect(screen.queryByText('aiGenerate.atsModeHintBothDocs')).not.toBeInTheDocument();
  });

  it('anchors the switch track to the label line (items-start, not items-center)', () => {
    // jsdom zeroes rects, so the class IS the assertion: with two free-wrapping
    // hint lines the row grows to ~80–90px and `items-center` drops the 16px
    // track that far below the label it belongs to.
    renderStep({ templateId: 'atelier', target: 'both', letterLayoutId: 'monogram' });
    const atsSwitch = screen.getByRole('switch');
    expect(atsSwitch.className).toContain('items-start');
    expect(atsSwitch.className).not.toContain('items-center');
  });
});
