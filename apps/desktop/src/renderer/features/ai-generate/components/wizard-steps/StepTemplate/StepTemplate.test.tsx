// Gallery, selection, thumbnails, accent picker, tier badges, captions.
// ATS toggle → `ats-toggle.test.tsx`; letter layout → `letter-layout.test.tsx`.
import { beforeEach, describe, expect, it, type Mock, vi } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

import { TEMPLATES } from '@/lib/generate';

import { StepTemplate } from './index';
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

  it('renders a button for every template', () => {
    renderStep({
      templateId: 'classic',
      atsMode: false,
    });
    for (const tpl of Object.values(TEMPLATES)) {
      expect(screen.getByText(tpl.name)).toBeInTheDocument();
    }
  });

  it('calls onTemplateChange with the clicked template id', async () => {
    const user = userEvent.setup();
    renderStep({
      templateId: 'classic',
      atsMode: false,
    });
    // Click the "ATS Classic" template button (id = "classic")
    const classicButton = screen.getByText('ATS Classic').closest('button');
    if (!classicButton) throw new Error('ATS Classic button not found');
    await user.click(classicButton);
    expect(onTemplateChange).toHaveBeenCalledWith('classic');
  });

  it('calls onAtsModeChange(false) when a single-column template is selected', async () => {
    const user = userEvent.setup();
    renderStep({
      templateId: 'atelier', // two-column (atsMode currently true)
      atsMode: true,
    });
    // "ATS Classic" is single-column — selecting it must reset ATS mode
    const classicButton = screen.getByText('ATS Classic').closest('button');
    if (!classicButton) throw new Error('ATS Classic button not found');
    await user.click(classicButton);
    expect(onTemplateChange).toHaveBeenCalledWith('classic');
    expect(onAtsModeChange).toHaveBeenCalledWith(false);
  });

  it('does NOT call onAtsModeChange when a two-column template is selected', async () => {
    const user = userEvent.setup();
    renderStep({
      templateId: 'classic',
      atsMode: false,
    });
    // "Atelier" is two-column — no ATS reset
    const atelierButton = screen.getByText('Atelier').closest('button');
    if (!atelierButton) throw new Error('Atelier button not found');
    await user.click(atelierButton);
    expect(onTemplateChange).toHaveBeenCalledWith('atelier');
    expect(onAtsModeChange).not.toHaveBeenCalled();
  });

  it('shows the ATS toggle for two-column templates', () => {
    renderStep({
      templateId: 'atelier',
      atsMode: false,
    });
    expect(screen.getByRole('switch', { name: 'aiGenerate.atsMode' })).toBeInTheDocument();
  });

  it('does not show the ATS toggle for single-column templates', () => {
    renderStep({
      templateId: 'classic',
      atsMode: false,
    });
    expect(screen.queryByRole('switch')).not.toBeInTheDocument();
  });

  it('toggles ATS mode when the switch is clicked', async () => {
    const user = userEvent.setup();
    renderStep({
      templateId: 'atelier',
      atsMode: false,
    });
    const atsSwitch = screen.getByRole('switch');
    await user.click(atsSwitch);
    expect(onAtsModeChange).toHaveBeenCalledWith(true);
  });

  // ── target='cover' behaviour ────────────────────────────────────────────────

  it('target=cover: hides the ATS toggle even for a two-column template', () => {
    // "atelier" is two-column — the toggle would normally appear for résumé.
    renderStep({
      templateId: 'atelier',
      atsMode: false,
      target: 'cover',
    });
    expect(screen.queryByRole('switch')).not.toBeInTheDocument();
  });

  it('target=cover: hides the ATS toggle for portrait (two-column) as well', () => {
    renderStep({
      templateId: 'portrait',
      atsMode: true,
      target: 'cover',
    });
    expect(screen.queryByRole('switch')).not.toBeInTheDocument();
  });

  it('target=cover: renders all template buttons and fires onTemplateChange on click', async () => {
    const user = userEvent.setup();
    renderStep({
      templateId: 'classic',
      atsMode: false,
      target: 'cover',
    });

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
    expect(screen.getByRole('switch', { name: 'aiGenerate.atsMode' })).toBeInTheDocument();
  });

  // ── thumbnail source tests ──────────────────────────────────────────────────

  it("target='both' uses résumé thumbnails (not cover)", () => {
    renderStep({
      templateId: 'classic',
      atsMode: false,
      target: 'both',
    });
    // "ATS Classic" image should be the résumé stub, not the cover stub.
    const classicImg = screen.getByAltText('ATS Classic');
    expect(classicImg.getAttribute('src')).toContain('resume-classic.png');
    expect(classicImg.getAttribute('src')).not.toContain('cover-classic.svg');
  });

  it("target='cover' uses cover thumbnails", () => {
    renderStep({
      templateId: 'classic',
      atsMode: false,
      target: 'cover',
    });
    const classicImg = screen.getByAltText('ATS Classic');
    expect(classicImg.getAttribute('src')).toContain('cover-classic.svg');
    expect(classicImg.getAttribute('src')).not.toContain('resume-classic.png');
  });

  it("target='cover': selecting a single-column template does NOT call onAtsModeChange", async () => {
    const user = userEvent.setup();
    renderStep({
      templateId: 'atelier',
      atsMode: true,
      target: 'cover',
    });
    const classicButton = screen.getByText('ATS Classic').closest('button');
    if (!classicButton) throw new Error('ATS Classic button not found');
    await user.click(classicButton);
    expect(onTemplateChange).toHaveBeenCalledWith('classic');
    expect(onAtsModeChange).not.toHaveBeenCalled();
  });

  it("target='resume': selecting a single-column template DOES call onAtsModeChange(false)", async () => {
    const user = userEvent.setup();
    renderStep({
      templateId: 'atelier',
      atsMode: true,
      target: 'resume',
    });
    const classicButton = screen.getByText('ATS Classic').closest('button');
    if (!classicButton) throw new Error('ATS Classic button not found');
    await user.click(classicButton);
    expect(onTemplateChange).toHaveBeenCalledWith('classic');
    expect(onAtsModeChange).toHaveBeenCalledWith(false);
  });

  // ── document accent picker ──────────────────────────────────────────────────

  it('omits the accent picker when onAccentChange is not provided', () => {
    renderStep({
      templateId: 'classic',
      atsMode: false,
    });
    expect(screen.queryByTestId(TEST_IDS.generation.accentDefault)).not.toBeInTheDocument();
  });

  it('renders the accent picker and forwards a swatch pick to onAccentChange', async () => {
    const user = userEvent.setup();
    const onAccentChange = vi.fn();
    renderStep({
      templateId: 'classic',
      atsMode: false,
      onAccentChange,
    });
    await user.click(screen.getByTestId(`${TEST_IDS.generation.accentSwatch}-navy`));
    expect(onAccentChange).toHaveBeenCalledWith('#1B3A5C');
  });

  it('groups the gallery into labeled ATS-Safe and Design sections', () => {
    renderStep();
    expect(screen.getByText('aiGenerate.tier.atsSafe')).toBeInTheDocument();
    expect(screen.getByText('aiGenerate.tier.design')).toBeInTheDocument();
  });

  it('shows a tier badge on every card (9 ATS + 7 design)', () => {
    renderStep();
    expect(screen.getAllByText('aiGenerate.tier.atsBadge')).toHaveLength(9);
    expect(screen.getAllByText('aiGenerate.tier.designBadge')).toHaveLength(7);
  });

  it('renders the cologne-navy card with an ATS badge', () => {
    // The aggregate count above would still pass if some OTHER card supplied
    // the eighth ATS badge and cologne-navy were missing entirely — so name it.
    renderStep();
    const card = screen.getByText('Cologne Navy').closest('[data-testid], button, div');
    expect(card).not.toBeNull();
    expect(screen.getByText('Cologne Navy')).toBeInTheDocument();
    expect(within(card as HTMLElement).getByText('aiGenerate.tier.atsBadge')).toBeInTheDocument();
  });

  // ── template caption resolves through i18n (#965 R7) ────────────────────────
  // TEMPLATE_CAPTIONS holds i18n KEYS; the card must call t() on them, not
  // render the key (or a raw baked-in string) verbatim.

  it('renders the translated caption for two templates, not the raw i18n keys', () => {
    // The full gallery renders regardless of the `templateId` prop (selection
    // only styles the card), so both cards are visible from a single render.
    renderStep();
    expect(screen.getByText('Best for maximum ATS safety.')).toBeInTheDocument();
    expect(screen.getByText('Best for a dense, classic single column.')).toBeInTheDocument();
    expect(screen.queryByText('aiGenerate.templateCaption.classic')).not.toBeInTheDocument();
    expect(screen.queryByText('aiGenerate.templateCaption.jake')).not.toBeInTheDocument();
  });
});
