/**
 * PostingRow — handleTailor + handleSave jobDescription payload tests.
 *
 * Strategy:
 *  - useSaveFromPosting is mocked (the mutation is the observable under test).
 *  - Heavy deps (router, services, store, ui) are stubbed at the module level.
 *  - motion/react, lucide-react, @ajh/ui primitives are stubbed.
 *  - @/features/jobs/providers mocked so usePostingActions (which calls useRowMatchScore
 *    internally) does not require MatchScoresProvider.
 *  - No QueryClient / AppClient / Tauri provider is needed.
 *  - The core assertion: mutateAsync is called with an object that includes
 *    `jobDescription: posting.description` for BOTH the Tailor and Save buttons.
 *
 * noUncheckedIndexedAccess: all mock.calls accesses are guarded.
 */

import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── motion/react ──────────────────────────────────────────────────────────────

vi.mock('motion/react', () => ({
  motion: {
    div: React.forwardRef(
      (
        { children, ...rest }: React.HTMLAttributes<HTMLDivElement>,
        ref: React.Ref<HTMLDivElement>
      ) => (
        <div ref={ref} {...rest}>
          {children}
        </div>
      )
    ),
  },
}));

// ── lucide-react — return null icons (no svg overhead in jsdom) ───────────────

vi.mock('lucide-react', () => ({
  Bookmark: () => null,
  Building2: () => null,
  CircleCheck: () => null,
  Copy: () => null,
  ExternalLink: () => null,
  Eye: () => null,
  MapPin: () => null,
  Save: () => null,
  Wand2: () => null,
}));

// ── @ajh/ui primitives — minimal stubs exposing onClick ──────────────────────

vi.mock('@ajh/ui', () => ({
  ActionMenu: () => null,
  Button: ({
    children,
    onClick,
    disabled,
  }: {
    children: React.ReactNode;
    onClick?: () => void;
    disabled?: boolean;
  }) => (
    <div role="button" tabIndex={0} onClick={onClick} aria-disabled={disabled}>
      {children}
    </div>
  ),
  SourceBadge: () => null,
  Tag: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
  transition: { fast: {} },
  useNotification: () => ({ success: vi.fn(), error: vi.fn() }),
}));

// ── CompanyAvatar ─────────────────────────────────────────────────────────────

vi.mock('@/features/jobs/components/CompanyAvatar', () => ({
  CompanyAvatar: () => null,
}));

// ── useRowMatchScore (via usePostingActions → MatchScoresProvider) ────────────
// PostingRow no longer renders RowMatchScore directly, but usePostingActions
// still calls useRowMatchScore internally. Mock the provider to avoid the
// "must be used within MatchScoresProvider" throw.

vi.mock('@/features/jobs/providers', () => ({
  useRowMatchScore: () => ({ score: undefined }),
}));

// ── @tanstack/react-router — navigation spy ───────────────────────────────────

const mockNavigate = vi.fn().mockResolvedValue(undefined);

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => mockNavigate,
}));

// ── useOpenExternal + usePersistJob (services) ────────────────────────────────

vi.mock('@/services', () => ({
  useOpenExternal: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
  usePersistJob: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
}));

// ── useSaveFromPosting — the mutation under test ──────────────────────────────

const mockMutateAsync = vi.fn().mockResolvedValue({ id: 'app-123' });

vi.mock('@/services/use-applications', () => ({
  useSaveFromPosting: () => ({
    mutateAsync: mockMutateAsync,
    isPending: false,
  }),
}));

// ── useSessionStore — capture setApplicationApply calls ──────────────────────

const mockSetApplicationApply = vi.fn();

vi.mock('@/store/session-store', () => ({
  useSessionStore: (sel: (s: { setApplicationApply: typeof mockSetApplicationApply }) => unknown) =>
    sel({ setApplicationApply: mockSetApplicationApply }),
}));

// ── component under test (after all mocks) ───────────────────────────────────

import { PostingRow } from './index';

// ── fixture ───────────────────────────────────────────────────────────────────

const POSTING = {
  id: 'post-1',
  source: 'linkedin',
  externalId: 'ext-1',
  url: 'https://linkedin.com/jobs/1',
  title: 'Software Engineer',
  company: 'Acme Corp',
  location: 'Berlin',
  remote: false,
  description: 'Exciting role with Rust and TypeScript skills required.',
  capturedAt: 1_700_000_000_000,
};

const formatRelativeTime = () => '2d ago';

function renderRow(description = POSTING.description) {
  return render(
    <PostingRow posting={{ ...POSTING, description }} formatRelativeTime={formatRelativeTime} />
  );
}

// ── reset ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  mockMutateAsync.mockClear();
  mockMutateAsync.mockResolvedValue({ id: 'app-123' });
  mockNavigate.mockClear();
  mockSetApplicationApply.mockClear();
});

// ─────────────────────────────────────────────────────────────────────────────
// handleTailor — jobDescription payload
// ─────────────────────────────────────────────────────────────────────────────

describe('PostingRow — handleTailor', () => {
  it('calls mutateAsync with jobDescription equal to posting.description', async () => {
    const user = userEvent.setup();
    renderRow();

    // The Tailor button renders with translation key 'jobs.tailor'.
    await user.click(screen.getByRole('button', { name: /jobs\.tailor/i }));

    expect(mockMutateAsync).toHaveBeenCalledTimes(1);
    const call = mockMutateAsync.mock.calls[0];
    const arg = call?.[0] as Record<string, unknown> | undefined;
    expect(arg).toMatchObject({
      jobUrl: POSTING.url,
      board: POSTING.source,
      company: POSTING.company,
      title: POSTING.title,
      jobDescription: POSTING.description,
    });
  });

  it('carries the exact description string, not a truncated or empty value', async () => {
    const user = userEvent.setup();
    const longDesc = 'Detailed job ad: '.padEnd(1200, 'x');
    renderRow(longDesc);

    await user.click(screen.getByRole('button', { name: /jobs\.tailor/i }));

    const call = mockMutateAsync.mock.calls[0];
    const arg = call?.[0] as Record<string, unknown> | undefined;
    expect(arg?.jobDescription).toBe(longDesc);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// handleSave — jobDescription payload
// ─────────────────────────────────────────────────────────────────────────────

describe('PostingRow — handleSave', () => {
  it('calls mutateAsync with jobDescription equal to posting.description', async () => {
    const user = userEvent.setup();
    renderRow();

    // The Save button renders with translation key 'applications.save'.
    await user.click(screen.getByRole('button', { name: /applications\.save/i }));

    expect(mockMutateAsync).toHaveBeenCalledTimes(1);
    const call = mockMutateAsync.mock.calls[0];
    const arg = call?.[0] as Record<string, unknown> | undefined;
    expect(arg).toMatchObject({
      jobUrl: POSTING.url,
      board: POSTING.source,
      company: POSTING.company,
      title: POSTING.title,
      jobDescription: POSTING.description,
    });
  });

  it('carries the exact description string on save', async () => {
    const user = userEvent.setup();
    const specificDesc = 'We need a Rust expert with 5+ years.';
    renderRow(specificDesc);

    await user.click(screen.getByRole('button', { name: /applications\.save/i }));

    const call = mockMutateAsync.mock.calls[0];
    const arg = call?.[0] as Record<string, unknown> | undefined;
    expect(arg?.jobDescription).toBe(specificDesc);
  });
});
