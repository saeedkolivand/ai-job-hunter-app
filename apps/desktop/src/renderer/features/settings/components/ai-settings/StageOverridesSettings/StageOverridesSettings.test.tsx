/**
 * StageOverridesSettings — one assertion per STATE's positive affordance
 * (what that state renders), never "the other state's text is absent".
 *
 *  - no override        → the row reads "default — <active model>".
 *  - override           → the row reads the override's own provider + model and
 *                         offers a reset control.
 *  - unconfigured       → the row warns BEFORE a run (the backend refuses at
 *                         run time rather than falling back).
 *  - reset              → clears exactly that stage.
 *  - stage vocabulary   → exactly the stages that make a provider call.
 */
import { describe, expect, it, vi } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type * as AjhUi from '@ajh/ui';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (key: string, params?: Record<string, unknown>) =>
      params ? `${key} ${Object.values(params).join(' ')}` : key,
  }),
}));

const mockNotify = {
  open: vi.fn(),
  success: vi.fn(),
  error: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
  destroy: vi.fn(),
};

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return { ...actual, useNotification: () => mockNotify };
});

const clearMutate = vi.fn();
const setMutate = vi.fn();
const overridesState: { data: Record<string, unknown> } = { data: {} };

vi.mock('@/services', () => ({
  useStageOverrides: () => overridesState,
  useClearStageOverride: () => ({ mutate: clearMutate, isPending: false }),
  useSetStageOverride: () => ({ mutate: setMutate, isPending: false }),
  useListProviderModels: () => ({ data: { models: [], cached: false } }),
}));

// The advisor has its own suite (and its own service surface); here only the
// entry point matters — that the button actually opens it.
vi.mock('../ModelAdvisor', () => ({
  ModelAdvisor: ({ open }: { open: boolean }) => (open ? <div>advisor-open</div> : null),
}));

import { StageOverridesSettings } from './index';
import { OVERRIDABLE_PIPELINE_STAGES } from './stage-routing';

const baseProps = {
  activeProvider: 'ollama',
  activeModel: 'qwen3:8b',
  ollamaModels: [{ name: 'qwen3:8b' }, { name: 'llama3.2:1b' }],
  isConfigured: () => true,
  configuredBaseUrl: undefined,
};

const rowFor = (stage: string) => {
  const label = screen.getByText(`settings.ai.stages.names.${stage}`);
  const row = label.closest('div.rounded-lg');
  if (!row) throw new Error(`no row rendered for stage ${stage}`);
  return within(row as HTMLElement);
};

describe('StageOverridesSettings — a stage with no override', () => {
  it('reads as the active model, marked as the default', () => {
    overridesState.data = {};
    render(<StageOverridesSettings {...baseProps} />);

    expect(rowFor('draft').getByText('settings.ai.stages.defaultModel qwen3:8b')).toBeVisible();
  });

  it('renders exactly the stages that make a provider call', () => {
    overridesState.data = {};
    render(<StageOverridesSettings {...baseProps} />);

    const rendered = OVERRIDABLE_PIPELINE_STAGES.filter((stage) =>
      screen.queryByText(`settings.ai.stages.names.${stage}`)
    );
    expect(rendered).toEqual([...OVERRIDABLE_PIPELINE_STAGES]);
    // `assemble`/`validate` make no provider call and are refused server-side,
    // so the vocabulary rendered is the overridable one, not PIPELINE_STAGES.
    expect(screen.getAllByText(/^settings\.ai\.stages\.names\./)).toHaveLength(
      OVERRIDABLE_PIPELINE_STAGES.length
    );
  });
});

describe('StageOverridesSettings — advisor entry point', () => {
  it('opens the advisor from the section header', async () => {
    overridesState.data = {};
    const user = userEvent.setup();
    render(<StageOverridesSettings {...baseProps} />);

    await user.click(screen.getByText('settings.ai.advisor.open'));

    expect(screen.getByText('advisor-open')).toBeVisible();
  });
});

describe('StageOverridesSettings — an overridden stage', () => {
  it('reads as the override’s own provider and model', () => {
    overridesState.data = { analyze_job: { provider: 'openai', model: 'gpt-5.1-mini' } };
    render(<StageOverridesSettings {...baseProps} />);

    expect(rowFor('analyze_job').getByText('OpenAI · gpt-5.1-mini')).toBeVisible();
    // Per-STAGE, not global: the untouched stage still reads as the default.
    // (Also proves `rowFor` scopes to one row rather than the whole card.)
    expect(rowFor('draft').getByText('settings.ai.stages.defaultModel qwen3:8b')).toBeVisible();
  });

  it('offers a reset control that clears exactly that stage', async () => {
    overridesState.data = { analyze_job: { provider: 'openai', model: 'gpt-5.1-mini' } };
    const user = userEvent.setup();
    render(<StageOverridesSettings {...baseProps} />);

    await user.click(rowFor('analyze_job').getByText('settings.ai.stages.reset'));

    expect(clearMutate).toHaveBeenCalledWith('analyze_job', expect.anything());
  });
});

describe('StageOverridesSettings — an override the backend will refuse', () => {
  it('warns on the row when the override’s provider is not configured', () => {
    overridesState.data = { llm_judge: { provider: 'anthropic', model: 'claude-sonnet-4-6' } };
    render(<StageOverridesSettings {...baseProps} isConfigured={(p) => p === 'ollama'} />);

    expect(
      rowFor('llm_judge').getByText(/settings\.ai\.stages\.warnUnconfigured Anthropic/)
    ).toBeVisible();
  });

  it('warns when the resolved provider has no model at all', () => {
    overridesState.data = {};
    render(
      <StageOverridesSettings {...baseProps} activeProvider="openai" activeModel={undefined} />
    );

    expect(rowFor('draft').getByText(/settings\.ai\.stages\.warnNoModel OpenAI/)).toBeVisible();
  });
});
