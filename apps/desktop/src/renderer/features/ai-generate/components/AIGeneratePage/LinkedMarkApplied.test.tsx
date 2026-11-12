import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { LinkedMarkApplied } from './LinkedMarkApplied';

const mutate = vi.fn();
let gens: Array<{ jobUrl: string; applicationId?: string }> = [];
let apps: Array<{ id: string; status: string }> = [];

vi.mock('@/services', () => ({
  useApplications: () => ({ data: apps }),
  useSetApplicationStatus: () => ({ mutate, isPending: false }),
}));
vi.mock('@/services/use-ai-generations', () => ({
  useAiGenerations: () => ({ data: gens }),
}));
vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));

const URL = 'https://acme.com/job/1';
const BTN = /applications\.detail\.markApplied$/;

describe('LinkedMarkApplied', () => {
  beforeEach(() => {
    mutate.mockReset();
    gens = [{ jobUrl: URL, applicationId: 'a1' }];
    apps = [{ id: 'a1', status: 'saved' }];
  });

  it('renders nothing without a job url', () => {
    render(<LinkedMarkApplied jobUrl={undefined} />);
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('renders nothing when no generation matches the url', () => {
    gens = [{ jobUrl: 'https://other.com', applicationId: 'a1' }];
    render(<LinkedMarkApplied jobUrl={URL} />);
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('marks the linked saved application as applied', () => {
    render(<LinkedMarkApplied jobUrl={URL} />);
    fireEvent.click(screen.getByRole('button', { name: BTN }));
    expect(mutate.mock.calls[0]?.[0]).toEqual({ id: 'a1', status: 'applied' });
  });

  it('renders nothing once the application is applied', () => {
    apps = [{ id: 'a1', status: 'applied' }];
    render(<LinkedMarkApplied jobUrl={URL} />);
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('renders nothing when the application is missing from the list', () => {
    apps = [];
    render(<LinkedMarkApplied jobUrl={URL} />);
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });
});
