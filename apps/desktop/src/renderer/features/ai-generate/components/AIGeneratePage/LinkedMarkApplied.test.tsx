import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { LinkedMarkApplied } from './LinkedMarkApplied';

const mutate = vi.fn();
let apps: Array<{ id: string; jobUrl: string; status: string }> = [];

vi.mock('@/services', () => ({
  useApplications: () => ({ data: apps }),
  useSetApplicationStatus: () => ({ mutate, isPending: false }),
}));
vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));

// Stored URLs are normalized (lowercase, no www/query/trailing slash).
const STORED = 'https://linkedin.com/jobs/view/123';
const BTN = { name: /applications\.detail\.markApplied$/ };

describe('LinkedMarkApplied', () => {
  beforeEach(() => {
    mutate.mockReset();
    apps = [{ id: 'a1', jobUrl: STORED, status: 'saved' }];
  });

  it('renders nothing without a job url', () => {
    const { container } = render(<LinkedMarkApplied jobUrl={undefined} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('renders nothing when no application matches the url', () => {
    const { container } = render(<LinkedMarkApplied jobUrl="https://other.com/x" />);
    expect(container).toBeEmptyDOMElement();
  });

  it('matches a raw www/trailing-slash/uppercase/utm page url to the normalized stored one', () => {
    render(<LinkedMarkApplied jobUrl="https://www.LinkedIn.com/jobs/view/123/?utm_source=x" />);
    fireEvent.click(screen.getByRole('button', BTN));
    expect(mutate.mock.calls[0]?.[0]).toEqual({ id: 'a1', status: 'applied' });
  });

  it('leaves its bar empty (hidden via empty:hidden) once the application is applied', () => {
    apps = [{ id: 'a1', jobUrl: STORED, status: 'applied' }];
    const { container } = render(<LinkedMarkApplied jobUrl={STORED} />);
    expect(container.firstElementChild).toBeEmptyDOMElement();
    expect(container.firstElementChild).toHaveClass('empty:hidden');
  });
});
