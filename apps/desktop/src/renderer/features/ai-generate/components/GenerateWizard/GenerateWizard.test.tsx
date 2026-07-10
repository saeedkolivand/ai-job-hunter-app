import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { Button } from '@ajh/ui';

import { GenerateWizard } from './index';

// ── Module stubs ──────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// Stub TEMPLATE_PREVIEWS/CAPTIONS (import.meta.glob not available in jsdom)
vi.mock('../../../samples', () => ({
  TEMPLATE_PREVIEWS: {} as Record<string, string>,
  TEMPLATE_CAPTIONS: {} as Record<string, string>,
}));

// Stub wizard step sub-components with lightweight stand-ins so this test
// focuses purely on wizard navigation + Generate handoff, not sub-component UI.
vi.mock('../wizard-steps/StepTarget', () => ({
  // Dynamically import Button so the mock factory can reference it.
  StepTarget: ({ onTargetChange }: { onTargetChange: (v: string) => void }) => (
    <Button onClick={() => onTargetChange('cover')}>StepTarget</Button>
  ),
}));
vi.mock('../wizard-steps/StepTemplate', () => ({
  StepTemplate: ({
    onTemplateChange,
    onAtsModeChange,
  }: {
    onTemplateChange: (id: string) => void;
    onAtsModeChange: (v: boolean) => void;
  }) => (
    <div>
      <Button onClick={() => onTemplateChange('classic')}>select-classic</Button>
      <Button onClick={() => onTemplateChange('lebenslauf')}>select-lebenslauf</Button>
      <Button onClick={() => onAtsModeChange(false)}>reset-ats</Button>
    </div>
  ),
}));
vi.mock('../wizard-steps/StepFineTune', () => ({
  StepFineTune: () => <div>StepFineTune</div>,
}));

// ── Session store mock ────────────────────────────────────────────────────────

let mockWizardStep = 0;
const mockSetAIGenerate = vi.fn((patch: Record<string, unknown>) => {
  if ('wizardStep' in patch) {
    mockWizardStep = patch.wizardStep as number;
  }
});

vi.mock('@/store/session-store', () => ({
  useSessionStore: () => ({
    aiGenerate: { wizardStep: mockWizardStep },
    setAIGenerate: mockSetAIGenerate,
  }),
}));

// ── Default props ─────────────────────────────────────────────────────────────

function makeProps(overrides: Partial<Parameters<typeof GenerateWizard>[0]> = {}) {
  return {
    mode: 'ats' as const,
    emphasis: [],
    target: 'both' as const,
    templateId: 'classic' as const,
    atsMode: false,
    accent: undefined,
    letterLayoutId: undefined,
    locale: '',
    researchCompany: false,
    isGenerating: false,
    onModeChange: vi.fn(),
    onEmphasisChange: vi.fn(),
    onTargetChange: vi.fn(),
    onTemplateChange: vi.fn(),
    onAtsModeChange: vi.fn(),
    onAccentChange: vi.fn(),
    onLetterLayoutChange: vi.fn(),
    onLocaleChange: vi.fn(),
    onResearchCompanyChange: vi.fn(),
    onGenerate: vi.fn(),
    ...overrides,
  };
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('GenerateWizard — navigation', () => {
  beforeEach(() => {
    mockWizardStep = 0;
    mockSetAIGenerate.mockClear();
  });

  it('shows the current step title, description and step counter', () => {
    render(<GenerateWizard {...makeProps()} />);
    // Only the current step's descriptive title + purpose are shown (a procedure,
    // not a clickable tab strip). t() returns keys verbatim in this mock.
    expect(screen.getByText('aiGenerate.wizard.steps.0')).toBeInTheDocument();
    expect(screen.getByText('aiGenerate.wizard.descriptions.0')).toBeInTheDocument();
    expect(screen.getByText('aiGenerate.wizard.stepCounter')).toBeInTheDocument();
  });

  it('shows step 0 content (StepTarget) on first render', () => {
    render(<GenerateWizard {...makeProps()} />);
    expect(screen.getByText('StepTarget')).toBeInTheDocument();
  });

  it('selecting a target on step 0 advances to step 1 (#10 — no separate Next)', async () => {
    const user = userEvent.setup();
    const onTargetChange = vi.fn();
    render(<GenerateWizard {...makeProps({ onTargetChange })} />);
    // Step 0 has no Next button — picking a target doubles as advancing.
    expect(
      screen.queryByRole('button', { name: /aiGenerate\.wizard\.next/i })
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'StepTarget' }));
    expect(onTargetChange).toHaveBeenCalledWith('cover');
    expect(mockSetAIGenerate).toHaveBeenCalledWith(expect.objectContaining({ wizardStep: 1 }));
  });

  it('Back button is visually disabled (disabled attr) on step 0', () => {
    render(<GenerateWizard {...makeProps()} />);
    const backBtn = screen.getByRole('button', { name: /aiGenerate\.wizard\.back/i });
    expect(backBtn).toBeDisabled();
  });

  it('shows Generate button on last step (step 2)', () => {
    mockWizardStep = 2;
    render(<GenerateWizard {...makeProps()} />);
    expect(
      screen.getByRole('button', { name: /aiGenerate\.wizard\.generate/i })
    ).toBeInTheDocument();
  });

  it('clicking Back on step 1 sets wizardStep to 0', async () => {
    mockWizardStep = 1;
    const user = userEvent.setup();
    render(<GenerateWizard {...makeProps()} />);
    await user.click(screen.getByRole('button', { name: /aiGenerate\.wizard\.back/i }));
    expect(mockSetAIGenerate).toHaveBeenCalledWith(expect.objectContaining({ wizardStep: 0 }));
  });

  it('Next button is replaced by Generate on step 2 (last step)', () => {
    mockWizardStep = 2;
    render(<GenerateWizard {...makeProps()} />);
    expect(
      screen.queryByRole('button', { name: /aiGenerate\.wizard\.next/i })
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /aiGenerate\.wizard\.generate/i })
    ).toBeInTheDocument();
  });
});

