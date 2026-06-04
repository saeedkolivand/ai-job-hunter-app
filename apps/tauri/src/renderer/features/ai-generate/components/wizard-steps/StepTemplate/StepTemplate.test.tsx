import { beforeEach, describe, expect, it, type Mock, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEMPLATES } from '@/lib/generate';

import { StepTemplate } from './index';

vi.mock('@/lib/i18n', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// TEMPLATE_PREVIEWS and TEMPLATE_CAPTIONS use import.meta.glob — stub them.
vi.mock('../../../samples', () => ({
  TEMPLATE_PREVIEWS: {} as Record<string, string>,
  TEMPLATE_CAPTIONS: {} as Record<string, string>,
}));

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
        templateId="modern"
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
        templateId="modern"
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
        templateId="modern"
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
});
