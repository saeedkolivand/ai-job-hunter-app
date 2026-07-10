import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { Button } from '@ajh/ui';

import { ResumeBuilderPage } from './index';

// ── Module stubs ──────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@/components/ui/ModelSelector', () => ({
  ModelSelector: () => <div data-testid="model-selector-stub" />,
}));

vi.mock('@/components/ui/AiSetupHint', () => ({
  AiSetupHint: () => null,
}));

// BuilderWizard stub: exposes just enough to drive the reset-gate behavior
// (mirrors the GenerateWizard test's StepTemplate stub pattern) without
// mounting the real react-hook-form wizard.
vi.mock('../BuilderWizard', () => ({
  BuilderWizard: ({ onTemplateChange }: { onTemplateChange: (id: string) => void }) => (
    <div>
      <Button onClick={() => onTemplateChange('atelier')}>select-atelier</Button>
      <Button onClick={() => onTemplateChange('lebenslauf')}>select-lebenslauf</Button>
      <Button onClick={() => onTemplateChange('classic')}>select-classic</Button>
    </div>
  ),
}));

const mockSetResumeBuilder = vi.fn();

vi.mock('../../hooks/useResumeBuilder', () => ({
  useResumeBuilder: () => ({
    language: 'en',
    locale: 'en',
    templateId: 'classic',
    atsMode: false,
    stage: 'interview',
    output: '',
    setResumeBuilder: mockSetResumeBuilder,
    meta: {},
    canUseAI: true,
    aiReason: '',
    isComplete: false,
    isGenerating: false,
    streamBuffer: '',
    thinkingBuffer: '',
    modelLoading: false,
    tokenCount: 0,
    tokenStartMs: null,
    error: null,
    synthesize: vi.fn(),
    tailorToJob: vi.fn(),
    reset: vi.fn(),
  }),
}));

describe('ResumeBuilderPage — template reset gate', () => {
  beforeEach(() => {
    mockSetResumeBuilder.mockClear();
  });

  it('resets atsMode when an ATS-tier template is selected', async () => {
    const user = userEvent.setup();
    render(<ResumeBuilderPage />);
    await user.click(screen.getByRole('button', { name: 'select-classic' }));
    expect(mockSetResumeBuilder).toHaveBeenCalledWith({ templateId: 'classic', atsMode: false });
  });

  it('does NOT reset atsMode when selecting Lebenslauf (design tier)', async () => {
    const user = userEvent.setup();
    render(<ResumeBuilderPage />);
    await user.click(screen.getByRole('button', { name: 'select-lebenslauf' }));
    expect(mockSetResumeBuilder).toHaveBeenCalledWith({ templateId: 'lebenslauf' });
    expect(mockSetResumeBuilder).not.toHaveBeenCalledWith(
      expect.objectContaining({ atsMode: false })
    );
  });

  it('does NOT reset atsMode when selecting a two-column template (Atelier)', async () => {
    const user = userEvent.setup();
    render(<ResumeBuilderPage />);
    await user.click(screen.getByRole('button', { name: 'select-atelier' }));
    expect(mockSetResumeBuilder).toHaveBeenCalledWith({ templateId: 'atelier' });
  });
});