describe('GenerateWizard — Generate handoff', () => {
  beforeEach(() => {
    mockWizardStep = 2;
    mockSetAIGenerate.mockClear();
  });

  it('calls onGenerate when the Generate button is clicked on step 2', async () => {
    const user = userEvent.setup();
    const onGenerate = vi.fn();
    render(<GenerateWizard {...makeProps({ onGenerate })} />);
    await user.click(screen.getByRole('button', { name: /aiGenerate\.wizard\.generate/i }));
    expect(onGenerate).toHaveBeenCalledOnce();
  });

  it('disables and shows loading state on Generate button when isGenerating=true', () => {
    render(<GenerateWizard {...makeProps({ isGenerating: true })} />);
    const btn = screen.getByRole('button', { name: /aiGenerate\.generating/i });
    expect(btn).toBeDisabled();
  });
});

describe('GenerateWizard — template selection side-effects', () => {
  beforeEach(() => {
    mockWizardStep = 1;
    mockSetAIGenerate.mockClear();
  });

  it('selecting a template calls onTemplateChange with the id', async () => {
    const user = userEvent.setup();
    const onTemplateChange = vi.fn();
    render(<GenerateWizard {...makeProps({ onTemplateChange })} />);
    await user.click(screen.getByRole('button', { name: 'select-classic' }));
    expect(onTemplateChange).toHaveBeenCalledWith('classic');
  });

  it('selecting a single-column template resets ATS mode via onAtsModeChange(false)', async () => {
    const user = userEvent.setup();
    const onAtsModeChange = vi.fn();
    render(<GenerateWizard {...makeProps({ onAtsModeChange })} />);
    // StepTemplate stub calls onAtsModeChange(false) via the "reset-ats" button
    await user.click(screen.getByRole('button', { name: 'reset-ats' }));
    expect(onAtsModeChange).toHaveBeenCalledWith(false);
  });

  it('selecting Lebenslauf (design tier) does NOT reset ATS mode', async () => {
    const user = userEvent.setup();
    const onAtsModeChange = vi.fn();
    const onTemplateChange = vi.fn();
    render(<GenerateWizard {...makeProps({ onTemplateChange, onAtsModeChange })} />);
    await user.click(screen.getByRole('button', { name: 'select-lebenslauf' }));
    expect(onTemplateChange).toHaveBeenCalledWith('lebenslauf');
    expect(onAtsModeChange).not.toHaveBeenCalled();
  });
});
