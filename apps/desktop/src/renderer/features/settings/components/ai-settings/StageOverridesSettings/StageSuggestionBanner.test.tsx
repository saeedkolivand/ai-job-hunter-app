/**
 * StageSuggestionBanner — the suggestion is offered, never applied.
 *
 * The load-bearing behaviour is the ORDER of events: rendering the banner must
 * write nothing, and only the click writes — one override row per stage, all
 * pointing at the suggested model.
 */
import { describe, expect, it, vi } from 'vitest';
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

vi.mock('@/services', () => ({
  useSetStageOverride: () => ({ mutateAsync: setStageOverrideAsync, isPending: false }),
}));

import { StageSuggestionBanner } from './StageSuggestionBanner';

const props = {
  activeProvider: 'ollama',
  activeModel: 'qwen3:32b',
  installedModels: ['qwen3:32b', 'llama3.2:1b'],
  overrides: {},
};

describe('StageSuggestionBanner', () => {
  it('renders the offer without writing anything', () => {
    render(<StageSuggestionBanner {...props} />);

    expect(screen.getByText(/settings\.ai\.stages\.suggest\.title/)).toBeVisible();
    expect(screen.getByText('settings.ai.stages.suggest.apply')).toBeVisible();
    // The whole point of a suggestion: nothing is pinned until it is accepted.
    expect(setStageOverrideAsync).not.toHaveBeenCalled();
  });

  it('names the model, the model it replaces, and the steps it touches', () => {
    render(<StageSuggestionBanner {...props} />);

    expect(screen.getByText(/llama3\.2:1b/)).toBeVisible();
    expect(
      screen.getByText(
        'settings.ai.stages.suggest.appliesTo settings.ai.stages.names.analyze_job, settings.ai.stages.names.match_evidence, settings.ai.stages.names.strategy'
      )
    ).toBeVisible();
  });

  it('pins every offered stage to the suggested model on accept', async () => {
    const user = userEvent.setup();
    render(<StageSuggestionBanner {...props} />);

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
  });

  it('renders nothing when nothing smaller is installed', () => {
    const { container } = render(
      <StageSuggestionBanner {...props} installedModels={['qwen3:32b']} />
    );

    expect(container).toBeEmptyDOMElement();
  });
});
