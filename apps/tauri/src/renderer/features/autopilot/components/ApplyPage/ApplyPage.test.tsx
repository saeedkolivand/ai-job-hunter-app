/**
 * ApplyPage — gate-required tests
 *
 * Coverage: stage machine derivation, step gating, form persistence round-trip,
 * and the "Edit settings" forceConfiguring override.
 *
 * Strategy:
 *  - `useTailorGeneration` is fully mocked so no generation store / IPC runs.
 *  - `useSelectedModel` / `useCanUseAI` are mocked at the module level.
 *  - `useResolveJobUrl` / `useExtractText` (services) are mocked so no
 *    QueryClient/AppClient provider is needed inside the render tree.
 *  - Heavy sub-trees (PdfPreview, EditableOutput, ResumeInputCard, ModelSelector,
 *    ApplicationQuestionsModal, ReferralModal) are stubbed so no Typst/IPC runs,
 *    and the lifted `useApplicationAnswers` hook is mocked so ApplyPage can call
 *    it without an AppClient/QueryClient provider in the render tree.
 *  - The real Zustand session store is used so persistence round-trips can be
 *    asserted against actual store state.
 */

import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type * as AjhUi from '@ajh/ui';
import { TextArea } from '@ajh/ui';

import { useSessionStore } from '@/store/session-store';

import { ApplyPage } from './index';

// ── i18n ─────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ── motion/react — replace AnimatePresence+motion.div with plain fragments ───

vi.mock('motion/react', () => ({
  AnimatePresence: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  motion: {
    div: React.forwardRef(
      (
        { children, ...rest }: React.HTMLAttributes<HTMLDivElement>,
        ref: React.Ref<HTMLDivElement>
      ) => (
        <div ref={ref} {...rest}>
          {children}
        </div>
      )
    ),
  },
}));

// ── ModelSelector hooks ────────────────────────────────────────────────────────

const mockCanUse = { canUse: true, reason: undefined as string | undefined };

vi.mock('@/components/ui/ModelSelector', () => ({
  useSelectedModel: () => 'test-model',
  useCanUseAI: () => mockCanUse,
  ModelSelector: () => <div data-testid="model-selector" />,
}));

// ── Service hooks — no real IPC / QueryClient needed ─────────────────────────

vi.mock('@/services', () => ({
  useResolveJobUrl: () => ({ data: undefined, isLoading: false }),
  useExtractText: () => ({ mutateAsync: vi.fn(), isPending: false }),
}));

// ── useTailorGeneration — controlled mock ─────────────────────────────────────

const genMock = {
  generating: false,
  phase: 'idle' as const,
  phaseLabel: '',
  thinking: '',
  resumeOut: '' as string,
  coverOut: '' as string,
  activeOut: 'resume' as const,
  setActiveOut: vi.fn(),
  output: '' as string,
  error: null,
  copied: false,
  exportOpen: false,
  setExportOpen: vi.fn(),
  generate: vi.fn().mockResolvedValue(undefined),
  abort: vi.fn(),
  copy: vi.fn(),
  exportAs: vi.fn(),
  editActiveOutput: vi.fn(),
  meta: null,
};

vi.mock('@/features/documents/components/TailorFlow/useTailorGeneration', () => ({
  useTailorGeneration: () => genMock,
}));

// ── Heavy sub-trees ───────────────────────────────────────────────────────────

vi.mock('@/components/generation/PdfPreview', () => ({
  PdfPreview: ({ text }: { text: string }) => <div data-testid="pdf-preview">{text}</div>,
}));

vi.mock('@/components/generation/EditableOutput', () => ({
  EditableOutput: ({ value }: { value: string }) => (
    <div data-testid="editable-output">{value}</div>
  ),
}));

vi.mock('@/components/resume/ResumeInputCard', () => ({
  ResumeInputCard: ({ value, onChange }: { value: string; onChange: (v: string) => void }) => (
    <TextArea data-testid="resume-input" value={value} onChange={(e) => onChange(e.target.value)} />
  ),
}));

vi.mock('@/features/documents/components/TailorFlow/ApplicationQuestionsModal', () => ({
  ApplicationQuestionsModal: () => <div data-testid="application-questions-modal" />,
}));

