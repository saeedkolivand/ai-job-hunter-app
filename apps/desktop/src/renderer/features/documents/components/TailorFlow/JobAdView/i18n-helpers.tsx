/** Render helpers for the real-copy Score-tab tests (kept apart from `i18n-support.tsx`, see its header). */
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import i18n from '@ajh/translations';

import { JobAdView } from '../JobAdView';
import { makeProps } from './i18n-support';

async function openScoreTab() {
  await userEvent.click(screen.getByText(i18n.t('autopilot.apply.jobAdView.scoreTab')));
}

/** Renders the view and opens its Score tab. */
export async function renderScoreTab(overrides: Parameters<typeof makeProps>[0] = {}) {
  const view = render(<JobAdView {...makeProps(overrides)} />);
  await openScoreTab();
  return view;
}
