import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { SourceBadge } from './SourceBadge';

describe('SourceBadge', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('renders the known platform label', () => {
    render(<SourceBadge source="linkedin" />);
    expect(screen.getByText('LinkedIn')).toBeInTheDocument();
  });

  it('falls back to the raw source name for unknown platforms', () => {
    render(<SourceBadge source="monster" />);
    expect(screen.getByText('monster')).toBeInTheDocument();
  });

  it('opens the url in a new tab when clicked', async () => {
    const open = vi.spyOn(window, 'open').mockImplementation(() => null);
    render(<SourceBadge source="indeed" url="https://example.com/job" />);
    await userEvent.click(screen.getByText('Indeed'));
    expect(open).toHaveBeenCalledWith('https://example.com/job', '_blank');
  });

  it('does nothing on click without a url', async () => {
    const open = vi.spyOn(window, 'open').mockImplementation(() => null);
    render(<SourceBadge source="xing" />);
    await userEvent.click(screen.getByText('XING'));
    expect(open).not.toHaveBeenCalled();
  });
});
