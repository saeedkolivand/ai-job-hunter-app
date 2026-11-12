import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { MarkAppliedButton } from './index';

const mutate = vi.fn();
let pending = false;
vi.mock('@/services', () => ({
  useSetApplicationStatus: () => ({ mutate, isPending: pending }),
}));
vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));

describe('MarkAppliedButton', () => {
  it('sets applied on click', () => {
    render(<MarkAppliedButton applicationId="a1" status="saved" />);
    fireEvent.click(screen.getByRole('button', { name: /markApplied/ }));
    expect(mutate.mock.calls[0]?.[0]).toEqual({ id: 'a1', status: 'applied' });
  });

  it('shows an error when the call fails', () => {
    mutate.mockImplementationOnce((_v: unknown, o?: { onError?: () => void }) => o?.onError?.());
    render(<MarkAppliedButton applicationId="a1" status="saved" />);
    fireEvent.click(screen.getByRole('button', { name: /markApplied/ }));
    expect(screen.getByRole('alert')).toHaveTextContent('applications.detail.markAppliedError');
  });

  it('is disabled while pending', () => {
    pending = true;
    render(<MarkAppliedButton applicationId="a1" status="saved" />);
    expect(screen.getByRole('button', { name: /markApplied/ })).toBeDisabled();
    pending = false;
  });

  it('renders nothing once applied', () => {
    const { container } = render(<MarkAppliedButton applicationId="a1" status="applied" />);
    expect(container).toBeEmptyDOMElement();
  });
});
