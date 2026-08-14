import type { ReactNode } from 'react';
import { FormProvider, useForm } from 'react-hook-form';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { buildTailorDefaults, type TailorWizardState } from '../../lib/tailor-state';
import { StepModel } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// The global model picker is out of scope here — stub it to a stable marker.
vi.mock('@/components/ui/ModelSelector', () => ({
  ModelSelector: () => <div data-testid="model-selector" />,
}));

// Controllable Ollama-key predicate — the seam under test.
let mockNeedsResearchKey = false;
vi.mock('@/hooks/use-needs-research-key', () => ({
  useNeedsResearchKey: () => mockNeedsResearchKey,
}));

function Wrapper({ children }: { children: ReactNode }) {
  const methods = useForm<TailorWizardState>({ defaultValues: buildTailorDefaults() });
  return <FormProvider {...methods}>{children}</FormProvider>;
}

const renderStep = (canUse: boolean) => render(<StepModel canUse={canUse} />, { wrapper: Wrapper });

describe('StepModel — Ollama research-key hint', () => {
  it('renders an announced (role="status", aria-live="polite") hint when the active provider is Ollama-family without the key', () => {
    mockNeedsResearchKey = true;
    renderStep(true);
    const hint = screen.getByRole('status');
    expect(hint).toHaveAttribute('aria-live', 'polite');
    expect(hint).toHaveTextContent('aiGenerate.research.ollamaKeyHint');
  });

  it('does not render the hint when the key is present / provider is not Ollama-family', () => {
    mockNeedsResearchKey = false;
    renderStep(true);
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  it('keeps the research toggle interactive regardless of the hint', async () => {
    mockNeedsResearchKey = true;
    const user = userEvent.setup();
    renderStep(true);
    const toggle = screen.getByRole('switch');
    expect(toggle).toHaveAttribute('aria-checked', 'false');
    await user.click(toggle);
    expect(toggle).toHaveAttribute('aria-checked', 'true');
  });

  it('keeps the research toggle interactive even when AI is unavailable (canUse=false)', async () => {
    mockNeedsResearchKey = false;
    const user = userEvent.setup();
    renderStep(false);
    const toggle = screen.getByRole('switch');
    await user.click(toggle);
    expect(toggle).toHaveAttribute('aria-checked', 'true');
  });
});

describe('StepModel — staged-run cost line', () => {
  it('shows the honest cost copy instead of a depth picker', () => {
    mockNeedsResearchKey = false;
    renderStep(true);
    expect(screen.getByText('autopilot.apply.wizard.model.stagedCost')).toBeInTheDocument();
    expect(screen.queryByRole('radio')).not.toBeInTheDocument();
  });
});
