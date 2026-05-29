import { describe, expect, it } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { ToastProvider, useToast } from './Toast';

function Harness() {
  const toast = useToast();
  return <button onClick={() => toast('Toasted', 'info', 0)}>trigger</button>;
}

describe('Toast (Notification shim)', () => {
  it('re-exports the notification provider and hook', () => {
    render(
      <ToastProvider>
        <Harness />
      </ToastProvider>
    );
    fireEvent.click(screen.getByText('trigger'));
    expect(screen.getByText('Toasted')).toBeInTheDocument();
  });
});
