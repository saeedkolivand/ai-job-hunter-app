import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { RefreshButton } from './RefreshButton';

describe('RefreshButton', () => {
  it('calls onRefresh when clicked', async () => {
    const onRefresh = vi.fn().mockResolvedValue(undefined);
    render(<RefreshButton onRefresh={onRefresh}>Refresh</RefreshButton>);
    await userEvent.click(screen.getByRole('button'));
    await waitFor(() => expect(onRefresh).toHaveBeenCalledOnce());
  });

  it('does not call onRefresh when disabled', async () => {
    const onRefresh = vi.fn();
    render(<RefreshButton onRefresh={onRefresh} disabled />);
    await userEvent.click(screen.getByRole('button')).catch(() => {});
    expect(onRefresh).not.toHaveBeenCalled();
  });

  it('ignores re-entrant clicks while refreshing', async () => {
    let resolve: () => void = () => {};
    const onRefresh = vi.fn().mockImplementation(
      () =>
        new Promise<void>((r) => {
          resolve = r;
        })
    );
    render(<RefreshButton onRefresh={onRefresh} />);
    const btn = screen.getByRole('button');
    await userEvent.click(btn);
    await userEvent.click(btn).catch(() => {});
    expect(onRefresh).toHaveBeenCalledOnce();
    resolve();
  });
});