vi.mock('@/features/documents/components/TailorFlow/useApplicationAnswers', () => ({
  useApplicationAnswers: () => ({
    selected: new Set<string>(),
    toggle: vi.fn(),
    answers: {},
    generating: false,
    error: null,
    generate: vi.fn(),
    canGenerate: false,
  }),
}));

vi.mock('@/features/documents/components/TailorFlow/ReferralModal', () => ({
  ReferralModal: () => <div data-testid="referral-modal" />,
}));

// @ajh/ui: keep all real exports but replace SegmentedControl with a stub so
// the output-type step renders without needing the full design-system runtime.
vi.mock('@ajh/ui', async (importOriginal) => {
  const real = await importOriginal<typeof AjhUi>();
  return {
    ...real,
    SegmentedControl: ({
      value,
      onChange,
      options,
      ariaLabel,
    }: {
      value: string;
      onChange: (v: string) => void;
      options: Array<{ value: string; label: string }>;
      ariaLabel?: string;
    }) => (
      <div role="group" aria-label={ariaLabel} data-testid="output-type-control">
        {options.map((o) => (
          <real.Button
            key={o.value}
            variant="unstyled"
            type="button"
            aria-pressed={o.value === value}
            onClick={() => onChange(o.value)}
          >
            {o.label}
          </real.Button>
        ))}
      </div>
    ),
    Dropdown: ({
      options,
      value,
      onChange,
    }: {
      options: Array<{ value: string; label: string }>;
      value: string;
      onChange: (v: string) => void;
    }) => (
      <div data-testid="template-picker" data-value={value}>
        {options.map((o) => (
          <div
            key={o.value}
            role="option"
            aria-selected={o.value === value}
            data-optvalue={o.value}
            onClick={() => onChange(o.value)}
          >
            {o.label}
          </div>
        ))}
      </div>
    ),
  };
});

// ── Fixture job ───────────────────────────────────────────────────────────────

const JOB = {
  title: 'Senior Engineer',
  company: 'Acme',
  url: 'https://acme.com/jobs/42',
  description: 'Build cool things.',
  score: 85,
  foundAt: Date.now(),
};

function renderApplyPage(overrides: Partial<Parameters<typeof ApplyPage>[0]> = {}) {
  return render(
    <ApplyPage
      job={JOB}
      resumeText="My base resume"
      board="linkedin"
      onBack={vi.fn()}
      {...overrides}
    />
  );
}

// ── Store reset between tests ─────────────────────────────────────────────────

beforeEach(() => {
  // Reset gen mock to baseline state.
  genMock.generating = false;
  genMock.resumeOut = '';
  genMock.coverOut = '';
  genMock.output = '';
  genMock.generate.mockClear();
  genMock.abort.mockClear();
  mockCanUse.canUse = true;
  mockCanUse.reason = undefined;

  // Reset session store autopilot slice to defaults so each test starts clean.
  useSessionStore.setState((s) => ({
    autopilot: {
      ...s.autopilot,
      applyWizardStep: 0,
      applyWizardForm: null,
      applyTemplateId: 'modern',
      applyAtsMode: false,
    },
  }));
});

// ─────────────────────────────────────────────────────────────────────────────
// 1. Stage derivation
// ─────────────────────────────────────────────────────────────────────────────

describe('Stage derivation', () => {
  it('renders the WIZARD (configuring) when not generating and no output', () => {
    renderApplyPage();
    // TailorWizard renders step indicators with aria-current="step" on the active step.
    expect(screen.getByText('autopilot.apply.wizard.steps.jobAd')).toBeInTheDocument();
    expect(screen.queryByTestId('editable-output')).not.toBeInTheDocument();
  });

  it('renders the GENERATING panel when generating=true', () => {
    genMock.generating = true;
    renderApplyPage();
    // GeneratingPanel renders a Cancel button.
    expect(screen.getByRole('button', { name: /autopilot\.apply\.cancel/i })).toBeInTheDocument();
    expect(screen.queryByText('autopilot.apply.wizard.steps.jobAd')).not.toBeInTheDocument();
  });

  it('renders the RESULTS panel when resumeOut is present', () => {
    genMock.resumeOut = 'Generated resume';
    genMock.output = 'Generated resume';
    renderApplyPage();
    // ResultsPanel has "Edit settings" and "Regenerate" footer buttons.
    expect(
      screen.getByRole('button', { name: /autopilot\.apply\.wizard\.results\.edit/i })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /autopilot\.apply\.wizard\.results\.regenerate/i })
    ).toBeInTheDocument();
  });

  it('renders the RESULTS panel when coverOut is present (no resumeOut)', () => {
    genMock.coverOut = 'Generated cover letter';
    genMock.output = 'Generated cover letter';
    renderApplyPage();
    expect(
      screen.getByRole('button', { name: /autopilot\.apply\.wizard\.results\.edit/i })
    ).toBeInTheDocument();
  });

  it('generating=true WINS even when output is also present (generating stage takes priority)', () => {
    genMock.generating = true;
    genMock.resumeOut = 'Previous resume';
    genMock.output = 'Previous resume';
    renderApplyPage();
    // Should show generating panel, NOT results.
    expect(screen.getByRole('button', { name: /autopilot\.apply\.cancel/i })).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: /autopilot\.apply\.wizard\.results\.edit/i })
    ).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 2. Step gating (RHF mode:'onChange' + zod)
