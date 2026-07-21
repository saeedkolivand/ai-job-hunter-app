import { FormProvider, useForm, useFormContext } from 'react-hook-form';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

import type { WizardState } from '@/features/autopilot/types';

import { StepAction } from './index';

// ── Module stubs ──────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// The live active provider the renderer would use for `ai_generate` today —
// controlled per-test so we can simulate "provider changed since this
// autopilot was saved".
let stubbedActiveProvider = {
  provider: 'ollama',
  model: 'llama3',
  baseUrl: undefined as string | undefined,
  isPending: false,
};

vi.mock('@/services', () => ({
  useGenerateConfig: () => stubbedActiveProvider,
}));

// ── Fixture helpers ───────────────────────────────────────────────────────────

function makeForm(overrides: Partial<WizardState> = {}): WizardState {
  return {
    name: 'Test run',
    boards: ['linkedin'],
    query: 'react developer',
    location: '',
    workType: 'any',
    amount: 50,
    dateFilter: '',
    watchedCompaniesOnly: false,
    minMatchScore: 0,
    keywords: '',
    excludeKeywords: '',
    resumeText: '',
    assistant: false,
    schedule: 'daily',
    scheduleHour: 9,
    scheduleMinute: 0,
    ...overrides,
  };
}

/** Surfaces the live assistant fields as JSON so tests can assert value + type. */
function Probe() {
  const { watch } = useFormContext<WizardState>();
  const v = watch();
  return (
    <output data-testid={TEST_IDS.autopilot.probe}>
      {JSON.stringify({
        assistant: v.assistant,
        assistantProvider: v.assistantProvider,
        assistantModel: v.assistantModel,
        assistantBaseUrl: v.assistantBaseUrl,
      })}
    </output>
  );
}

function renderStep(overrides: Partial<WizardState> = {}) {
  function Host() {
    const methods = useForm<WizardState>({ defaultValues: makeForm(overrides) });
    return (
      <FormProvider {...methods}>
        <StepAction />
        <Probe />
      </FormProvider>
    );
  }
  return render(<Host />);
}

function readProbe(): Pick<
  WizardState,
  'assistant' | 'assistantProvider' | 'assistantModel' | 'assistantBaseUrl'
