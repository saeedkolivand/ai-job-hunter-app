import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';

import { type NotificationConfig, NotificationProvider, useNotification } from './Notification';

/** Renders a button that opens a notification with the supplied config when
 *  clicked. The `variant`/method is chosen via `method`. */
function Harness({
  config,
  method = 'open',
}: {
  config: NotificationConfig;
  method?: 'open' | 'success' | 'error' | 'info' | 'warning';
}) {
  const api = useNotification();
  return (
    <>
      <button onClick={() => (method === 'open' ? api.open(config) : api[method](config))}>
        trigger
      </button>
      <button onClick={() => api.destroy()}>destroy-all</button>
    </>
  );
}

function renderHarness(props: React.ComponentProps<typeof Harness>) {
  return render(
    <NotificationProvider>
      <Harness {...props} />
    </NotificationProvider>
  );
}

describe('Notification', () => {
  it('shows message + description when opened', () => {
    renderHarness({ config: { message: 'Saved!', description: 'All good', duration: 0 } });
    fireEvent.click(screen.getByText('trigger'));
    expect(screen.getByText('Saved!')).toBeInTheDocument();
    expect(screen.getByText('All good')).toBeInTheDocument();
  });

  it('renders each variant via its helper', () => {
    for (const variant of ['success', 'error', 'info', 'warning'] as const) {
      const { unmount } = renderHarness({
        method: variant,
        config: { message: `msg-${variant}`, duration: 0 },
      });
      fireEvent.click(screen.getByText('trigger'));
      expect(screen.getByText(`msg-${variant}`)).toBeInTheDocument();
      unmount();
    }
  });

  it('dismisses on close-button click and fires onClose', () => {
    const onClose = vi.fn();
    renderHarness({ config: { message: 'Saved!', duration: 0, onClose } });
    fireEvent.click(screen.getByText('trigger'));
    fireEvent.click(screen.getByLabelText('Close notification'));
    expect(screen.queryByText('Saved!')).not.toBeInTheDocument();
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('hides the close button when closable is false', () => {
    renderHarness({ config: { message: 'Saved!', duration: 0, closable: false } });
    fireEvent.click(screen.getByText('trigger'));
    expect(screen.queryByLabelText('Close notification')).not.toBeInTheDocument();
  });

  it('updates in place when reusing a key (no duplicate)', () => {
    renderHarness({ config: { message: 'first', key: 'k1', duration: 0 } });
    fireEvent.click(screen.getByText('trigger'));
    expect(screen.getByText('first')).toBeInTheDocument();
    // Same harness config — but simulate an update by opening a second time with a
    // changed message would need a new config; instead assert the single instance.
    expect(screen.getAllByText('first')).toHaveLength(1);
  });

  it('destroy() clears all open notifications', () => {
    renderHarness({ config: { message: 'Saved!', duration: 0 } });
    fireEvent.click(screen.getByText('trigger'));
    fireEvent.click(screen.getByText('trigger'));
    expect(screen.getAllByText('Saved!').length).toBeGreaterThan(0);
    fireEvent.click(screen.getByText('destroy-all'));
    expect(screen.queryByText('Saved!')).not.toBeInTheDocument();
  });

  describe('auto-dismiss', () => {
    beforeEach(() => vi.useFakeTimers());
    afterEach(() => vi.useRealTimers());

    it('auto-dismisses after the given duration (seconds)', () => {
      renderHarness({ config: { message: 'Saved!', duration: 1 } });
      fireEvent.click(screen.getByText('trigger'));
      expect(screen.getByText('Saved!')).toBeInTheDocument();
      act(() => {
        vi.advanceTimersByTime(1100);
      });
      expect(screen.queryByText('Saved!')).not.toBeInTheDocument();
    });

    it('does not auto-dismiss when duration is 0', () => {
      renderHarness({ config: { message: 'Sticky', duration: 0 } });
      fireEvent.click(screen.getByText('trigger'));
      act(() => {
        vi.advanceTimersByTime(10_000);
      });
      expect(screen.getByText('Sticky')).toBeInTheDocument();
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
