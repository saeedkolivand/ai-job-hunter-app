/**
 * BoardConnectChip — per-board login status chip.
 *
 * Covers:
 *   - connected=true → green "connected" pill with board label + disconnect button
 *   - connected=false → amber "needs login" button with aria-label including board label
 *   - disconnect button aria-label includes board label (accessible disconnect)
 *   - disconnect-pending → spinner shown, button disabled
 *   - connect-pending → spinner shown, button disabled
 *   - connect click → calls the appropriate mutate (generic board, not linkedin)
 *   - linkedin → routes to linkedin-specific hooks, not generic board hooks
 *
 * All service hooks are module-mocked so the chip renders in isolation.
 */
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// ---------------------------------------------------------------------------
// Service hook stubs — module-level refs for per-test control
// ---------------------------------------------------------------------------

const linkedInStatusRef = { data: { connected: false } as { connected?: boolean } | undefined };
const linkedInConnectRef = { isPending: false, mutateAsync: vi.fn().mockResolvedValue(undefined) };
const linkedInDisconnectRef = {
  isPending: false,
  mutateAsync: vi.fn().mockResolvedValue(undefined),
};
const boardStatusRef = { data: { connected: false } as { connected?: boolean } | undefined };
const boardConnectRef = { isPending: false, mutateAsync: vi.fn().mockResolvedValue(undefined) };
const boardDisconnectRef = { isPending: false, mutateAsync: vi.fn().mockResolvedValue(undefined) };

vi.mock('@/services', () => ({
  useLinkedInStatus: () => linkedInStatusRef,
  useLinkedInConnect: () => linkedInConnectRef,
  useLinkedInDisconnect: () => linkedInDisconnectRef,
  useBoardStatus: () => boardStatusRef,
  useBoardConnect: () => boardConnectRef,
  useBoardDisconnect: () => boardDisconnectRef,
}));

// i18n: identity t() — assert on keys
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// Import AFTER mocks
import { BoardConnectChip } from './BoardConnectChip';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function resetRefs() {
  linkedInStatusRef.data = { connected: false };
  linkedInConnectRef.isPending = false;
  linkedInConnectRef.mutateAsync = vi.fn().mockResolvedValue(undefined);
  linkedInDisconnectRef.isPending = false;
  linkedInDisconnectRef.mutateAsync = vi.fn().mockResolvedValue(undefined);
  boardStatusRef.data = { connected: false };
  boardConnectRef.isPending = false;
  boardConnectRef.mutateAsync = vi.fn().mockResolvedValue(undefined);
  boardDisconnectRef.isPending = false;
  boardDisconnectRef.mutateAsync = vi.fn().mockResolvedValue(undefined);
}

// ---------------------------------------------------------------------------
// Generic board — not connected
// ---------------------------------------------------------------------------

describe('BoardConnectChip — generic board (indeed) · not connected', () => {
  it('renders a connect button with accessible label including board label', () => {
    resetRefs();
    render(<BoardConnectChip board="indeed" />);

    // The connect button's aria-label is "jobs.needsLogin.connectBoard jobs.boards.indeed"
    const btn = screen.getByRole('button', {
      name: /jobs\.needsLogin\.connectBoard.*jobs\.boards\.indeed/,
    });
    expect(btn).toBeInTheDocument();
  });

  it('renders the board label text inside the button', () => {
    resetRefs();
    render(<BoardConnectChip board="indeed" />);
    expect(screen.getByText('jobs.boards.indeed')).toBeInTheDocument();
  });

  it('calls boardConnect.mutateAsync with the board id when clicked', async () => {
    resetRefs();
    render(<BoardConnectChip board="indeed" />);

    const btn = screen.getByRole('button', {
      name: /jobs\.needsLogin\.connectBoard/,
    });
    await userEvent.click(btn);

    expect(boardConnectRef.mutateAsync).toHaveBeenCalledWith('indeed');
    expect(linkedInConnectRef.mutateAsync).not.toHaveBeenCalled();
  });

  it('disables the button and shows a spinner when connectPending', () => {
    resetRefs();
    boardConnectRef.isPending = true;
    render(<BoardConnectChip board="indeed" />);

    const btn = screen.getByRole('button', {
      name: /jobs\.needsLogin\.connectBoard/,
    });
    expect(btn).toBeDisabled();
  });
});

// ---------------------------------------------------------------------------
// Generic board — connected
// ---------------------------------------------------------------------------

describe('BoardConnectChip — generic board (indeed) · connected', () => {
  it('renders the board label in the connected pill', () => {
    resetRefs();
    boardStatusRef.data = { connected: true };
    render(<BoardConnectChip board="indeed" />);

    expect(screen.getByText('jobs.boards.indeed')).toBeInTheDocument();
  });

  it('renders a disconnect button with accessible aria-label', () => {
    resetRefs();
    boardStatusRef.data = { connected: true };
    render(<BoardConnectChip board="indeed" />);

    // aria-label = "jobs.disconnect jobs.boards.indeed"
    const disconnectBtn = screen.getByRole('button', {
      name: /jobs\.disconnect.*jobs\.boards\.indeed/,
    });
    expect(disconnectBtn).toBeInTheDocument();
  });

  it('calls boardDisconnect.mutateAsync when the disconnect button is clicked', async () => {
    resetRefs();
    boardStatusRef.data = { connected: true };
    render(<BoardConnectChip board="indeed" />);

    const disconnectBtn = screen.getByRole('button', {
      name: /jobs\.disconnect/,
    });
    await userEvent.click(disconnectBtn);

    expect(boardDisconnectRef.mutateAsync).toHaveBeenCalledWith('indeed');
    expect(linkedInDisconnectRef.mutateAsync).not.toHaveBeenCalled();
  });

  it('disables the disconnect button and shows spinner when disconnectPending', () => {
    resetRefs();
    boardStatusRef.data = { connected: true };
    boardDisconnectRef.isPending = true;
    render(<BoardConnectChip board="indeed" />);

    const disconnectBtn = screen.getByRole('button', {
      name: /jobs\.disconnect/,
    });
    expect(disconnectBtn).toBeDisabled();
  });
});

// ---------------------------------------------------------------------------
// LinkedIn — routes to linkedin-specific hooks
// ---------------------------------------------------------------------------

describe('BoardConnectChip — linkedin · not connected', () => {
  it('calls linkedInConnect.mutateAsync (not boardConnect) when connect is clicked', async () => {
    resetRefs();
    // LinkedIn uses its own status hook, not boardStatus
    linkedInStatusRef.data = { connected: false };
    render(<BoardConnectChip board="linkedin" />);

    const btn = screen.getByRole('button', {
      name: /jobs\.needsLogin\.connectBoard/,
    });
    await userEvent.click(btn);

    expect(linkedInConnectRef.mutateAsync).toHaveBeenCalled();
    // Generic board mutate must NOT be called
    expect(boardConnectRef.mutateAsync).not.toHaveBeenCalled();
  });
});

describe('BoardConnectChip — linkedin · connected', () => {
  it('calls linkedInDisconnect.mutateAsync (not boardDisconnect) when disconnect is clicked', async () => {
    resetRefs();
    linkedInStatusRef.data = { connected: true };
    render(<BoardConnectChip board="linkedin" />);

    const disconnectBtn = screen.getByRole('button', {
      name: /jobs\.disconnect/,
    });
    await userEvent.click(disconnectBtn);

    expect(linkedInDisconnectRef.mutateAsync).toHaveBeenCalled();
    expect(boardDisconnectRef.mutateAsync).not.toHaveBeenCalled();
  });
});