> {
  return JSON.parse(screen.getByTestId(TEST_IDS.autopilot.probe).textContent ?? '{}');
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('StepAction — AI notes toggle', () => {
  it('renders unchecked and writes no snapshot by default', () => {
    stubbedActiveProvider = {
      provider: 'ollama',
      model: 'llama3',
      baseUrl: undefined,
      isPending: false,
    };
    renderStep();
    const probe = readProbe();
    expect(probe.assistant).toBe(false);
    expect(probe.assistantProvider).toBeUndefined();
  });

  it('enabling the switch sets assistant: true WITHOUT snapshotting the provider (task #16)', async () => {
    stubbedActiveProvider = {
      provider: 'openai',
      model: 'gpt-4o',
      baseUrl: 'https://api.example.com',
      isPending: false,
    };
    const user = userEvent.setup();
    renderStep();

    await user.click(screen.getByRole('switch'));

    const probe = readProbe();
    expect(probe.assistant).toBe(true);
    // A scheduled headless run now resolves the CURRENTLY-active provider from the
    // backend store at run time, so the wizard captures nothing.
    expect(probe.assistantProvider).toBeUndefined();
    expect(probe.assistantModel).toBeUndefined();
    expect(probe.assistantBaseUrl).toBeUndefined();
  });

  it('leaves a stored (now-vestigial) snapshot untouched — the backend resolves the provider at run time', () => {
    // The autopilot was saved while "openai" was active; the user has since
    // switched the active provider to "anthropic" in Settings. The wizard no
    // longer re-snapshots — whatever was stored simply stays as-is (and is ignored
    // by the backend, which reads the live store).
    stubbedActiveProvider = {
      provider: 'anthropic',
      model: 'claude',
      baseUrl: undefined,
      isPending: false,
    };
    renderStep({
      assistant: true,
      assistantProvider: 'openai',
      assistantModel: 'gpt-4o',
      assistantBaseUrl: undefined,
    });

    const probe = readProbe();
    expect(probe.assistantProvider).toBe('openai');
    expect(probe.assistantModel).toBe('gpt-4o');
  });

  it('does NOT touch the snapshot while the toggle is off, even if it looks stale', () => {
    stubbedActiveProvider = {
      provider: 'anthropic',
      model: 'claude',
      baseUrl: undefined,
      isPending: false,
    };
    renderStep({
      assistant: false,
      assistantProvider: 'openai',
      assistantModel: 'gpt-4o',
      assistantBaseUrl: undefined,
    });

    const probe = readProbe();
    expect(probe.assistant).toBe(false);
    expect(probe.assistantProvider).toBe('openai');
    expect(probe.assistantModel).toBe('gpt-4o');
  });
});

describe('StepAction — disclosure copy + provider hint (a11y/UX fix)', () => {
  it('renders the disclosure caption as its own element, not the Switch description slot', () => {
    stubbedActiveProvider = {
      provider: 'ollama',
      model: 'llama3',
      baseUrl: undefined,
      isPending: false,
    };
    renderStep();

    // The caption text must be present in the document regardless of toggle state...
    const caption = screen.getByText('autopilot.wizard.action.assistantCaption');
    // ...and must NOT be the Switch's low-contrast internal description slot
    // (that div is only ever rendered when a `description` prop is passed to
    // `Switch` — it must no longer receive one).
    expect(caption.className).not.toContain('foreground/45');
  });

  it('does NOT render the provider hint while the toggle is off', () => {
    renderStep({ assistant: false });
    expect(
      screen.queryByText('autopilot.wizard.action.assistantProviderHint')
    ).not.toBeInTheDocument();
  });

  it('renders the provider hint once the toggle is on', async () => {
    stubbedActiveProvider = {
      provider: 'openai',
      model: 'gpt-4o',
      baseUrl: undefined,
      isPending: false,
    };
    const user = userEvent.setup();
    renderStep();

    await user.click(screen.getByRole('switch'));

    expect(screen.getByText('autopilot.wizard.action.assistantProviderHint')).toBeInTheDocument();
  });
});

describe('StepAction — no usable provider configured', () => {
  it('disables the switch and shows the no-provider caption when model is empty', () => {
    stubbedActiveProvider = { provider: 'ollama', model: '', baseUrl: undefined, isPending: false };
    renderStep();

    expect(screen.getByRole('switch')).toBeDisabled();
    expect(screen.getByText('autopilot.wizard.action.assistantNoProvider')).toBeInTheDocument();
    expect(screen.queryByText('autopilot.wizard.action.assistantCaption')).not.toBeInTheDocument();
  });

  it('never writes a snapshot while disabled, even if toggled on programmatically', async () => {
    stubbedActiveProvider = { provider: 'ollama', model: '', baseUrl: undefined, isPending: false };
    const user = userEvent.setup();
    renderStep({ assistant: true });

    // The switch is disabled, so a real click is a no-op — but the guard we're
    // testing is the effect itself, which must refuse to snapshot an empty
    // model even though `assistant` is already true.
    await user.click(screen.getByRole('switch'));

    const probe = readProbe();
    expect(probe.assistantProvider).toBeUndefined();
    expect(probe.assistantModel).toBeUndefined();
  });

  it('does NOT render the dangling provider hint when model is empty', () => {
    stubbedActiveProvider = { provider: 'ollama', model: '', baseUrl: undefined, isPending: false };
    renderStep({ assistant: true });

    expect(
      screen.queryByText('autopilot.wizard.action.assistantProviderHint')
    ).not.toBeInTheDocument();
  });
});

describe('StepAction — cold-boot caption flash (isPending guard)', () => {
  it('renders neither caption while the active config is still loading', () => {
    stubbedActiveProvider = { provider: 'ollama', model: '', baseUrl: undefined, isPending: true };
    renderStep();

    expect(
      screen.queryByText('autopilot.wizard.action.assistantNoProvider')
    ).not.toBeInTheDocument();
    expect(screen.queryByText('autopilot.wizard.action.assistantCaption')).not.toBeInTheDocument();
  });

  it('shows the real no-provider caption once loading resolves with no provider configured', () => {
    stubbedActiveProvider = { provider: 'ollama', model: '', baseUrl: undefined, isPending: false };
    renderStep();

    expect(screen.getByText('autopilot.wizard.action.assistantNoProvider')).toBeInTheDocument();
  });
});
