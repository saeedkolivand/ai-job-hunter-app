import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// i18n returns keys as-is so chips carry a stable accessible name.
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// Avoid loading the heavy generation barrel; provide the canonical audience ids.
vi.mock('@/lib/generate', () => ({
  INTERVIEW_AUDIENCES: ['recruiter', 'hiringManager', 'team', 'leadership', 'general'],
}));

import { AudienceSelector } from './AudienceSelector';

describe('AudienceSelector', () => {
  it('renders one chip per audience and marks the selected ones', () => {
    render(<AudienceSelector selected={['recruiter', 'hiringManager']} onToggle={() => {}} />);

    expect(screen.getAllByRole('button')).toHaveLength(5);
    expect(screen.getByRole('button', { name: /audience\.recruiter/ })).toHaveAttribute(
      'aria-pressed',
      'true'
    );
    expect(screen.getByRole('button', { name: /audience\.hiringManager/ })).toHaveAttribute(
      'aria-pressed',
      'true'
    );
    expect(screen.getByRole('button', { name: /audience\.team/ })).toHaveAttribute(
      'aria-pressed',
      'false'
    );
  });

  it('calls onToggle with the audience id when a chip is clicked', async () => {
    const onToggle = vi.fn();
    const user = userEvent.setup();
    render(<AudienceSelector selected={[]} onToggle={onToggle} />);

    await user.click(screen.getByRole('button', { name: /audience\.team/ }));

    expect(onToggle).toHaveBeenCalledWith('team');
  });
});
