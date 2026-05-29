import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';

import { NotificationProvider, useNotification } from './Notification';

function Harness({
  message = 'Saved!',
  variant,
  duration,
}: {
  message?: string;
  variant?: 'success' | 'error' | 'info' | 'warning';
  duration?: number;
}) {
  const notify = useNotification();
  return <button onClick={() => notify(message, variant, duration)}>trigger</button>;
}

describe('Notification', () => {
  it('shows a notification when notify is called', () => {
    render(
      <NotificationProvider>
        <Harness duration={0} />
      </NotificationProvider>
    );
    fireEvent.click(screen.getByText('trigger'));
    expect(screen.getByText('Saved!')).toBeInTheDocument();
  });

  it('renders each variant', () => {
    for (const variant of ['success', 'error', 'info', 'warning'] as const) {
      const { unmount } = render(
        <NotificationProvider>
          <Harness message={`msg-${variant}`} variant={variant} duration={0} />
        </NotificationProvider>
      );
      fireEvent.click(screen.getByText('trigger'));
      expect(screen.getByText(`msg-${variant}`)).toBeInTheDocument();
      unmount();
    }
  });

  it('dismisses on close-button click', () => {
    render(
      <NotificationProvider>
        <Harness duration={0} />
      </NotificationProvider>
    );
    fireEvent.click(screen.getByText('trigger'));
    const closeButton = screen.getAllByRole('button').find((b) => b.textContent === '');
    expect(closeButton).toBeDefined();
    fireEvent.click(closeButton as HTMLElement);
    expect(screen.queryByText('Saved!')).not.toBeInTheDocument();
  });

  describe('auto-dismiss', () => {
    beforeEach(() => vi.useFakeTimers());
    afterEach(() => vi.useRealTimers());

    it('auto-dismisses after the given duration', () => {
      render(
        <NotificationProvider>
          <Harness duration={1000} />
        </NotificationProvider>
      );
      fireEvent.click(screen.getByText('trigger'));
      expect(screen.getByText('Saved!')).toBeInTheDocument();
      act(() => {
        vi.advanceTimersByTime(1100);
      });
      expect(screen.queryByText('Saved!')).not.toBeInTheDocument();
    });
  });

  it('throws when useNotification is used outside the provider', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    function Bare() {
      useNotification();
      return null;
    }
    expect(() => render(<Bare />)).toThrow(/within NotificationProvider/);
    vi.restoreAllMocks();
  });
});
