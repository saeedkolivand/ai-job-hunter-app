/** Render helpers for the ReferralModal tests (kept apart from `modal.test-support.tsx`, see its header). */
import { type Mock, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';

import type { AutopilotFoundJob } from '@ajh/shared';

import { ReferralModal } from './index';

const JOB: AutopilotFoundJob = {
  title: 'Senior Engineer',
  company: 'Acme',
  url: 'https://acme.com/jobs/1',
  foundAt: 1_000,
};

const RESUME = 'Jane Doe\nSenior Engineer with 8 years of experience.';

export function renderModal(jobOverrides: Partial<AutopilotFoundJob> = {}) {
  const onClose = vi.fn() as Mock;
  render(<ReferralModal job={{ ...JOB, ...jobOverrides }} resume={RESUME} onClose={onClose} />);
  return { onClose };
}

/** Fill the person-name input so canSave can become true. */
export function fillPersonName(name = 'Bob Chen') {
  const input = screen.getByPlaceholderText('autopilot.referral.personNamePlaceholder');
  fireEvent.change(input, { target: { value: name } });
}

/** Switch to the given channel via its SegmentedControl radio. */
export function switchChannel(channelKey: string) {
  fireEvent.click(screen.getByRole('radio', { name: `autopilot.referral.channel.${channelKey}` }));
}

export const getSaveBtn = () => screen.getByRole('button', { name: /autopilot\.referral\.save/i });

/** Clicks Save inside `act`. */
export function clickSave() {
  act(() => {
    fireEvent.click(getSaveBtn());
  });
}