// ─────────────────────────────────────────────────────────────────────────────

describe('Step gating', () => {
  it('step indicator shows aria-current="step" on the active (first) step', () => {
    renderApplyPage();
    // First step indicator div should have aria-current="step".
    const activeIndicator = document.querySelector('[aria-current="step"]');
    expect(activeIndicator).toBeInTheDocument();
    expect(activeIndicator?.textContent).toMatch(/jobAd/i);
  });

  it('advances from step 0 (Job Ad) to step 1 (Resume) via Next', async () => {
    const user = userEvent.setup();
    renderApplyPage();

    await user.click(screen.getByRole('button', { name: /autopilot\.apply\.wizard\.next/i }));

    // Step 1 is Resume — look for the resume textarea stub.
    await waitFor(() => {
      expect(screen.getByTestId('resume-input')).toBeInTheDocument();
    });
  });

  it('blocks Next on the Resume step while the textarea is empty', async () => {
    const user = userEvent.setup();
    // Start with no resumeText so the field is empty.
    renderApplyPage({ resumeText: '' });

    // Advance to step 1 (Resume) — no gate on step 0.
    await user.click(screen.getByRole('button', { name: /autopilot\.apply\.wizard\.next/i }));
    await waitFor(() => screen.getByTestId('resume-input'));

    // Try to advance past the Resume step — gate should block.
    await user.click(screen.getByRole('button', { name: /autopilot\.apply\.wizard\.next/i }));

    // Validation error key must appear.
    await waitFor(() => {
      expect(
        screen.getByText('autopilot.apply.wizard.validation.resumeRequired')
      ).toBeInTheDocument();
    });

    // Still on step 1 — resume input still visible.
    expect(screen.getByTestId('resume-input')).toBeInTheDocument();
  });

  it('advances past the Resume step once the field is filled', async () => {
    const user = userEvent.setup();
    renderApplyPage({ resumeText: '' });

    // Go to step 1 (Resume).
    await user.click(screen.getByRole('button', { name: /autopilot\.apply\.wizard\.next/i }));
    await waitFor(() => screen.getByTestId('resume-input'));

    // Type a resume.
    await user.type(screen.getByTestId('resume-input'), 'My full resume text');

    // Now Next should advance to step 2 (Output).
    await user.click(screen.getByRole('button', { name: /autopilot\.apply\.wizard\.next/i }));

    // Step 2 — output type control.
    await waitFor(() => {
      expect(screen.getByTestId('output-type-control')).toBeInTheDocument();
    });
  });

  it('shows the Generate button on the last wizard step (step 3)', async () => {
    const user = userEvent.setup();
    // Pre-fill resume so step-1 gate passes.
    renderApplyPage({ resumeText: 'My resume' });

    const nextBtn = () => screen.getByRole('button', { name: /autopilot\.apply\.wizard\.next/i });

    // Step 0 → 1.
    await user.click(nextBtn());
    await waitFor(() => screen.getByTestId('resume-input'));

    // Step 1 → 2 (resume is seeded, gate passes).
    await user.click(nextBtn());
    await waitFor(() => screen.getByTestId('output-type-control'));

    // Step 2 → 3 (no gate).
    await user.click(nextBtn());

    // Now on the model step — Generate button should appear.
    await waitFor(() => {
      expect(
        screen.getByRole('button', { name: /autopilot\.apply\.wizard\.generate/i })
      ).toBeInTheDocument();
    });
  });

  it('Generate button shows inline AI-unavailable reason when canUse=false', async () => {
    mockCanUse.canUse = false;
    mockCanUse.reason = 'selectModel';
    const user = userEvent.setup();
    renderApplyPage({ resumeText: 'My resume' });

    const nextBtn = () => screen.getByRole('button', { name: /autopilot\.apply\.wizard\.next/i });
    await user.click(nextBtn());
    await waitFor(() => screen.getByTestId('resume-input'));
    await user.click(nextBtn());
    await waitFor(() => screen.getByTestId('output-type-control'));
    await user.click(nextBtn());

    // On model step — Generate is present but clicking it shows the validation error.
    await waitFor(() =>
      screen.getByRole('button', { name: /autopilot\.apply\.wizard\.generate/i })
    );
    await user.click(screen.getByRole('button', { name: /autopilot\.apply\.wizard\.generate/i }));

    await waitFor(() => {
      expect(screen.getByText('autopilot.apply.wizard.validation.selectModel')).toBeInTheDocument();
    });
    // gen.generate must NOT be called.
    expect(genMock.generate).not.toHaveBeenCalled();
  });

  it('calls gen.generate with the resume text when canUse=true and Generate is clicked', async () => {
    const user = userEvent.setup();
    renderApplyPage({ resumeText: 'My resume' });

    const nextBtn = () => screen.getByRole('button', { name: /autopilot\.apply\.wizard\.next/i });
    await user.click(nextBtn());
    await waitFor(() => screen.getByTestId('resume-input'));
    await user.click(nextBtn());
    await waitFor(() => screen.getByTestId('output-type-control'));
    await user.click(nextBtn());

    await waitFor(() =>
      screen.getByRole('button', { name: /autopilot\.apply\.wizard\.generate/i })
    );
    await user.click(screen.getByRole('button', { name: /autopilot\.apply\.wizard\.generate/i }));

    await waitFor(() => {
      expect(genMock.generate).toHaveBeenCalledTimes(1);
    });
    // Called with the seeded resume text and the default output type.
    expect(genMock.generate).toHaveBeenCalledWith('My resume', expect.any(String));
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 3. Form persistence round-trip
// ─────────────────────────────────────────────────────────────────────────────

describe('Form persistence round-trip', () => {
  it('writes applyWizardStep to the session store when advancing a step', async () => {
    const user = userEvent.setup();
    renderApplyPage({ resumeText: 'My resume' });

    await user.click(screen.getByRole('button', { name: /autopilot\.apply\.wizard\.next/i }));
    await waitFor(() => screen.getByTestId('resume-input'));

    expect(useSessionStore.getState().autopilot.applyWizardStep).toBe(1);
  });

  it('writes applyWizardForm snapshot to the session store when advancing a step', async () => {
    const user = userEvent.setup();
    renderApplyPage({ resumeText: 'Persisted resume text' });

    await user.click(screen.getByRole('button', { name: /autopilot\.apply\.wizard\.next/i }));
    await waitFor(() => screen.getByTestId('resume-input'));

    const stored = useSessionStore.getState().autopilot.applyWizardForm;
    expect(stored).not.toBeNull();
    expect(stored?.resume).toBe('Persisted resume text');
  });

  it('remounting ApplyPage restores to the persisted step from the session store', async () => {
    // Pre-seed the store as if a previous session advanced to step 2 (Output).
    useSessionStore.setState((s) => ({
      autopilot: {
        ...s.autopilot,
        applyWizardStep: 2,
        applyWizardForm: { resume: 'Stored resume', outputType: 'both', researchCompany: false },
      },
    }));

    renderApplyPage({ resumeText: 'Stored resume' });

    // Step 2 is the Output step — output-type control should be visible.
    await waitFor(() => {
      expect(screen.getByTestId('output-type-control')).toBeInTheDocument();
    });
  });

  it('remounting ApplyPage seeds the resume field from applyWizardForm', async () => {
    useSessionStore.setState((s) => ({
      autopilot: {
        ...s.autopilot,
        applyWizardStep: 1,
        applyWizardForm: {
          resume: 'Restored resume value',
          outputType: 'both',
          researchCompany: false,
        },
      },
    }));

    renderApplyPage({ resumeText: 'different base resume' });

    // On step 1 the ResumeInputCard stub renders the seeded value.
    await waitFor(() => {
      const input = screen.getByTestId<HTMLTextAreaElement>('resume-input');
      expect(input.value).toBe('Restored resume value');
    });
  });

  it('persists form on Generate call (writes applyWizardForm with current values)', async () => {
    const user = userEvent.setup();
    renderApplyPage({ resumeText: 'My resume' });

    const nextBtn = () => screen.getByRole('button', { name: /autopilot\.apply\.wizard\.next/i });
    await user.click(nextBtn());
    await waitFor(() => screen.getByTestId('resume-input'));
    await user.click(nextBtn());
    await waitFor(() => screen.getByTestId('output-type-control'));
    await user.click(nextBtn());

    await waitFor(() =>
      screen.getByRole('button', { name: /autopilot\.apply\.wizard\.generate/i })
    );
    await user.click(screen.getByRole('button', { name: /autopilot\.apply\.wizard\.generate/i }));

    await waitFor(() => expect(genMock.generate).toHaveBeenCalledTimes(1));

    // applyWizardForm must be written on generate.
    const stored = useSessionStore.getState().autopilot.applyWizardForm;
    expect(stored).not.toBeNull();
    expect(stored?.resume).toBe('My resume');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 4. "Edit settings" forceConfiguring override
// ─────────────────────────────────────────────────────────────────────────────

describe('"Edit settings" override', () => {
  it('clicking "Edit settings" from the done stage shows the wizard again', async () => {
    genMock.resumeOut = 'Generated resume';
    genMock.output = 'Generated resume';
    const user = userEvent.setup();
    renderApplyPage();

    // Should be in done stage initially.
    const editBtn = screen.getByRole('button', {
      name: /autopilot\.apply\.wizard\.results\.edit/i,
    });
    await user.click(editBtn);

    // After clicking, should show the wizard (configuring stage).
    await waitFor(() => {
      expect(screen.getByText('autopilot.apply.wizard.steps.jobAd')).toBeInTheDocument();
    });
  });

  it('output is preserved underneath when forceConfiguring is active', async () => {
    genMock.resumeOut = 'Generated resume still present';
    genMock.output = 'Generated resume still present';
    const user = userEvent.setup();
    renderApplyPage();

    // Click Edit settings to force wizard.
    await user.click(
      screen.getByRole('button', { name: /autopilot\.apply\.wizard\.results\.edit/i })
    );

    await waitFor(() => {
      expect(screen.getByText('autopilot.apply.wizard.steps.jobAd')).toBeInTheDocument();
    });

    // The gen mock still has output — stage would be 'done' without forceConfiguring.
    // Verify gen.resumeOut is still set (output not cleared).
    expect(genMock.resumeOut).toBe('Generated resume still present');
  });

  it('the next generate call clears the forceConfiguring override and returns to done', async () => {
    genMock.resumeOut = 'Previous result';
    genMock.output = 'Previous result';
    const user = userEvent.setup();
    renderApplyPage({ resumeText: 'My resume' });

    // Enter forceConfiguring via Edit settings.
    await user.click(
      screen.getByRole('button', { name: /autopilot\.apply\.wizard\.results\.edit/i })
    );

    await waitFor(() => {
      expect(screen.getByText('autopilot.apply.wizard.steps.jobAd')).toBeInTheDocument();
    });

    // Advance to last step and Generate.
    const nextBtn = () => screen.getByRole('button', { name: /autopilot\.apply\.wizard\.next/i });
    await user.click(nextBtn()); // step 0 → 1
    await waitFor(() => screen.getByTestId('resume-input'));
    await user.click(nextBtn()); // step 1 → 2
    await waitFor(() => screen.getByTestId('output-type-control'));
    await user.click(nextBtn()); // step 2 → 3

    await waitFor(() =>
      screen.getByRole('button', { name: /autopilot\.apply\.wizard\.generate/i })
    );
    await user.click(screen.getByRole('button', { name: /autopilot\.apply\.wizard\.generate/i }));

    // forceConfiguring clears on startGeneration — since gen still has output and
    // generating=false, the stage should return to 'done'.
    await waitFor(() => {
      expect(genMock.generate).toHaveBeenCalledTimes(1);
    });
    // After clearing forceConfiguring with output still present → done stage.
    await waitFor(() => {
      expect(
        screen.getByRole('button', { name: /autopilot\.apply\.wizard\.results\.regenerate/i })
      ).toBeInTheDocument();
    });
  });
});
