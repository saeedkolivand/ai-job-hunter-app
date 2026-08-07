import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import { TrimPanel } from './index';

const suggestions = {
  maxPages: 2,
  lines: [
    { text: 'Organised the team offsite', hits: [], score: 0 },
    { text: 'Shipped Docker containers', hits: ['docker'], score: 1 },
  ],
};

const useTrimSuggestions = vi.fn();
vi.mock('@/services/use-match', () => ({
  useTrimSuggestions: (...args: unknown[]) => useTrimSuggestions(...args),
}));

describe('TrimPanel', () => {
  it('stays hidden while the document is within the market length', () => {
    useTrimSuggestions.mockReturnValue({ data: suggestions });
    const { container } = render(
      <TrimPanel resumeText="body" jobText="docker role" pages={2} locale="us" />
    );
    expect(container).toBeEmptyDOMElement();
  });

  it('surfaces the weakest lines once the document runs over', () => {
    useTrimSuggestions.mockReturnValue({ data: suggestions });
    render(<TrimPanel resumeText="body" jobText="docker role" pages={3} locale="us" />);

    expect(screen.getByText('Organised the team offsite')).toBeInTheDocument();
    // Keyword hits are shown so the user can see WHY a line ranks where it does.
    expect(screen.getByText('docker')).toBeInTheDocument();
  });

  it('respects a market that tolerates a longer document', () => {
    // 3 pages is over the US target but exactly at DACH's — nothing to advise.
    useTrimSuggestions.mockReturnValue({ data: { ...suggestions, maxPages: 3 } });
    const { container } = render(
      <TrimPanel resumeText="body" jobText="docker role" pages={3} locale="de" />
    );
    expect(container).toBeEmptyDOMElement();
  });

  it('does not query until the document could possibly be over-long', () => {
    useTrimSuggestions.mockReturnValue({ data: undefined });
    render(<TrimPanel resumeText="body" jobText="docker role" pages={2} locale="us" />);
    // 4th arg is `enabled` — a 2-page résumé is under every market's target.
    expect(useTrimSuggestions).toHaveBeenLastCalledWith('body', 'docker role', 'us', false);

    render(<TrimPanel resumeText="body" jobText="docker role" pages={3} locale="us" />);
    expect(useTrimSuggestions).toHaveBeenLastCalledWith('body', 'docker role', 'us', true);
  });
});
