import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { MarkAppliedButton } from './index';

type Opts = { onSuccess?: (d: { error?: string }) => void; onError?: () => void };
const mutate = vi.fn();
let pending = false;
vi.mock('@/services', () => ({
  useSetApplicationStatus: () => ({ mutate, isPending: pending }),
}));
vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));

const BTN = { name: /markApplied$/ };
const resolveWith = (d: { error?: string }) => (_v: unknown, o?: Opts) => o?.onSuccess?.(d);

describe('MarkAppliedButton', () => {
  beforeEach(() => {
    mutate.mockReset();
    pending = false;
  });

  it('sets applied on click', () => {
    render(<MarkAppliedButton applicationId="a1" status="saved" />);
    fireEvent.click(screen.getByRole('button', BTN));
    expect(mutate.mock.calls[0]?.[0]).toEqual({ id: 'a1', status: 'applied' });
  });

  it('shows an error when the call rejects', () => {
    mutate.mockImplementationOnce((_v: unknown, o?: Opts) => o?.onError?.());
    render(<MarkAppliedButton applicationId="a1" status="saved" />);
    fireEvent.click(screen.getByRole('button', BTN));
    expect(screen.getByRole('alert')).toHaveTextContent('applications.detail.markAppliedError');
  });

  it('shows an error, not a confirmation, when the call resolves { error }', () => {
    mutate.mockImplementationOnce(resolveWith({ error: 'x' }));
    const { rerender } = render(<MarkAppliedButton applicationId="a1" status="saved" />);
    fireEvent.click(screen.getByRole('button', BTN));
    expect(screen.getByRole('alert')).toBeInTheDocument();
    rerender(<MarkAppliedButton applicationId="a1" status="applied" />);
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  it('is disabled while pending', () => {
    pending = true;
    render(<MarkAppliedButton applicationId="a1" status="saved" />);
    expect(screen.getByRole('button', BTN)).toBeDisabled();
  });

  it('renders nothing once applied', () => {
    const { container } = render(<MarkAppliedButton applicationId="a1" status="applied" />);
    expect(container).toBeEmptyDOMElement();
  });

  it('clears the error on a successful retry and announces success', () => {
    mutate.mockImplementationOnce((_v: unknown, o?: Opts) => o?.onError?.());
    mutate.mockImplementationOnce(resolveWith({}));
    const { rerender } = render(<MarkAppliedButton applicationId="a1" status="saved" />);
    fireEvent.click(screen.getByRole('button', BTN));
    expect(screen.getByRole('alert')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', BTN));
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    rerender(<MarkAppliedButton applicationId="a1" status="applied" />);
    expect(screen.getByRole('status')).toHaveTextContent('applications.detail.markedApplied');
  });

  it('brings the button back if the status returns to saved', () => {
    mutate.mockImplementationOnce(resolveWith({}));
    const { rerender } = render(<MarkAppliedButton applicationId="a1" status="saved" />);
    fireEvent.click(screen.getByRole('button', BTN));
    rerender(<MarkAppliedButton applicationId="a1" status="applied" />);
    expect(screen.getByRole('status')).toBeInTheDocument();
    rerender(<MarkAppliedButton applicationId="a1" status="saved" />);
    expect(screen.getByRole('button', BTN)).toBeInTheDocument();
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });
});
