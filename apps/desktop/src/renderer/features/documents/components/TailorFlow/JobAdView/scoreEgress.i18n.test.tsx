/**
 * JobAdView — Score tab (real en copy): CLI-agent egress disclosure.
 *
 * A CLI-agent provider (Claude Code, Codex, Gemini CLI) egresses despite reading
 * as "local" elsewhere in this app; the translation path a foreign-language
 * posting routes through can send it the job ad text. Undisclosed, this surface
 * would say nothing about it.
 *
 * Shared stubs live in `i18n-support.tsx` (see its header).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/react';

import i18n from '@ajh/translations';

import { renderScoreTab } from './i18n-helpers';
import { resetStub, setScore, stub } from './i18n-support';

vi.mock('@/services', async () => (await import('./i18n-support')).servicesModule);
vi.mock('@/components/ui/ModelSelector', async () => {
  return (await import('./i18n-support')).modelSelectorModule;
});
vi.mock('@/lib/generate', async () => (await import('./i18n-support')).generateModule);

beforeEach(resetStub);

const egressText = (provider: string) =>
  i18n.t('autopilot.apply.jobAdView.score.cliAgentEgress', { provider });
const t = (key: string) => i18n.t(`autopilot.apply.jobAdView.score.${key}`);
const fail = () => ({ data: undefined, isLoading: false, isError: true, refetch: vi.fn() });
const loading = () => ({ data: undefined, isLoading: true, isError: false, refetch: vi.fn() });

describe('JobAdView — Score tab: CLI-agent egress disclosure', () => {
  it('discloses egress, with the provider named, when the active provider is a CLI agent', async () => {
    stub.provider = 'claude-code';
    setScore();
    await renderScoreTab();

    expect(screen.getByText(egressText('Claude Code'))).toBeInTheDocument();
  });

  // Cloud providers already read as external everywhere else in the app; this
  // disclosure exists specifically because CLI agents are the ones that WRONGLY
  // read as local (`is_local()`) despite egressing.
  it.each([
    ['a local provider (ollama)', 'ollama', 'Ollama (Local)'],
    ['a cloud API provider (openai) — only CLI agents egress unexpectedly', 'openai', 'OpenAI'],
  ])('does NOT disclose egress for %s', async (_name, provider, label) => {
    stub.provider = provider;
    setScore();
    await renderScoreTab();

    // Anchored on the untranslated key resolving to an actual sentence, so a
    // false negative here can't hide behind a missing-key echo.
    expect(egressText(label)).not.toBe('autopilot.apply.jobAdView.score.cliAgentEgress');
    expect(screen.queryByText(egressText(label))).not.toBeInTheDocument();
  });

  // Regression: the disclosure used to render unconditionally above the
  // five-way body branch, so it warned about egress on states that will
  // NEVER send anything (nothing is scored yet). It must render ONLY where a
  // score is actually loading or shown.
  it.each([
    ['no-résumé', { resumeId: undefined }, i18n.t('jobs.scoreNoResume')],
    ['no-posting', { jobDesc: '' }, i18n.t('autopilot.apply.jobAdView.score.noPosting')],
  ])(
    'does NOT disclose egress on the %s reason — nothing will ever be sent from that state',
    async (_name, overrides, reason) => {
      stub.provider = 'claude-code';
      await renderScoreTab(overrides);

      expect(screen.getByText(reason)).toBeInTheDocument();
      expect(screen.queryByText(egressText('Claude Code'))).not.toBeInTheDocument();
    }
  );

  // The error branch is `scoreError || (score && !isMeasured(score))`. Reaching
  // it at all means the request went out; the second disjunct means it came
  // BACK. Either way the posting text was already sent, so this is the one
  // failure state that still owes the user a disclosure.
  it.each([
    ['while the score is loading — a call is genuinely in flight', loading(), t('loading')],
    [
      'on the error state — a resolved-but-unusable response already egressed',
      fail(),
      t('errorTitle'),
    ],
  ])('DOES disclose egress %s', async (_name, score, marker) => {
    stub.provider = 'claude-code';
    stub.score = score;
    await renderScoreTab();

    expect(screen.getByText(marker)).toBeInTheDocument();
    expect(screen.getByText(egressText('Claude Code'))).toBeInTheDocument();
  });
});
