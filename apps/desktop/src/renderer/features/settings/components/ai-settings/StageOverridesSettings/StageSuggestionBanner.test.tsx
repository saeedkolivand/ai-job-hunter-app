/**
 * StageSuggestionBanner — the suggestion is offered, never applied.
 *
 * The load-bearing behaviour is the ORDER of events: rendering the banner must
 * write nothing, and only the click writes — one override row per stage, all
 * pointing at the suggested model. A partial failure has to say how far it got,
 * because some stages really are pinned by then.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
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

const setStageOverrideAsync = vi.fn().mockResolvedValue({});
const inspectionState: { byModel: Record<string, unknown>; isPending: boolean } = {
  byModel: {
    'qwen3:32b': { parameterSize: '32.8B', family: 'qwen3' },
    'llama3.2:1b': { parameterSize: '1.2B', family: 'llama' },
  },
  isPending: false,
};
/** Records what the banner asked to be probed — the cost it imposes. */
const inspectedWith = vi.fn();

vi.mock('@/services', () => ({
  useSetStageOverride: () => ({ mutateAsync: setStageOverrideAsync, isPending: false }),
  useModelInspections: (models: string[]) => {
    inspectedWith(models);
    return inspectionState;
  },
}));

import { StageSuggestionBanner } from './StageSuggestionBanner';

const props = {
  activeProvider: 'ollama',
  activeModel: 'qwen3:32b',
  installedModels: ['qwen3:32b', 'llama3.2:1b'],
  overrides: {},
};

describe('StageSuggestionBanner', () => {
  beforeEach(() => {
    // Call history only — `clearAllMocks` keeps the resolved-value
    // implementations these cases rely on.
    vi.clearAllMocks();
  });

  it('renders the offer without writing anything', () => {
    render(<StageSuggestionBanner {...props} />);

    expect(screen.getByText(/settings\.ai\.stages\.suggest\.title/)).toBeVisible();
    expect(screen.getByText('settings.ai.stages.suggest.apply')).toBeVisible();
    // The whole point of a suggestion: nothing is pinned until it is accepted.
    expect(setStageOverrideAsync).not.toHaveBeenCalled();
  });

  it('quotes the MEASURED sizes of both models', () => {
    render(<StageSuggestionBanner {...props} />);

    // 1.2B replacing 32.8B — the numbers make the claim checkable instead of
    // asking the user to trust a name-parsed guess.
    expect(screen.getByText(/llama3\.2:1b 1\.2 qwen3:32b 32\.8/)).toBeVisible();
  });

  it('names the steps it touches', () => {
    render(<StageSuggestionBanner {...props} />);

    expect(
      screen.getByText(
        'settings.ai.stages.suggest.appliesTo settings.ai.stages.names.analyze_job, settings.ai.stages.names.match_evidence, settings.ai.stages.names.strategy'
      )
    ).toBeVisible();
  });

  it('pins every offered stage to the suggested model on accept', async () => {
    const user = userEvent.setup();
    const onApplied = vi.fn();
    render(<StageSuggestionBanner {...props} onApplied={onApplied} />);

    await user.click(screen.getByText('settings.ai.stages.suggest.apply'));

    expect(setStageOverrideAsync).toHaveBeenCalledTimes(3);
    for (const stage of ['analyze_job', 'match_evidence', 'strategy']) {
      expect(setStageOverrideAsync).toHaveBeenCalledWith({
        stage,
        provider: 'ollama',
        model: 'llama3.2:1b',
      });
    }
    expect(mockNotify.success).toHaveBeenCalled();
    // The accepted banner unmounts; focus has to go somewhere deliberate.
    expect(onApplied).toHaveBeenCalledWith(['analyze_job', 'match_evidence', 'strategy']);
  });

  it('reports how far it got when a write fails mid-way', async () => {
    setStageOverrideAsync.mockReset();
    setStageOverrideAsync
      .mockResolvedValueOnce({})
      .mockRejectedValueOnce(new Error('daily cap reached'));
    const user = userEvent.setup();
    const onApplied = vi.fn();
    render(<StageSuggestionBanner {...props} onApplied={onApplied} />);

    await user.click(screen.getByText('settings.ai.stages.suggest.apply'));

    // "Failed" alone would hide that one stage IS pinned now.
    expect(mockNotify.error).toHaveBeenCalledWith({
      message: 'settings.ai.stages.suggest.partial 1 3 daily cap reached',
    });
    expect(onApplied).toHaveBeenCalledWith(['analyze_job']);
    setStageOverrideAsync.mockResolvedValue({});
  });

  it('offers a decline only when the host owns the dismissal', async () => {
    const user = userEvent.setup();
    const onDismiss = vi.fn();
    const { rerender } = render(<StageSuggestionBanner {...props} onDismiss={onDismiss} />);

    await user.click(screen.getByText('settings.ai.stages.suggest.dismiss'));
    expect(onDismiss).toHaveBeenCalled();
    expect(setStageOverrideAsync).not.toHaveBeenCalled();

    // Without a handler there is no decline button at all — dismissal state the
    // host cannot see is state that desynchronises what it renders instead.
    rerender(<StageSuggestionBanner {...props} />);
    expect(screen.queryByText('settings.ai.stages.suggest.dismiss')).toBeNull();
  });

  it('probes no models for a provider that can never get a suggestion', () => {
    render(<StageSuggestionBanner {...props} activeProvider="openai" activeModel="gpt-5.1" />);

    // One `/api/show` per installed model, for an answer that is always null.
    expect(inspectedWith).toHaveBeenLastCalledWith([]);
  });

  it('probes the installed models when a suggestion is possible', () => {
    render(<StageSuggestionBanner {...props} />);

    expect(inspectedWith).toHaveBeenLastCalledWith(['qwen3:32b', 'llama3.2:1b']);
  });

  it('renders nothing when no measured size makes a smaller model provable', () => {
    const { container } = render(
      <StageSuggestionBanner {...props} installedModels={['qwen3:32b']} />
    );

    expect(container).toBeEmptyDOMElement();
  });
});
