import { beforeEach, describe, expect, it, type Mock, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

import { TEMPLATES } from '@/lib/generate';

import { StepTemplate } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// TEMPLATE_PREVIEWS, COVER_TEMPLATE_PREVIEWS, and TEMPLATE_CAPTIONS use
// import.meta.glob — stub them all so no Vite transform is needed in jsdom.
// Distinct non-empty URLs per template id so thumbnail-source tests can assert
// which preview set is used. The factory must be self-contained (vi.mock is hoisted).
vi.mock('../../../samples', () => {
  const ids = [
    'classic',
    'swiss-minimal',
    'academic',
    'atelier',
    'meridian',
    'throughline',
    'portrait',
    'lebenslauf',
  ] as const;
  const resumePreviews = Object.fromEntries(ids.map((id) => [id, `resume-${id}.png`]));
  const coverPreviews = Object.fromEntries(ids.map((id) => [id, `cover-${id}.svg`]));
  return {
    TEMPLATE_PREVIEWS: resumePreviews as Record<string, string>,
    COVER_TEMPLATE_PREVIEWS: coverPreviews as Record<string, string>,
    TEMPLATE_CAPTIONS: {} as Record<string, string>,
  };
});

describe('StepTemplate', () => {
  let onTemplateChange: Mock;
  let onAtsModeChange: Mock;

  beforeEach(() => {
    onTemplateChange = vi.fn();
    onAtsModeChange = vi.fn();
  });

  it('renders a button for every template', () => {
    render(
      <StepTemplate
        templateId="classic"
        atsMode={false}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
      />
    );
    for (const tpl of Object.values(TEMPLATES)) {
      expect(screen.getByText(tpl.name)).toBeInTheDocument();
    }
  });

  it('calls onTemplateChange with the clicked template id', async () => {
    const user = userEvent.setup();
    render(
      <StepTemplate
        templateId="classic"
        atsMode={false}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
      />
    );
    // Click the "ATS Classic" template button (id = "classic")
    const classicButton = screen.getByText('ATS Classic').closest('button');
    if (!classicButton) throw new Error('ATS Classic button not found');
    await user.click(classicButton);
    expect(onTemplateChange).toHaveBeenCalledWith('classic');
  });

  it('calls onAtsModeChange(false) when a single-column template is selected', async () => {
    const user = userEvent.setup();
    render(
      <StepTemplate
        templateId="atelier" // two-column (atsMode currently true)
        atsMode={true}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
      />
    );
    // "ATS Classic" is single-column — selecting it must reset ATS mode
    const classicButton = screen.getByText('ATS Classic').closest('button');
    if (!classicButton) throw new Error('ATS Classic button not found');
    await user.click(classicButton);
    expect(onTemplateChange).toHaveBeenCalledWith('classic');
    expect(onAtsModeChange).toHaveBeenCalledWith(false);
  });

  it('does NOT call onAtsModeChange when a two-column template is selected', async () => {
    const user = userEvent.setup();
    render(
      <StepTemplate
        templateId="classic"
        atsMode={false}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
      />
    );
    // "Atelier" is two-column — no ATS reset
    const atelierButton = screen.getByText('Atelier').closest('button');
    if (!atelierButton) throw new Error('Atelier button not found');
    await user.click(atelierButton);
    expect(onTemplateChange).toHaveBeenCalledWith('atelier');
    expect(onAtsModeChange).not.toHaveBeenCalled();
  });

  it('shows the ATS toggle for two-column templates', () => {
    render(
      <StepTemplate
        templateId="atelier"
        atsMode={false}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
      />
    );
    expect(screen.getByRole('switch', { name: /aiGenerate\.atsMode/i })).toBeInTheDocument();
  });

  it('does not show the ATS toggle for single-column templates', () => {
    render(
      <StepTemplate
        templateId="classic"
        atsMode={false}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
      />
    );
    expect(screen.queryByRole('switch')).not.toBeInTheDocument();
  });

  it('toggles ATS mode when the switch is clicked', async () => {
    const user = userEvent.setup();
    render(
      <StepTemplate
        templateId="atelier"
        atsMode={false}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
      />
    );
    const atsSwitch = screen.getByRole('switch');
    await user.click(atsSwitch);
    expect(onAtsModeChange).toHaveBeenCalledWith(true);
  });

  // ── target='cover' behaviour ────────────────────────────────────────────────

  it('target=cover: hides the ATS toggle even for a two-column template', () => {
    // "atelier" is two-column — the toggle would normally appear for résumé.
    render(
      <StepTemplate
        templateId="atelier"
        atsMode={false}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
        target="cover"
      />
    );
    expect(screen.queryByRole('switch')).not.toBeInTheDocument();
  });

  it('target=cover: hides the ATS toggle for portrait (two-column) as well', () => {
    render(
      <StepTemplate
        templateId="portrait"
        atsMode={true}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
        target="cover"
      />
    );
    expect(screen.queryByRole('switch')).not.toBeInTheDocument();
  });

  it('target=cover: renders all template buttons and fires onTemplateChange on click', async () => {
    const user = userEvent.setup();
    render(
      <StepTemplate
        templateId="classic"
        atsMode={false}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
        target="cover"
      />
    );

    // Gallery is still present — all template names should be visible.
    for (const tpl of Object.values(TEMPLATES)) {
      expect(screen.getByText(tpl.name)).toBeInTheDocument();
    }

    // Clicking a template fires onTemplateChange with its id.
    const classicButton = screen.getByText('ATS Classic').closest('button');
    if (!classicButton) throw new Error('ATS Classic button not found');
    await user.click(classicButton);
    expect(onTemplateChange).toHaveBeenCalledWith('classic');
  });

  // ── regression guard — résumé behaviour unchanged ──────────────────────────

  it('target=resume (default): still shows the ATS toggle for a two-column template', () => {
    render(
      <StepTemplate
        templateId="atelier"
        atsMode={false}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
        // target omitted → defaults to 'resume'
      />
    );
    expect(screen.getByRole('switch', { name: /aiGenerate\.atsMode/i })).toBeInTheDocument();
  });

  // ── thumbnail source tests ──────────────────────────────────────────────────

  it("target='both' uses résumé thumbnails (not cover)", () => {
    render(
      <StepTemplate
        templateId="classic"
        atsMode={false}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
        target="both"
      />
    );
    // "ATS Classic" image should be the résumé stub, not the cover stub.
    const classicImg = screen.getByAltText('ATS Classic');
    expect(classicImg.getAttribute('src')).toContain('resume-classic.png');
    expect(classicImg.getAttribute('src')).not.toContain('cover-classic.svg');
  });

  it("target='cover' uses cover thumbnails", () => {
    render(
      <StepTemplate
        templateId="classic"
        atsMode={false}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
        target="cover"
      />
    );
    const classicImg = screen.getByAltText('ATS Classic');
    expect(classicImg.getAttribute('src')).toContain('cover-classic.svg');
    expect(classicImg.getAttribute('src')).not.toContain('resume-classic.png');
  });

  it("target='cover': selecting a single-column template does NOT call onAtsModeChange", async () => {
    const user = userEvent.setup();
    render(
      <StepTemplate
        templateId="atelier"
        atsMode={true}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
        target="cover"
      />
    );
    const classicButton = screen.getByText('ATS Classic').closest('button');
    if (!classicButton) throw new Error('ATS Classic button not found');
    await user.click(classicButton);
    expect(onTemplateChange).toHaveBeenCalledWith('classic');
    expect(onAtsModeChange).not.toHaveBeenCalled();
  });

  it("target='resume': selecting a single-column template DOES call onAtsModeChange(false)", async () => {
    const user = userEvent.setup();
    render(
      <StepTemplate
        templateId="atelier"
        atsMode={true}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
        target="resume"
      />
    );
    const classicButton = screen.getByText('ATS Classic').closest('button');
    if (!classicButton) throw new Error('ATS Classic button not found');
    await user.click(classicButton);
    expect(onTemplateChange).toHaveBeenCalledWith('classic');
    expect(onAtsModeChange).toHaveBeenCalledWith(false);
  });

  // ── document accent picker ──────────────────────────────────────────────────

  it('omits the accent picker when onAccentChange is not provided', () => {
    render(
      <StepTemplate
        templateId="classic"
        atsMode={false}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
      />
    );
    expect(screen.queryByTestId(TEST_IDS.generation.accentDefault)).not.toBeInTheDocument();
  });

  it('renders the accent picker and forwards a swatch pick to onAccentChange', async () => {
    const user = userEvent.setup();
    const onAccentChange = vi.fn();
    render(
      <StepTemplate
        templateId="classic"
        atsMode={false}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
        onAccentChange={onAccentChange}
      />
    );
    await user.click(screen.getByTestId(`${TEST_IDS.generation.accentSwatch}-navy`));
    expect(onAccentChange).toHaveBeenCalledWith('#1B3A5C');
  });

  // ── tier grouping + badges ──────────────────────────────────────────────────

  const renderStep = (props: Partial<Parameters<typeof StepTemplate>[0]> = {}) =>
    render(
      <StepTemplate
        templateId="classic"
        atsMode={false}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
        {...props}
      />
    );

  it('groups the gallery into labeled ATS-Safe and Design sections', () => {
    renderStep();
    expect(screen.getByText('aiGenerate.tier.atsSafe')).toBeInTheDocument();
    expect(screen.getByText('aiGenerate.tier.design')).toBeInTheDocument();
  });

  it('shows a tier badge on every card (7 ATS + 3 design)', () => {
    renderStep();
    expect(screen.getAllByText('aiGenerate.tier.atsBadge')).toHaveLength(7);
    expect(screen.getAllByText('aiGenerate.tier.designBadge')).toHaveLength(3);
  });

  // ── ATS toggle gate is tier-aware (the Lebenslauf photo fix) ─────────────────

  it.each(['atelier', 'portrait', 'lebenslauf'] as const)(
    'shows the ATS toggle for the design-tier template %s',
    (id) => {
      renderStep({ templateId: id });
      expect(screen.getByRole('switch', { name: /aiGenerate\.atsMode/i })).toBeInTheDocument();
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
});
